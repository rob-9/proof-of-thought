//! Challenge filer: decide whether to submit an on-chain `challenge` ix and,
//! if so, build + sign + send it.
//!
//! The decision step is independent of submission so we can test EV math
//! without an RPC. Submission is behind a trait so the pipeline can be
//! exercised end-to-end with [`MockChallengeSubmitter`].
//!
//! TODO(post-merge): the on-chain ix layout (account list, args order) is
//! mirrored from spec §5.2 and the program agent's pre-merge code. Once
//! `feat/program` lands, generate `ChallengeArgs` from the IDL instead of
//! the hand-rolled struct here.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use thiserror::Error;
use tracing::{info, warn};

use crate::config::BondStrategy;
use crate::types::{ChallengeClaim, CHALLENGE_IX_DISCRIMINATOR};
use crate::verify::VerifyOutcome;

/// Estimated transaction fee for a challenge submission, in lamports.
///
/// The default fee on Solana is 5_000 lamports per signature, but compute
/// budget + priority fees push real cost higher under contention. We use
/// 5_000_000 (~0.005 SOL) as a conservative ceiling for the EV check;
/// this is intentionally pessimistic so we don't fire EV-marginal challenges
/// and lose money on fee swings. Watcher operators can tune via config.
pub const DEFAULT_TX_FEE_LAMPORTS: u64 = 5_000_000;

#[derive(Debug, Error)]
pub enum ChallengeError {
    #[error("rpc submission failed: {0}")]
    Submit(String),
    #[error("missing thought_pda for challenge")]
    MissingThoughtPda,
}

/// Args mirror the on-chain `challenge(claim, bond, evidence_uri_hash)` ix.
///
/// Wire format (Borsh): 8-byte discriminator || claim:u8 || bond:u64 (LE)
/// || evidence_uri_hash:[u8;32].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeArgs {
    pub thought_pda: Pubkey,
    pub claim: ChallengeClaim,
    pub bond: u64,
    pub evidence_uri_hash: [u8; 32],
}

impl ChallengeArgs {
    /// Serialize ix data exactly as the program expects.
    ///
    /// Layout: `discriminator(8) || claim(1) || bond(8 LE) || evidence(32)`.
    pub fn ix_data(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 1 + 8 + 32);
        out.extend_from_slice(&CHALLENGE_IX_DISCRIMINATOR);
        out.push(self.claim as u8);
        out.extend_from_slice(&self.bond.to_le_bytes());
        out.extend_from_slice(&self.evidence_uri_hash);
        out
    }
}

#[async_trait]
pub trait ChallengeSubmitter: Send + Sync {
    async fn submit(&self, args: ChallengeArgs) -> Result<Signature, ChallengeError>;
}

// ---------------------------------------------------------------------------
// Mock implementation for tests
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct MockChallengeSubmitter {
    inner: Arc<Mutex<Vec<ChallengeArgs>>>,
}

impl MockChallengeSubmitter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn submitted(&self) -> Vec<ChallengeArgs> {
        self.inner
            .lock()
            .expect("MockChallengeSubmitter mutex poisoned")
            .clone()
    }

    pub fn count(&self) -> usize {
        self.inner
            .lock()
            .expect("MockChallengeSubmitter mutex poisoned")
            .len()
    }
}

#[async_trait]
impl ChallengeSubmitter for MockChallengeSubmitter {
    async fn submit(&self, args: ChallengeArgs) -> Result<Signature, ChallengeError> {
        self.inner
            .lock()
            .expect("MockChallengeSubmitter mutex poisoned")
            .push(args);
        // Deterministic dummy signature for tests — count-encoded so each
        // submission has a distinct value.
        let count = self
            .inner
            .lock()
            .expect("MockChallengeSubmitter mutex poisoned")
            .len() as u64;
        let mut sig_bytes = [0u8; 64];
        sig_bytes[..8].copy_from_slice(&count.to_le_bytes());
        Ok(Signature::from(sig_bytes))
    }
}

// ---------------------------------------------------------------------------
// Real RPC implementation (stub — flesh out post-merge)
// ---------------------------------------------------------------------------

/// Real submitter that builds + signs + sends a Solana transaction.
///
/// Currently a stub that returns an error: building the tx requires the
/// canonical account list for the `challenge` ix, which is part of the
/// program agent's IDL output (not yet merged). Once the IDL is in tree,
/// fill in the AccountMeta list, recent_blockhash fetch, and rpc send.
pub struct RpcChallengeSubmitter {
    pub program_id: Pubkey,
    pub rpc_url: String,
    // TODO(post-merge): hold a Keypair, an RpcClient, and the policy account
    // pubkey for the resolver path. Sketched in tests/integration.rs.
}

#[async_trait]
impl ChallengeSubmitter for RpcChallengeSubmitter {
    async fn submit(&self, _args: ChallengeArgs) -> Result<Signature, ChallengeError> {
        // TODO(post-merge): construct Transaction with AccountMetas:
        //   [thought_pda (writable), challenger_signer, challenger_bond_vault,
        //    policy, system_program]
        // Use args.ix_data() for the instruction data.
        Err(ChallengeError::Submit(
            "RpcChallengeSubmitter is stubbed pending program IDL merge — \
             use MockChallengeSubmitter or wire the real Anchor IDL types"
                .into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Filer — decision + dispatch
// ---------------------------------------------------------------------------

/// Decision outcome — exposed for testing the EV math without going through
/// the submitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDecision {
    /// Verifier confirmed match — nothing to do.
    NoFraud,
    /// Verifier was inconclusive — leave to others.
    Inconclusive(String),
    /// Fraud detected but EV-negative — skip.
    EvNegative {
        expected_payout: u64,
        bond: u64,
        tx_fee: u64,
    },
    /// Filing was disabled by config.
    FilingDisabled,
    /// Fraud detected and EV-positive — file.
    File(ChallengeArgs),
}

pub struct ChallengeFiler<S: ChallengeSubmitter> {
    submitter: S,
    bond_strategy: BondStrategy,
    /// Per-watcher cap. Sum of (active bond) + (this challenge's bond) must
    /// not exceed this value, or the filer skips. Defends against a single
    /// dispute draining the watcher's wallet.
    pub max_stake_at_risk_lamports: u64,
    /// Estimated tx fee for the EV check. Defaults to
    /// [`DEFAULT_TX_FEE_LAMPORTS`].
    pub tx_fee_lamports: u64,
}

impl<S: ChallengeSubmitter> ChallengeFiler<S> {
    pub fn new(
        submitter: S,
        bond_strategy: BondStrategy,
        max_stake_at_risk_lamports: u64,
    ) -> Self {
        Self {
            submitter,
            bond_strategy,
            max_stake_at_risk_lamports,
            tx_fee_lamports: DEFAULT_TX_FEE_LAMPORTS,
        }
    }

    /// Compute the bond for this filing per the configured strategy.
    ///
    /// `policy_bond_min` is the policy-declared floor; aggressive strategies
    /// may bid higher to outpace competing watchers (priority fee analog).
    fn bond_for(&self, policy_bond_min: u64) -> u64 {
        match self.bond_strategy {
            BondStrategy::Disabled => policy_bond_min,
            BondStrategy::Conservative => policy_bond_min,
            BondStrategy::Aggressive => policy_bond_min.saturating_mul(2),
        }
    }

    /// Decide whether to file. Pure function (no I/O) — exposed for tests.
    pub fn decide(
        &self,
        outcome: &VerifyOutcome,
        agent_stake_lamports: u64,
        thought_pda: Pubkey,
        policy_bond_min: u64,
    ) -> FileDecision {
        if matches!(self.bond_strategy, BondStrategy::Disabled) {
            return FileDecision::FilingDisabled;
        }

        let (claim, evidence_uri) = match outcome {
            VerifyOutcome::Match => return FileDecision::NoFraud,
            VerifyOutcome::Inconclusive { reason } => {
                return FileDecision::Inconclusive(reason.clone())
            }
            VerifyOutcome::Mismatch {
                claim,
                evidence_uri,
            } => (*claim, evidence_uri.clone()),
        };

        let bond = self.bond_for(policy_bond_min);
        // Distribution from spec §5.4: 60% of slashed stake to challenger.
        let expected_payout = agent_stake_lamports.saturating_mul(6) / 10;
        let cost = bond.saturating_add(self.tx_fee_lamports);

        if expected_payout <= cost {
            return FileDecision::EvNegative {
                expected_payout,
                bond,
                tx_fee: self.tx_fee_lamports,
            };
        }

        if bond > self.max_stake_at_risk_lamports {
            return FileDecision::EvNegative {
                expected_payout,
                bond,
                tx_fee: self.tx_fee_lamports,
            };
        }

        let evidence_uri_hash = blake3::hash(evidence_uri.as_bytes()).into();

        FileDecision::File(ChallengeArgs {
            thought_pda,
            claim,
            bond,
            evidence_uri_hash,
        })
    }

    /// Decide and dispatch.
    pub async fn decide_and_file(
        &self,
        outcome: &VerifyOutcome,
        agent_stake_lamports: u64,
        thought_pda: Pubkey,
        policy_bond_min: u64,
    ) -> Result<Option<Signature>, ChallengeError> {
        match self.decide(outcome, agent_stake_lamports, thought_pda, policy_bond_min) {
            FileDecision::File(args) => {
                info!(
                    claim = ?args.claim,
                    bond = args.bond,
                    "filing challenge"
                );
                let sig = self.submitter.submit(args).await?;
                Ok(Some(sig))
            }
            FileDecision::EvNegative {
                expected_payout,
                bond,
                tx_fee,
            } => {
                warn!(expected_payout, bond, tx_fee, "skipping EV-negative challenge");
                Ok(None)
            }
            FileDecision::FilingDisabled => Ok(None),
            FileDecision::Inconclusive(reason) => {
                info!(%reason, "verifier inconclusive — leaving to other watchers");
                Ok(None)
            }
            FileDecision::NoFraud => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_pda() -> Pubkey {
        Pubkey::new_from_array([7u8; 32])
    }

    fn fraud_outcome() -> VerifyOutcome {
        VerifyOutcome::Mismatch {
            claim: ChallengeClaim::InconsistentCommitments,
            evidence_uri: "ar://evidence-1234".into(),
        }
    }

    #[test]
    fn ix_data_layout_is_stable() {
        let args = ChallengeArgs {
            thought_pda: dummy_pda(),
            claim: ChallengeClaim::OutputMismatch,
            bond: 1_000_000,
            evidence_uri_hash: [0xab; 32],
        };
        let data = args.ix_data();
        assert_eq!(data.len(), 8 + 1 + 8 + 32);
        assert_eq!(data[8], 1); // OutputMismatch = 1
        assert_eq!(&data[9..17], &1_000_000u64.to_le_bytes());
        assert_eq!(&data[17..49], &[0xab; 32]);
    }

    #[test]
    fn decide_files_when_ev_positive() {
        let f = ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            BondStrategy::Conservative,
            10 * solana_sdk::native_token::LAMPORTS_PER_SOL,
        );
        // 60% of 100 SOL stake = 60 SOL expected payout; bond 0.5 SOL +
        // tx fee 0.005 SOL ⇒ EV very positive.
        let outcome = f.decide(
            &fraud_outcome(),
            100 * solana_sdk::native_token::LAMPORTS_PER_SOL,
            dummy_pda(),
            500_000_000,
        );
        assert!(matches!(outcome, FileDecision::File(_)));
    }

    #[test]
    fn decide_skips_when_ev_negative() {
        let f = ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            BondStrategy::Conservative,
            10 * solana_sdk::native_token::LAMPORTS_PER_SOL,
        );
        // 60% of 0.001 SOL = 0.0006 SOL; bond 1 SOL ⇒ EV very negative.
        let outcome = f.decide(
            &fraud_outcome(),
            1_000_000,
            dummy_pda(),
            solana_sdk::native_token::LAMPORTS_PER_SOL,
        );
        assert!(matches!(outcome, FileDecision::EvNegative { .. }));
    }

    #[test]
    fn decide_respects_max_stake_at_risk() {
        let f = ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            BondStrategy::Conservative,
            100_000, // 0.0001 SOL cap
        );
        let outcome = f.decide(
            &fraud_outcome(),
            1_000 * solana_sdk::native_token::LAMPORTS_PER_SOL,
            dummy_pda(),
            500_000_000,
        );
        assert!(matches!(outcome, FileDecision::EvNegative { .. }));
    }

    #[test]
    fn decide_skips_when_disabled() {
        let f = ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            BondStrategy::Disabled,
            u64::MAX,
        );
        let outcome = f.decide(
            &fraud_outcome(),
            100 * solana_sdk::native_token::LAMPORTS_PER_SOL,
            dummy_pda(),
            500_000_000,
        );
        assert_eq!(outcome, FileDecision::FilingDisabled);
    }

    #[test]
    fn aggressive_strategy_doubles_bond() {
        let f = ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            BondStrategy::Aggressive,
            10 * solana_sdk::native_token::LAMPORTS_PER_SOL,
        );
        if let FileDecision::File(args) = f.decide(
            &fraud_outcome(),
            100 * solana_sdk::native_token::LAMPORTS_PER_SOL,
            dummy_pda(),
            500_000_000,
        ) {
            assert_eq!(args.bond, 1_000_000_000);
        } else {
            panic!("expected File");
        }
    }

    #[tokio::test]
    async fn mock_submitter_records_calls() {
        let sub = MockChallengeSubmitter::new();
        let f = ChallengeFiler::new(
            sub.clone(),
            BondStrategy::Conservative,
            10 * solana_sdk::native_token::LAMPORTS_PER_SOL,
        );
        let _ = f
            .decide_and_file(
                &fraud_outcome(),
                100 * solana_sdk::native_token::LAMPORTS_PER_SOL,
                dummy_pda(),
                500_000_000,
            )
            .await
            .unwrap();
        assert_eq!(sub.count(), 1);
    }
}
