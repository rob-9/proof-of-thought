use anchor_lang::prelude::*;

use crate::errors::PotError;
use crate::state::{Policy, ThoughtConsumed, ThoughtRecord, ThoughtStatus};

#[derive(Accounts)]
pub struct ConsumeThought<'info> {
    /// Caller. Typically the consumer program signs as PDA via CPI.
    pub consumer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"thought", thought.agent.as_ref(), &thought.vrf_nonce_idx.to_le_bytes()],
        bump = thought.bump,
    )]
    pub thought: Account<'info, ThoughtRecord>,

    #[account(
        seeds = [b"policy", thought.policy_id.as_ref()],
        bump = policy.bump,
        constraint = policy.policy_id == thought.policy_id @ PotError::WrongPolicy,
    )]
    pub policy: Account<'info, Policy>,

    /// CHECK: opaque action PDA — we only check key equality against the
    /// thought commitment.
    pub action: UncheckedAccount<'info>,
}

/// Gate a downstream action on a finalized thought.
///
/// Designed to be invoked via CPI: a consumer program forwards its action
/// PDA, and we verify (a) the thought commits to that exact action, (b) the
/// thought is finalized or attestation-verified, and (c) the thought is fresh
/// per policy.max_action_age_slots.
pub fn consume_thought_handler(ctx: Context<ConsumeThought>) -> Result<()> {
    let thought = &mut ctx.accounts.thought;
    let policy = &ctx.accounts.policy;
    let clock = Clock::get()?;

    require!(
        thought.action_pda == ctx.accounts.action.key(),
        PotError::WrongAction
    );

    let status = ThoughtStatus::from_u8(thought.status).ok_or(PotError::InvalidStatus)?;
    require!(
        status == ThoughtStatus::Finalized || thought.attestation_verified,
        PotError::ThoughtNotReady
    );

    let age = clock
        .slot
        .checked_sub(thought.slot)
        .ok_or(PotError::Overflow)?;
    require!(age <= policy.max_action_age_slots, PotError::ThoughtStale);

    thought.consumed_count = thought
        .consumed_count
        .checked_add(1)
        .ok_or(PotError::Overflow)?;

    emit!(ThoughtConsumed {
        thought_pda: thought.key(),
        action_pda: thought.action_pda,
        consumer: ctx.accounts.consumer.key(),
        consumed_count: thought.consumed_count,
    });

    Ok(())
}
