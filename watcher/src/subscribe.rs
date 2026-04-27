//! Solana log subscription + Anchor event parsing.
//!
//! Watchers ingest the firehose of program logs and decode `ThoughtSubmitted`
//! events. Anchor emits events as base64-encoded payloads on a log line that
//! starts with `Program data:`. Each payload is:
//!
//! ```text
//! [u8; 8]   event discriminator (= sha256("event:<EventName>")[..8])
//! [..]      Borsh-serialized event fields
//! ```
//!
//! We match against [`crate::types::THOUGHT_SUBMITTED_DISCRIMINATOR`] and
//! decode the fields manually using the layout documented on
//! [`crate::types::ThoughtSubmittedEvent`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

use crate::types::{ThoughtSubmittedEvent, THOUGHT_SUBMITTED_DISCRIMINATOR};

#[derive(Debug, Error)]
pub enum SubscribeError {
    #[error("websocket connect failed: {0}")]
    Connect(String),
    #[error("malformed Anchor event payload: {0}")]
    BadPayload(&'static str),
    #[error("invalid pubkey: {0}")]
    BadPubkey(String),
}

/// Trait so the verifier pipeline can be tested without a live RPC.
#[async_trait]
pub trait LogStream: Send + Sync {
    /// Spawn the background subscription task and return a receiver that
    /// emits decoded events. Returning the receiver lets callers wire in
    /// backpressure via channel size.
    async fn run(self: Arc<Self>, tx: mpsc::Sender<ThoughtSubmittedEvent>) -> anyhow::Result<()>;
}

// ---------------------------------------------------------------------------
// Live websocket implementation
// ---------------------------------------------------------------------------

/// Real implementation: subscribes via Solana's pubsub websocket and pushes
/// decoded events into the channel.
pub struct WebsocketLogStream {
    pub ws_url: String,
    pub program_id: Pubkey,
}

impl WebsocketLogStream {
    pub fn new(ws_url: impl Into<String>, program_id: Pubkey) -> Self {
        Self {
            ws_url: ws_url.into(),
            program_id,
        }
    }
}

#[async_trait]
impl LogStream for WebsocketLogStream {
    async fn run(self: Arc<Self>, tx: mpsc::Sender<ThoughtSubmittedEvent>) -> anyhow::Result<()> {
        // Reconnect loop. Solana's websocket is flaky; spec §8 says watchers
        // are expected to be long-lived.
        loop {
            match self.connect_and_pump(&tx).await {
                Ok(()) => {
                    warn!("websocket pump returned cleanly — reconnecting");
                }
                Err(e) => {
                    error!(error = %e, "websocket pump failed; backing off 5s");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

impl WebsocketLogStream {
    async fn connect_and_pump(
        &self,
        tx: &mpsc::Sender<ThoughtSubmittedEvent>,
    ) -> anyhow::Result<()> {
        info!(ws_url = %self.ws_url, program = %self.program_id, "connecting watcher websocket");
        let client = PubsubClient::new(&self.ws_url)
            .await
            .map_err(|e| SubscribeError::Connect(e.to_string()))?;

        let filter = RpcTransactionLogsFilter::Mentions(vec![self.program_id.to_string()]);
        let cfg = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };
        let (mut stream, _unsub) = client
            .logs_subscribe(filter, cfg)
            .await
            .map_err(|e| SubscribeError::Connect(e.to_string()))?;

        while let Some(resp) = stream.next().await {
            let logs = resp.value.logs;
            for line in logs {
                if let Some(event) = match parse_anchor_event_line(&line) {
                    Ok(maybe) => maybe,
                    Err(e) => {
                        debug!(error = %e, line = %line, "skipping malformed log line");
                        None
                    }
                } {
                    if tx.send(event).await.is_err() {
                        warn!("verifier channel closed; ending log pump");
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mock implementation for tests
// ---------------------------------------------------------------------------

/// Mock log stream backed by a one-shot pre-filled mpsc receiver. The pipeline
/// behaves identically to the live version because it consumes the same
/// `mpsc::Receiver<ThoughtSubmittedEvent>` shape.
pub struct MockLogStream {
    /// Wrapped in a Mutex so the trait method can take `&self` and still
    /// drain the buffered events exactly once.
    events: tokio::sync::Mutex<Option<Vec<ThoughtSubmittedEvent>>>,
}

impl MockLogStream {
    pub fn new(events: Vec<ThoughtSubmittedEvent>) -> Self {
        Self {
            events: tokio::sync::Mutex::new(Some(events)),
        }
    }
}

#[async_trait]
impl LogStream for MockLogStream {
    async fn run(self: Arc<Self>, tx: mpsc::Sender<ThoughtSubmittedEvent>) -> anyhow::Result<()> {
        let events = {
            let mut g = self.events.lock().await;
            g.take().unwrap_or_default()
        };
        for ev in events {
            // Don't crash if the consumer has dropped — just stop.
            if tx.send(ev).await.is_err() {
                break;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a single log line. Returns `Ok(Some(event))` if this line was an
/// Anchor event payload for `ThoughtSubmitted`, `Ok(None)` for any other line,
/// `Err` only for malformed `Program data:` lines (so the caller can log).
pub fn parse_anchor_event_line(line: &str) -> Result<Option<ThoughtSubmittedEvent>, SubscribeError> {
    let prefix = "Program data: ";
    let Some(b64) = line.strip_prefix(prefix) else {
        return Ok(None);
    };
    let bytes = B64
        .decode(b64.trim())
        .map_err(|_| SubscribeError::BadPayload("base64 decode"))?;
    parse_anchor_event_bytes(&bytes)
}

/// Decode a raw Anchor event payload. Returns Ok(None) when the discriminator
/// matches some *other* event the program emits (we ignore those silently).
pub fn parse_anchor_event_bytes(
    bytes: &[u8],
) -> Result<Option<ThoughtSubmittedEvent>, SubscribeError> {
    if bytes.len() < 8 {
        return Err(SubscribeError::BadPayload("payload < 8 bytes"));
    }
    let (disc, body) = bytes.split_at(8);
    if disc != THOUGHT_SUBMITTED_DISCRIMINATOR {
        return Ok(None);
    }
    decode_thought_submitted_body(body).map(Some)
}

fn decode_thought_submitted_body(body: &[u8]) -> Result<ThoughtSubmittedEvent, SubscribeError> {
    // Expected layout (Borsh):
    //   8 fixed-size fields = 32+32+32+32+32+32+32+32 = 256 bytes (Pubkeys + [u8;32])
    //   8 bytes slot (u64 LE)
    //   4 bytes string length (u32 LE) + UTF-8 bytes
    let mut cursor = Cursor::new(body);
    let agent = cursor.read_pubkey()?;
    let thought_pda = cursor.read_pubkey()?;
    let model_id = cursor.read_array32()?;
    let input_commitment = cursor.read_array32()?;
    let output_commitment = cursor.read_array32()?;
    let trace_uri_hash = cursor.read_array32()?;
    let vrf_seed = cursor.read_array32()?;
    let policy_id = cursor.read_array32()?;
    let slot = cursor.read_u64_le()?;
    let trace_uri = cursor.read_borsh_string()?;

    Ok(ThoughtSubmittedEvent {
        agent,
        thought_pda,
        model_id,
        input_commitment,
        output_commitment,
        trace_uri_hash,
        vrf_seed,
        policy_id,
        slot,
        trace_uri,
    })
}

// ---------------------------------------------------------------------------
// Tiny zero-copy decoder helper
// ---------------------------------------------------------------------------

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], SubscribeError> {
        if self.pos + n > self.buf.len() {
            return Err(SubscribeError::BadPayload("unexpected EOF"));
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_pubkey(&mut self) -> Result<Pubkey, SubscribeError> {
        let s = self.take(32)?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(s);
        Ok(Pubkey::from(arr))
    }

    fn read_array32(&mut self) -> Result<[u8; 32], SubscribeError> {
        let s = self.take(32)?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(s);
        Ok(arr)
    }

    fn read_u64_le(&mut self) -> Result<u64, SubscribeError> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_le_bytes(a))
    }

    fn read_u32_le(&mut self) -> Result<u32, SubscribeError> {
        let s = self.take(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(s);
        Ok(u32::from_le_bytes(a))
    }

    fn read_borsh_string(&mut self) -> Result<String, SubscribeError> {
        let len = self.read_u32_le()? as usize;
        let s = self.take(len)?;
        std::str::from_utf8(s)
            .map(|s| s.to_owned())
            .map_err(|_| SubscribeError::BadPayload("non-utf8 trace_uri"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic Anchor event payload for round-trip testing.
    fn synth_payload(ev: &ThoughtSubmittedEvent) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&THOUGHT_SUBMITTED_DISCRIMINATOR);
        out.extend_from_slice(ev.agent.as_ref());
        out.extend_from_slice(ev.thought_pda.as_ref());
        out.extend_from_slice(&ev.model_id);
        out.extend_from_slice(&ev.input_commitment);
        out.extend_from_slice(&ev.output_commitment);
        out.extend_from_slice(&ev.trace_uri_hash);
        out.extend_from_slice(&ev.vrf_seed);
        out.extend_from_slice(&ev.policy_id);
        out.extend_from_slice(&ev.slot.to_le_bytes());
        out.extend_from_slice(&(ev.trace_uri.len() as u32).to_le_bytes());
        out.extend_from_slice(ev.trace_uri.as_bytes());
        out
    }

    fn fixture_event() -> ThoughtSubmittedEvent {
        ThoughtSubmittedEvent {
            agent: Pubkey::new_unique(),
            thought_pda: Pubkey::new_unique(),
            model_id: [1u8; 32],
            input_commitment: [2u8; 32],
            output_commitment: [3u8; 32],
            trace_uri_hash: [4u8; 32],
            vrf_seed: [5u8; 32],
            policy_id: [6u8; 32],
            slot: 1234,
            trace_uri: "ar://abcdef123456".to_string(),
        }
    }

    #[test]
    fn ignores_lines_without_program_data_prefix() {
        let res = parse_anchor_event_line("Program log: hi").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn decodes_thought_submitted_event() {
        let ev = fixture_event();
        let raw = synth_payload(&ev);
        let b64 = B64.encode(&raw);
        let line = format!("Program data: {}", b64);
        let parsed = parse_anchor_event_line(&line).unwrap().unwrap();
        assert_eq!(parsed, ev);
    }

    #[test]
    fn ignores_other_event_discriminators() {
        let mut raw = vec![0xAA; 8];
        raw.extend_from_slice(&[0u8; 16]);
        let res = parse_anchor_event_bytes(&raw).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn errors_on_short_payload() {
        let raw = vec![0u8; 4];
        assert!(parse_anchor_event_bytes(&raw).is_err());
    }
}
