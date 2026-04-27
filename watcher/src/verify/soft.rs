//! Soft equivalence regime (spec §6.3).
//!
//! Three sub-modes:
//! - `StructuralJsonEquiv`: compare only the `decision` field of canonical_output (working).
//! - `SemanticCommitteeEquiv`: ask k committee LLMs whether two outputs entail the same downstream action (stub).
//! - `AnyOfNEquiv`: Merkle non-membership over a committed set of N samples (skeleton).

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::trace_fetch::TraceBundle;
use crate::types::{ChallengeClaim, ThoughtSubmittedEvent};

use super::{VerifyOutcome, Verifier};

/// `StructuralJsonEquiv`: extracts `decision` from the on-chain trace's
/// canonical_output AND from any reference (re-execution) the watcher has,
/// and demands exact equality on the decision alone. Reasoning text may
/// differ.
///
/// In the watcher's role here we do NOT have a re-execution to compare
/// against (committee-based regimes need an engine). What we *can* do is
/// validate the decision is well-formed JSON and matches the policy schema.
/// Anything more requires a reference output or a committee. This MVP
/// returns Inconclusive when no reference is available.
pub struct StructuralJsonEquiv {
    /// If supplied, this is a reference canonical_output (e.g. from a re-run
    /// or a known-good output for this input). The verifier compares
    /// `decision` fields. Wired in by tests; production wiring is future
    /// work.
    pub reference_decision: Option<ciborium::Value>,
}

impl StructuralJsonEquiv {
    pub fn new() -> Self {
        Self {
            reference_decision: None,
        }
    }

    pub fn with_reference(decision: ciborium::Value) -> Self {
        Self {
            reference_decision: Some(decision),
        }
    }

    /// Pull the `decision` field out of a canonical_output CBOR map.
    pub fn extract_decision(manifest: &ciborium::Value) -> Option<ciborium::Value> {
        manifest.as_map().and_then(|map| {
            map.iter()
                .find(|(k, _)| k.as_text() == Some("decision"))
                .map(|(_, v)| v.clone())
        })
    }
}

impl Default for StructuralJsonEquiv {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verifier for StructuralJsonEquiv {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome {
        let Some(decision) = Self::extract_decision(&bundle.manifest) else {
            return VerifyOutcome::Mismatch {
                claim: ChallengeClaim::OutputMismatch,
                evidence_uri: event.trace_uri.clone(),
            };
        };
        let Some(reference) = self.reference_decision.as_ref() else {
            return VerifyOutcome::Inconclusive {
                reason: "no reference decision available — committee regime required".to_string(),
            };
        };
        if &decision == reference {
            VerifyOutcome::Match
        } else {
            warn!(
                thought_pda = %event.thought_pda,
                "decision diverges from reference under StructuralJsonEquiv"
            );
            VerifyOutcome::Mismatch {
                claim: ChallengeClaim::OutputMismatch,
                evidence_uri: event.trace_uri.clone(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Semantic committee
// ---------------------------------------------------------------------------

/// One member of a SemanticCommittee — a small open-weights model.
#[async_trait]
pub trait CommitteeMember: Send + Sync {
    async fn entails_same_action(
        &self,
        output_a: &ciborium::Value,
        output_b: &ciborium::Value,
    ) -> CommitteeVote;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitteeVote {
    Same,
    Different,
    Unsure,
}

/// Stub committee member that can't actually run a model. Real members will
/// run e.g. `gpt-4o-mini` / `claude-haiku-4.5` / `llama-3.3-70b-instruct` —
/// see spec §6.3.
pub struct NoopMember;

#[async_trait]
impl CommitteeMember for NoopMember {
    async fn entails_same_action(
        &self,
        _output_a: &ciborium::Value,
        _output_b: &ciborium::Value,
    ) -> CommitteeVote {
        // We deliberately don't fake a vote here.
        CommitteeVote::Unsure
    }
}

pub struct SemanticCommitteeEquiv {
    pub members: Vec<Box<dyn CommitteeMember>>,
    pub quorum: usize,
}

impl SemanticCommitteeEquiv {
    pub fn new(members: Vec<Box<dyn CommitteeMember>>, quorum: usize) -> Self {
        Self { members, quorum }
    }
}

#[async_trait]
impl Verifier for SemanticCommitteeEquiv {
    async fn verify(
        &self,
        _event: &ThoughtSubmittedEvent,
        _bundle: &TraceBundle,
    ) -> VerifyOutcome {
        if self.members.is_empty() {
            return VerifyOutcome::Inconclusive {
                reason: "empty committee".into(),
            };
        }
        // No reference to compare against in this skeleton — real
        // implementation needs a re-executed output_b. Pull from the
        // Strict/AnyOfN regimes.
        VerifyOutcome::Inconclusive {
            reason: "semantic committee re-execution not implemented (NoopMember)".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// AnyOfN
// ---------------------------------------------------------------------------

/// Skeleton for the AnyOfN regime. The agent commits a Merkle root over N
/// pre-computed samples (with declared seeds) in canonical_output and reveals
/// one. Watcher re-samples and checks that the revealed sample is one of the
/// N. Fraud is proved by a Merkle non-membership proof for the revealed
/// output.
///
/// Skeleton: implements only the structural plumbing. Real Merkle proof
/// verification depends on the on-chain commit format, which is currently
/// being designed in the program crate.
pub struct AnyOfNEquiv;

impl AnyOfNEquiv {
    pub fn new() -> Self {
        Self
    }

    /// Verify that `revealed_leaf_hash` is a member of the Merkle tree rooted
    /// at `claimed_root` using the supplied authentication path. Returns
    /// `true` iff the reveal is in the set. The caller derives `Mismatch`
    /// from a `false` result.
    pub fn merkle_member(
        revealed_leaf_hash: [u8; 32],
        proof: &[(bool, [u8; 32])],
        claimed_root: [u8; 32],
    ) -> bool {
        let mut acc = revealed_leaf_hash;
        for (is_right, sibling) in proof {
            let mut hasher = blake3::Hasher::new();
            if *is_right {
                hasher.update(&acc);
                hasher.update(sibling);
            } else {
                hasher.update(sibling);
                hasher.update(&acc);
            }
            acc = hasher.finalize().into();
        }
        acc == claimed_root
    }
}

impl Default for AnyOfNEquiv {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verifier for AnyOfNEquiv {
    async fn verify(
        &self,
        _event: &ThoughtSubmittedEvent,
        _bundle: &TraceBundle,
    ) -> VerifyOutcome {
        // Real impl: parse the AnyOfN sidecar from the trace, verify the
        // revealed seed produces the revealed output (re-exec), check Merkle
        // membership against the on-chain root.
        VerifyOutcome::Inconclusive {
            reason: "AnyOfN re-execution not yet implemented".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregator
// ---------------------------------------------------------------------------

/// Convenience composite: tries StructuralJSON first, then committee.
/// This is the entry point for the soft regime.
pub struct SoftVerifier {
    pub structural: StructuralJsonEquiv,
    pub committee: SemanticCommitteeEquiv,
}

impl SoftVerifier {
    pub fn new(structural: StructuralJsonEquiv, committee: SemanticCommitteeEquiv) -> Self {
        Self {
            structural,
            committee,
        }
    }
}

#[async_trait]
impl Verifier for SoftVerifier {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome {
        match self.structural.verify(event, bundle).await {
            VerifyOutcome::Match => VerifyOutcome::Match,
            VerifyOutcome::Mismatch { claim, evidence_uri } => {
                VerifyOutcome::Mismatch { claim, evidence_uri }
            }
            VerifyOutcome::Inconclusive { reason } => {
                debug!(%reason, "structural inconclusive; falling back to committee");
                self.committee.verify(event, bundle).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_member_roundtrip_single_sibling() {
        let leaf: [u8; 32] = blake3::hash(b"a").into();
        let sibling: [u8; 32] = blake3::hash(b"b").into();

        let mut hasher = blake3::Hasher::new();
        hasher.update(&leaf);
        hasher.update(&sibling);
        let root: [u8; 32] = hasher.finalize().into();

        assert!(AnyOfNEquiv::merkle_member(
            leaf,
            &[(true, sibling)], // we are the LEFT child, sibling on right
            root
        ));
        assert!(!AnyOfNEquiv::merkle_member(
            leaf,
            &[(false, sibling)],
            root
        ));
    }

    #[test]
    fn extracts_decision_field() {
        let v = ciborium::Value::Map(vec![
            (
                ciborium::Value::Text("decision".into()),
                ciborium::Value::Text("buy".into()),
            ),
            (
                ciborium::Value::Text("reasoning".into()),
                ciborium::Value::Text("because".into()),
            ),
        ]);
        let d = StructuralJsonEquiv::extract_decision(&v).unwrap();
        assert_eq!(d.as_text(), Some("buy"));
    }
}
