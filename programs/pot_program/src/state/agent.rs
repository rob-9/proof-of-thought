use anchor_lang::prelude::*;

/// Per-operator agent registration. PDA: ["agent", operator].
///
/// `stake_amount` is the lamport balance held in the vault PDA; we cache it
/// here so submit_thought can do a cheap stake-vs-bond check without loading
/// the vault account. The two MUST stay in sync — every code path that moves
/// lamports into/out of the vault also updates this field.
#[account]
pub struct AgentProfile {
    /// Solana key authorized to act for this agent. Signs every state-changing
    /// ix on the agent's behalf.
    pub operator: Pubkey,
    /// Vault PDA address, derived as ["vault", agent_profile].
    pub stake_vault: Pubkey,
    /// Lamports currently locked as stake.
    pub stake_amount: u64,
    /// Reputation tally — incremented on finalized thoughts, decremented on
    /// successful slashes against the agent. i64 so it can go negative.
    pub reputation: i64,
    /// Number of thoughts in Pending or Challenged status. Withdraw is blocked
    /// while this is non-zero.
    pub active_thoughts: u32,
    /// Slot until which withdrawals are blocked after a withdraw request.
    pub cooldown_until: u64,
    /// Monotonic nonce used by `request_vrf` to derive unique VrfRequest PDAs.
    pub vrf_nonce: u64,
    /// PDA bump for ["agent", operator].
    pub bump: u8,
}

impl AgentProfile {
    // disc + 32 + 32 + 8 + 8 + 4 + 8 + 8 + 1 = 109
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 4 + 8 + 8 + 1;
}
