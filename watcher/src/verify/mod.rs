//! Verification regimes (spec §6).
//!
//! - `strict`:   re-exec / byte-compare
//! - `attested`: parse + validate Intel TDX TEE quote
//! - `soft`:     equivalence-class checks (StructuralJSON, SemanticCommittee, AnyOfN)
//!
//! All regimes share the [`Verifier`] trait so the pipeline can dispatch on
//! policy.

pub mod attested;
pub mod soft;
pub mod strict;

use async_trait::async_trait;

use crate::trace_fetch::TraceBundle;
use crate::types::{ChallengeClaim, ThoughtSubmittedEvent};

/// Outcome of a single verification attempt.
#[derive(Debug, Clone)]
pub enum VerifyOutcome {
    /// Verifier confirmed the trace matches the agent's claim.
    Match,
    /// Verifier proved a fraud — file a challenge with this claim + evidence.
    Mismatch {
        claim: ChallengeClaim,
        evidence_uri: String,
    },
    /// Verifier could not prove either way — leave to other watchers.
    Inconclusive { reason: String },
}

#[async_trait]
pub trait Verifier: Send + Sync {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome;
}

pub use attested::AttestedVerifier;
pub use soft::SoftVerifier;
pub use strict::{ByteCompareVerifier, LocalInferenceEngine, NoopEngine, StrictVerifier};
