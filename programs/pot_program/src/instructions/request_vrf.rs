use anchor_lang::prelude::*;

use crate::errors::PotError;
use crate::state::{AgentProfile, VrfRequest};

#[derive(Accounts)]
#[instruction(nonce_idx: u64)]
pub struct RequestVrf<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"agent", operator.key().as_ref()],
        bump = agent.bump,
        has_one = operator @ PotError::WrongAgent,
    )]
    pub agent: Account<'info, AgentProfile>,

    #[account(
        init,
        payer = operator,
        space = VrfRequest::LEN,
        seeds = [b"vrf", agent.key().as_ref(), &nonce_idx.to_le_bytes()],
        bump
    )]
    pub vrf_request: Account<'info, VrfRequest>,

    pub system_program: Program<'info, System>,
}

/// Allocate a VrfRequest at (agent, nonce_idx) and seal it with `seed`.
///
/// MVP semantics: the agent supplies the seed. A watcher cross-checks the seed
/// against the on-chain Pyth Entropy stream and challenges if it doesn't
/// belong. This is honest about the trust shape — the canonical input
/// commitment binds the seed to the inference, so any mismatch is a
/// `StaleVRF` claim.
///
/// TODO(pyth-entropy): convert to a CPI request + callback once the Pyth
/// Entropy SDK is wired in. See docs/future-work.md (#pyth-entropy).
pub fn request_vrf_handler(ctx: Context<RequestVrf>, nonce_idx: u64, seed: [u8; 32]) -> Result<()> {
    // I1: pin nonce_idx to the agent's monotonic counter. Without this an
    // agent could pre-allocate VrfRequest PDAs at arbitrary indexes and
    // gain a chosen relationship between nonce_idx and future block hashes.
    require!(
        nonce_idx == ctx.accounts.agent.vrf_nonce,
        PotError::VRFAlreadyConsumed
    );

    let vrf = &mut ctx.accounts.vrf_request;
    vrf.agent = ctx.accounts.agent.key();
    vrf.nonce_idx = nonce_idx;
    vrf.seed = seed;
    vrf.request_slot = Clock::get()?.slot;
    vrf.consumed = false;
    vrf.bump = ctx.bumps.vrf_request;

    // Bump the monotonic nonce on the agent so SDK clients can default to
    // `agent.vrf_nonce` for the next request.
    let agent = &mut ctx.accounts.agent;
    agent.vrf_nonce = agent
        .vrf_nonce
        .checked_add(1)
        .ok_or(PotError::Overflow)?;

    Ok(())
}
