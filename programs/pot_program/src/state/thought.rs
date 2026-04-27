use anchor_lang::prelude::*;

/// Lifecycle states for a ThoughtRecord. Stored as `u8` so the on-chain
/// layout matches the spec table verbatim.
#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Copy, Clone, Debug)]
pub enum ThoughtStatus {
    Pending = 0,
    Challenged = 1,
    Finalized = 2,
    Slashed = 3,
}

impl ThoughtStatus {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Pending),
            1 => Some(Self::Challenged),
            2 => Some(Self::Finalized),
            3 => Some(Self::Slashed),
            _ => None,
        }
    }
}

/// Arguments accepted by `submit_thought`. Mirrors the on-chain layout 1:1
/// minus the slot, status, and challenge_deadline fields the program fills in.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ThoughtRecordArgs {
    pub model_id: [u8; 32],
    pub input_commitment: [u8; 32],
    pub output_commitment: [u8; 32],
    pub trace_uri_hash: [u8; 32],
    pub vrf_seed: [u8; 32],
    pub policy_id: [u8; 32],
    pub action_pda: Pubkey,
    /// Index used to derive the VrfRequest PDA: ["vrf", agent, nonce_idx_le].
    pub vrf_nonce_idx: u64,
}

/// On-chain thought commitment. PDA: ["thought", agent, vrf_nonce_idx_le].
///
/// Spec §4.1 calls for 264 bytes total (incl. 8-byte discriminator). We keep
/// the same field set but add `challenge_deadline_slot`, `attestation_verified`,
/// `consumed_count`, and `bump` — required to wire the lifecycle correctly —
/// and grow the account to fit. Rationale lives in the constant below.
#[account]
pub struct ThoughtRecord {
    pub agent: Pubkey,                 // 32
    pub model_id: [u8; 32],            // 32
    pub input_commitment: [u8; 32],    // 32
    pub output_commitment: [u8; 32],   // 32
    pub trace_uri_hash: [u8; 32],      // 32
    pub vrf_seed: [u8; 32],            // 32
    pub policy_id: [u8; 32],           // 32
    pub slot: u64,                     // 8
    pub action_pda: Pubkey,            // 32
    pub status: u8,                    // 1
    pub attestation_verified: bool,    // 1
    pub challenge_deadline_slot: u64,  // 8
    pub consumed_count: u32,           // 4
    pub vrf_nonce_idx: u64,            // 8
    pub bump: u8,                      // 1
    pub _pad: [u8; 7],                 // 7  alignment to 8-byte boundary
}

impl ThoughtRecord {
    // disc(8) + 32*7 + 8 + 32 + 1 + 1 + 8 + 4 + 8 + 1 + 7 = 8 + 224 + 70 = 302
    // Spec target was 264; the additions (lifecycle bookkeeping) push us to 302.
    // Documented deviation — see report.
    pub const LEN: usize =
        8 + 32 + 32 + 32 + 32 + 32 + 32 + 32 + 8 + 32 + 1 + 1 + 8 + 4 + 8 + 1 + 7;
}

/// Event emitted on successful `submit_thought`. trace_uri lives here, not
/// in the account, to keep account size bounded.
///
/// **Canonical wire ordering — DO NOT REORDER.** The watcher
/// (`watcher/src/types.rs::ThoughtSubmittedEvent`) decodes this event with
/// the field order below. Anchor serializes events in declaration order,
/// so any change here breaks every deployed watcher.
///
/// We include the commitments inline (rather than forcing watchers to fetch
/// the ThoughtRecord account) so the verifier pipeline can run with
/// websocket-only access — saves an RPC round-trip per event under load.
#[event]
pub struct ThoughtSubmitted {
    pub agent: Pubkey,
    pub thought_pda: Pubkey,
    pub model_id: [u8; 32],
    pub input_commitment: [u8; 32],
    pub output_commitment: [u8; 32],
    pub trace_uri_hash: [u8; 32],
    pub vrf_seed: [u8; 32],
    pub policy_id: [u8; 32],
    pub slot: u64,
    pub trace_uri: String,
}

#[event]
pub struct ThoughtFinalized {
    pub thought_pda: Pubkey,
    pub agent: Pubkey,
    pub status: u8,
    pub slot: u64,
}

#[event]
pub struct ThoughtConsumed {
    pub thought_pda: Pubkey,
    pub action_pda: Pubkey,
    pub consumer: Pubkey,
    pub consumed_count: u32,
}
