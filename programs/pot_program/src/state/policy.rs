use anchor_lang::prelude::*;

/// Equivalence classes from spec §6.3 — how watchers compare re-executed
/// outputs against committed outputs.
#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EquivClass {
    /// Byte-equal canonical_output after re-exec. Open-weights only.
    Strict = 0,
    /// Only the `decision` field must match. Reasoning may diverge.
    StructuralJSON = 1,
    /// k-of-n verifier model committee decides equivalence.
    SemanticCommittee = 2,
    /// Agent commits Merkle root of N samples; reveals one. Watcher checks
    /// non-membership.
    AnyOfN = 3,
}

/// Policy account. PDA: ["policy", policy_id].
///
/// Permissionless to register; consumers choose which policies they trust.
/// `allowed_models` is bounded — at most `MAX_ALLOWED_MODELS` entries.
#[account]
pub struct Policy {
    pub policy_id: [u8; 32],
    pub schema_uri_hash: [u8; 32],
    pub equiv_class: u8, // EquivClass
    /// Cap on inference latency converted to slots — VRF freshness check uses
    /// this directly.
    pub max_inference_slots: u32,
    /// Slots a thought stays valid for downstream `consume_thought`.
    pub max_action_age_slots: u64,
    /// Window during which a watcher may file a challenge.
    pub challenge_window_slots: u64,
    /// Minimum stake an agent must hold AND minimum challenger bond.
    pub bond_min: u64,
    /// Authorized resolver for challenged thoughts. Single key for MVP.
    /// TODO(decentralized-dispute): replace with multisig / committee vote
    ///   (see docs/future-work.md).
    pub resolver: Pubkey,
    /// Treasury account that receives the protocol cut on slash/finalize.
    pub treasury: Pubkey,
    /// Whitelist of model_ids accepted under this policy. Bounded to keep
    /// the account size deterministic.
    pub allowed_models: Vec<[u8; 32]>,
    pub bump: u8,
}

impl Policy {
    pub const MAX_ALLOWED_MODELS: usize = 16;
    // disc + 32 + 32 + 1 + 4 + 8 + 8 + 8 + 32 + 32 + (4 + 16*32) + 1
    pub const LEN: usize =
        8 + 32 + 32 + 1 + 4 + 8 + 8 + 8 + 32 + 32 + (4 + Self::MAX_ALLOWED_MODELS * 32) + 1;
}
