//! Fetch and validate off-chain trace bundles (spec §4.5).
//!
//! Watchers download `trace_<commit_id>.tar.zst` from Arweave or Shadow Drive
//! at the URI logged in the `ThoughtSubmitted` event, then verify
//! `blake3(uri) == trace_uri_hash`. Mismatch ⇒ trace was swapped after
//! commit ⇒ challengeable directly.
//!
//! The verifier downstream needs both:
//! - structured manifest fields (`canonical_input.cbor`, `canonical_output.cbor`)
//! - raw bytes of `canonical_output.cbor` for byte-compare verification
//!
//! For MVP we accept a *flat CBOR manifest* from the URI body — i.e. the agent
//! posts a single CBOR document with the manifest fields inline rather than a
//! tarball. Tarball support is future work. The trait is shaped so a richer
//! parser slots in without changing call sites.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("http error: {0}")]
    Http(String),
    #[error("malformed CBOR manifest: {0}")]
    BadManifest(&'static str),
    #[error("trace_uri_hash mismatch: expected {expected:?}, got {actual:?}")]
    UriHashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    #[error("uri scheme not supported by this fetcher: {0}")]
    UnsupportedScheme(String),
    #[error("trace not found at {0}")]
    NotFound(String),
}

/// In-memory representation of a trace bundle.
///
/// `raw_canonical_output` is the *exact bytes* the agent canonicalized — the
/// strict-regime byte-compare verifier hashes these. `manifest` is the parsed
/// CBOR for ergonomic field access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceBundle {
    /// Bytes that were committed to via `output_commitment = blake3(canonical_output)`.
    #[serde(with = "serde_bytes")]
    pub raw_canonical_output: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub raw_canonical_input: Vec<u8>,
    /// Optional: raw TEE attestation quote bytes, present only for the
    /// hardware-attested regime.
    #[serde(default, with = "serde_bytes_opt")]
    pub attestation: Option<Vec<u8>>,
    /// Decoded canonical_output as a generic CBOR map for soft-equiv access.
    pub manifest: ciborium::Value,
}

mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let v: serde_bytes::ByteBuf = Deserialize::deserialize(d)?;
        Ok(v.into_vec())
    }
}

mod serde_bytes_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(b) => s.serialize_bytes(b),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let v: Option<serde_bytes::ByteBuf> = Deserialize::deserialize(d)?;
        Ok(v.map(|b| b.into_vec()))
    }
}

#[async_trait]
pub trait TraceFetcher: Send + Sync {
    async fn fetch(&self, uri: &str, expected_uri_hash: [u8; 32]) -> Result<TraceBundle, FetchError>;
}

/// Verify that `blake3(uri) == expected`.
pub fn verify_uri_hash(uri: &str, expected: [u8; 32]) -> Result<(), FetchError> {
    let actual: [u8; 32] = blake3::hash(uri.as_bytes()).into();
    if actual != expected {
        return Err(FetchError::UriHashMismatch {
            expected,
            actual,
        });
    }
    Ok(())
}

/// Decode a CBOR-encoded manifest blob into a [`TraceBundle`].
///
/// Manifest schema (spec §4.5, MVP flat form):
/// ```text
/// {
///   "canonical_input":  bytes,
///   "canonical_output": bytes,
///   "attestation":      bytes | null,    // optional
/// }
/// ```
pub fn decode_manifest(bytes: &[u8]) -> Result<TraceBundle, FetchError> {
    let value: ciborium::Value =
        ciborium::from_reader(bytes).map_err(|_| FetchError::BadManifest("cbor decode"))?;
    let map = value
        .as_map()
        .ok_or(FetchError::BadManifest("manifest not a map"))?;

    let mut fields: HashMap<String, ciborium::Value> = HashMap::new();
    for (k, v) in map.iter() {
        let key = k
            .as_text()
            .ok_or(FetchError::BadManifest("non-string key"))?
            .to_string();
        fields.insert(key, v.clone());
    }

    let raw_canonical_input = fields
        .get("canonical_input")
        .and_then(|v| v.as_bytes())
        .ok_or(FetchError::BadManifest("missing canonical_input"))?
        .clone();

    let raw_canonical_output = fields
        .get("canonical_output")
        .and_then(|v| v.as_bytes())
        .ok_or(FetchError::BadManifest("missing canonical_output"))?
        .clone();

    let attestation = fields
        .get("attestation")
        .and_then(|v| v.as_bytes())
        .cloned();

    // Decode canonical_output as CBOR for downstream soft-equiv inspection.
    // If decoding fails we still return the raw bytes — strict verifier only
    // needs the bytes — but soft verifier will produce Inconclusive.
    let manifest = ciborium::from_reader(raw_canonical_output.as_slice())
        .unwrap_or(ciborium::Value::Null);

    Ok(TraceBundle {
        raw_canonical_input,
        raw_canonical_output,
        attestation,
        manifest,
    })
}

// ---------------------------------------------------------------------------
// Arweave (HTTPS gateway)
// ---------------------------------------------------------------------------

pub struct ArweaveFetcher {
    client: reqwest::Client,
    /// Configurable so tests can point at a local gateway. Defaults to the
    /// public Arweave HTTPS gateway.
    pub gateway: String,
}

impl ArweaveFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            gateway: "https://arweave.net".to_string(),
        }
    }

    pub fn with_gateway(gateway: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            gateway: gateway.into(),
        }
    }
}

impl Default for ArweaveFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraceFetcher for ArweaveFetcher {
    async fn fetch(&self, uri: &str, expected_uri_hash: [u8; 32]) -> Result<TraceBundle, FetchError> {
        verify_uri_hash(uri, expected_uri_hash)?;

        // Accept either `ar://<txid>` or a fully-qualified arweave URL.
        let txid = uri
            .strip_prefix("ar://")
            .or_else(|| uri.strip_prefix("arweave://"))
            .ok_or_else(|| FetchError::UnsupportedScheme(uri.to_string()))?;

        let url = format!("{}/{}", self.gateway.trim_end_matches('/'), txid);
        debug!(%url, "fetching trace from arweave");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;

        if resp.status().as_u16() == 404 {
            return Err(FetchError::NotFound(url));
        }
        if !resp.status().is_success() {
            return Err(FetchError::Http(format!("HTTP {}", resp.status())));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;
        decode_manifest(&bytes)
    }
}

// ---------------------------------------------------------------------------
// Shadow Drive
// ---------------------------------------------------------------------------

/// Shadow Drive HTTPS fetcher.
///
/// TODO(shadow-drive): the canonical Shadow Drive URL pattern is
/// `https://shdw-drive.genesysgo.net/<storage-account>/<filename>`. We accept
/// either `shdw://<account>/<file>` or a fully-qualified URL here and
/// rewrite the former to the latter. The official URL pattern needs to be
/// confirmed against shadow-drive-cli before mainnet deploy.
pub struct ShadowFetcher {
    client: reqwest::Client,
    pub gateway: String,
}

impl ShadowFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            gateway: "https://shdw-drive.genesysgo.net".to_string(),
        }
    }

    pub fn with_gateway(gateway: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            gateway: gateway.into(),
        }
    }
}

impl Default for ShadowFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraceFetcher for ShadowFetcher {
    async fn fetch(&self, uri: &str, expected_uri_hash: [u8; 32]) -> Result<TraceBundle, FetchError> {
        verify_uri_hash(uri, expected_uri_hash)?;

        let path = uri
            .strip_prefix("shdw://")
            .or_else(|| uri.strip_prefix("shadow://"))
            .ok_or_else(|| FetchError::UnsupportedScheme(uri.to_string()))?;

        let url = format!("{}/{}", self.gateway.trim_end_matches('/'), path);
        debug!(%url, "fetching trace from shadow drive (stub URL pattern)");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(FetchError::Http(format!("HTTP {}", resp.status())));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;
        decode_manifest(&bytes)
    }
}

// ---------------------------------------------------------------------------
// Mock fetcher
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct MockFetcher {
    pub bundles: Arc<std::sync::Mutex<HashMap<String, TraceBundle>>>,
}

impl MockFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, uri: &str, bundle: TraceBundle) {
        self.bundles
            .lock()
            .expect("MockFetcher mutex poisoned")
            .insert(uri.to_string(), bundle);
    }
}

#[async_trait]
impl TraceFetcher for MockFetcher {
    async fn fetch(&self, uri: &str, expected_uri_hash: [u8; 32]) -> Result<TraceBundle, FetchError> {
        verify_uri_hash(uri, expected_uri_hash)?;
        let g = self
            .bundles
            .lock()
            .expect("MockFetcher mutex poisoned");
        g.get(uri)
            .cloned()
            .ok_or_else(|| FetchError::NotFound(uri.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_manifest(input: &[u8], output: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        let value = ciborium::Value::Map(vec![
            (
                ciborium::Value::Text("canonical_input".into()),
                ciborium::Value::Bytes(input.to_vec()),
            ),
            (
                ciborium::Value::Text("canonical_output".into()),
                ciborium::Value::Bytes(output.to_vec()),
            ),
        ]);
        ciborium::into_writer(&value, &mut buf).unwrap();
        buf
    }

    #[test]
    fn decode_manifest_roundtrip() {
        let input = b"input-bytes";
        let output = b"output-bytes";
        let raw = build_manifest(input, output);
        let bundle = decode_manifest(&raw).unwrap();
        assert_eq!(bundle.raw_canonical_input, input);
        assert_eq!(bundle.raw_canonical_output, output);
        assert!(bundle.attestation.is_none());
    }

    #[test]
    fn uri_hash_mismatch_rejected() {
        let bad = [0u8; 32];
        let r = verify_uri_hash("ar://abc", bad);
        assert!(matches!(r, Err(FetchError::UriHashMismatch { .. })));
    }

    #[test]
    fn uri_hash_match_accepted() {
        let uri = "ar://abc";
        let h: [u8; 32] = blake3::hash(uri.as_bytes()).into();
        verify_uri_hash(uri, h).unwrap();
    }

    #[tokio::test]
    async fn mock_fetcher_returns_inserted_bundle() {
        let f = MockFetcher::new();
        let uri = "ar://abc";
        let h: [u8; 32] = blake3::hash(uri.as_bytes()).into();
        let raw = build_manifest(b"i", b"o");
        let bundle = decode_manifest(&raw).unwrap();
        f.insert(uri, bundle.clone());
        let got = f.fetch(uri, h).await.unwrap();
        assert_eq!(got.raw_canonical_output, bundle.raw_canonical_output);
    }
}
