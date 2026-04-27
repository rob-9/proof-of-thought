use anchor_lang::prelude::*;

/// VRF request bound to a single (agent, nonce_idx) pair.
/// PDA: ["vrf", agent, nonce_idx_le_bytes].
///
/// In production this is populated by Pyth Entropy via callback. For the MVP
/// we let the agent supply a seed in `request_vrf`; the watcher network
/// validates that the seed actually came from a registered randomness source
/// post-hoc (the watcher subscribes to Pyth Entropy events and challenges
/// any `submit_thought` whose `vrf_seed` is not present in the on-chain Pyth
/// stream — see docs/future-work.md).
///
/// TODO(pyth-entropy): replace `request_vrf` with a callback-based flow:
///   `request_vrf` issues a CPI to `pyth_entropy::request`, and the seed is
///   filled in via a `fulfill_vrf` callback. See docs/future-work.md.
#[account]
pub struct VrfRequest {
    pub agent: Pubkey,
    pub nonce_idx: u64,
    pub seed: [u8; 32],
    pub request_slot: u64,
    pub consumed: bool,
    pub bump: u8,
}

impl VrfRequest {
    // disc + 32 + 8 + 32 + 8 + 1 + 1 = 90
    pub const LEN: usize = 8 + 32 + 8 + 32 + 8 + 1 + 1;
}
