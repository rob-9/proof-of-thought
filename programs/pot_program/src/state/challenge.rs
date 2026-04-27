use anchor_lang::prelude::*;

/// Challenge claim categories from spec §5.1.
///
/// **Canonical wire ordering — DO NOT REORDER.** This enum is the source of
/// truth; the SDK (`sdk/ts/src/types.ts`) and watcher (`watcher/src/types.rs`)
/// must mirror these values. Cross-language conformance test pending — see
/// code review C3 / `docs/code-review/2026-04-26-initial-review.md`.
#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Copy, Clone, Debug)]
pub enum ChallengeClaim {
    ModelMismatch = 0,
    OutputMismatch = 1,
    InputOmission = 2,
    Replay = 3,
    StaleVRF = 4,
    AttestationInvalid = 5,
    /// Agent's claimed `output_commitment` doesn't match
    /// `blake3(canonical_output)` from the trace bundle. Catchable without
    /// re-execution; the byte-compare verifier is the canonical detector.
    InconsistentCommitments = 6,
}

/// Challenge account. PDA: ["challenge", thought_pda, challenger].
#[account]
pub struct Challenge {
    pub thought: Pubkey,
    pub challenger: Pubkey,
    pub bond: u64,
    pub claim: u8, // ChallengeClaim
    pub evidence_uri_hash: [u8; 32],
    pub opened_at_slot: u64,
    pub resolved: bool,
    /// Verdict written by `resolve` — only meaningful when `resolved == true`.
    /// `true` = challenger won (agent is guilty).
    pub verdict: bool,
    pub bump: u8,
}

impl Challenge {
    // disc + 32 + 32 + 8 + 1 + 32 + 8 + 1 + 1 + 1 = 124
    pub const LEN: usize = 8 + 32 + 32 + 8 + 1 + 32 + 8 + 1 + 1 + 1;
}

#[event]
pub struct ChallengeOpened {
    pub challenge: Pubkey,
    pub thought: Pubkey,
    pub challenger: Pubkey,
    pub claim: u8,
    pub bond: u64,
}

#[event]
pub struct ChallengeResolved {
    pub challenge: Pubkey,
    pub thought: Pubkey,
    pub verdict: bool,
    pub slashed_amount: u64,
}
