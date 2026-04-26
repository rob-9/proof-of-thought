//! Strict regime (spec §6.1): byte-compare a re-executed output, or detect
//! commitment-trace inconsistencies up front.
//!
//! Two pieces:
//!
//! 1. [`ByteCompareVerifier`] — cheap, always-on. Hashes the trace's
//!    `canonical_output` bytes and compares to the on-chain
//!    `output_commitment`. Any mismatch is real, working evidence of fraud
//!    (the agent committed to an output that doesn't match what they posted
//!    to storage). Catches the lazy-fraud cases without any inference.
//!
//! 2. [`StrictVerifier`] (composes ByteCompare + a [`LocalInferenceEngine`]) —
//!    if commitments are consistent, asks the engine to actually re-execute
//!    the model on the canonical input. The MVP ships only [`NoopEngine`]
//!    which returns Inconclusive; real vLLM / llama.cpp integration is
//!    future work.
//!
//! TODO(re-exec): wire a real engine. See `docs/future-work.md`.
//! Suggested approach: spawn a sidecar `vllm` server, hit it via OpenAI-compatible
//! HTTP, set `temperature=0`, `seed=derive(vrf_seed)`, byte-compare the
//! resulting `canonical_output`. The engine must be pinned per-policy
//! (kernel selection differs across SM versions).

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::trace_fetch::TraceBundle;
use crate::types::{ChallengeClaim, ThoughtSubmittedEvent};

use super::{VerifyOutcome, Verifier};

/// Hash the trace bytes and compare against on-chain commitment.
///
/// This is real, working fraud detection: an agent that commits to
/// `output_commitment = H(A)` but uploads bytes `B ≠ A` to Arweave is caught
/// here, by *anyone*, with no model dependency.
pub struct ByteCompareVerifier;

impl ByteCompareVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ByteCompareVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verifier for ByteCompareVerifier {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome {
        let actual: [u8; 32] = blake3::hash(&bundle.raw_canonical_output).into();
        if actual == event.output_commitment {
            debug!(thought_pda = %event.thought_pda, "output_commitment matches trace bytes");
            VerifyOutcome::Match
        } else {
            warn!(
                thought_pda = %event.thought_pda,
                expected = ?event.output_commitment,
                actual = ?actual,
                "INCONSISTENT COMMITMENTS — agent committed to an output that doesn't match the trace"
            );
            VerifyOutcome::Mismatch {
                claim: ChallengeClaim::InconsistentCommitments,
                evidence_uri: event.trace_uri.clone(),
            }
        }
    }
}

/// Pluggable model-execution backend. The strict regime uses this to re-run
/// the agent's claimed model on the canonical input and compare outputs.
#[async_trait]
pub trait LocalInferenceEngine: Send + Sync {
    /// Re-run the agent's model and return canonical_output bytes.
    /// Implementations MUST canonicalize identically to the agent's SDK.
    async fn execute(
        &self,
        model_id: &[u8; 32],
        canonical_input: &[u8],
        vrf_seed: &[u8; 32],
    ) -> EngineResult;
}

#[derive(Debug, Clone)]
pub enum EngineResult {
    Output(Vec<u8>),
    Unavailable { reason: String },
}

/// Default no-op engine — re-execution disabled in MVP. Returns `Unavailable`
/// so the StrictVerifier emits `Inconclusive` rather than a false positive.
pub struct NoopEngine;

#[async_trait]
impl LocalInferenceEngine for NoopEngine {
    async fn execute(
        &self,
        _model_id: &[u8; 32],
        _canonical_input: &[u8],
        _vrf_seed: &[u8; 32],
    ) -> EngineResult {
        EngineResult::Unavailable {
            reason: "re-exec disabled in MVP — see docs/future-work.md".to_string(),
        }
    }
}

/// Composition: byte-compare gate + optional re-execution.
pub struct StrictVerifier<E: LocalInferenceEngine> {
    pub byte_compare: ByteCompareVerifier,
    pub engine: E,
}

impl<E: LocalInferenceEngine> StrictVerifier<E> {
    pub fn new(engine: E) -> Self {
        Self {
            byte_compare: ByteCompareVerifier::new(),
            engine,
        }
    }
}

#[async_trait]
impl<E: LocalInferenceEngine> Verifier for StrictVerifier<E> {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome {
        // 1. Cheap byte-compare gate. Any mismatch here is *real* fraud.
        match self.byte_compare.verify(event, bundle).await {
            VerifyOutcome::Mismatch { claim, evidence_uri } => {
                return VerifyOutcome::Mismatch { claim, evidence_uri };
            }
            VerifyOutcome::Inconclusive { reason } => {
                return VerifyOutcome::Inconclusive { reason };
            }
            VerifyOutcome::Match => {} // proceed to re-exec
        }

        // 2. Re-execute (engine-backed; NoopEngine in MVP).
        let result = self
            .engine
            .execute(&event.model_id, &bundle.raw_canonical_input, &event.vrf_seed)
            .await;

        match result {
            EngineResult::Unavailable { reason } => {
                info!(
                    thought_pda = %event.thought_pda,
                    %reason,
                    "strict re-exec unavailable; commitments consistent so leaving Inconclusive"
                );
                VerifyOutcome::Inconclusive { reason }
            }
            EngineResult::Output(bytes) => {
                let actual: [u8; 32] = blake3::hash(&bytes).into();
                if actual == event.output_commitment {
                    VerifyOutcome::Match
                } else {
                    warn!(
                        thought_pda = %event.thought_pda,
                        "re-exec produced different bytes than committed output"
                    );
                    VerifyOutcome::Mismatch {
                        claim: ChallengeClaim::OutputMismatch,
                        evidence_uri: event.trace_uri.clone(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    fn fixture_event(output_bytes: &[u8]) -> ThoughtSubmittedEvent {
        let output_commitment: [u8; 32] = blake3::hash(output_bytes).into();
        ThoughtSubmittedEvent {
            agent: Pubkey::new_unique(),
            thought_pda: Pubkey::new_unique(),
            model_id: [1u8; 32],
            input_commitment: [2u8; 32],
            output_commitment,
            trace_uri_hash: [4u8; 32],
            vrf_seed: [5u8; 32],
            policy_id: [6u8; 32],
            slot: 1,
            trace_uri: "ar://x".into(),
        }
    }

    fn fixture_bundle(output_bytes: &[u8]) -> TraceBundle {
        TraceBundle {
            raw_canonical_input: vec![0xab],
            raw_canonical_output: output_bytes.to_vec(),
            attestation: None,
            manifest: ciborium::Value::Null,
        }
    }

    #[tokio::test]
    async fn matching_commitments_emit_match() {
        let bytes = b"hello-canonical-output".to_vec();
        let event = fixture_event(&bytes);
        let bundle = fixture_bundle(&bytes);
        let v = ByteCompareVerifier::new();
        assert!(matches!(v.verify(&event, &bundle).await, VerifyOutcome::Match));
    }

    #[tokio::test]
    async fn mismatched_commitments_emit_mismatch() {
        let event = fixture_event(b"committed-bytes");
        let bundle = fixture_bundle(b"different-bytes-on-arweave");
        let v = ByteCompareVerifier::new();
        match v.verify(&event, &bundle).await {
            VerifyOutcome::Mismatch { claim, .. } => {
                assert_eq!(claim, ChallengeClaim::InconsistentCommitments);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn strict_falls_back_to_inconclusive_with_noop_engine() {
        let bytes = b"x".to_vec();
        let event = fixture_event(&bytes);
        let bundle = fixture_bundle(&bytes);
        let v = StrictVerifier::new(NoopEngine);
        match v.verify(&event, &bundle).await {
            VerifyOutcome::Inconclusive { .. } => {}
            other => panic!("expected Inconclusive, got {:?}", other),
        }
    }
}
