use anchor_lang::prelude::*;

use crate::errors::PotError;
use crate::state::{
    AgentProfile, Challenge, ChallengeResolved, Policy, ThoughtFinalized, ThoughtRecord,
    ThoughtStatus,
};

/// Resolve an unchallenged thought past its deadline → Finalized.
///
/// Permissionless crank — anyone may call once the window has elapsed.
#[derive(Accounts)]
pub struct ResolveUnchallenged<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [b"thought", thought.agent.as_ref(), &thought.vrf_nonce_idx.to_le_bytes()],
        bump = thought.bump,
    )]
    pub thought: Account<'info, ThoughtRecord>,

    #[account(
        mut,
        seeds = [b"agent", agent.operator.as_ref()],
        bump = agent.bump,
        constraint = agent.key() == thought.agent @ PotError::WrongAgent,
    )]
    pub agent: Account<'info, AgentProfile>,
}

pub fn resolve_unchallenged_handler(ctx: Context<ResolveUnchallenged>) -> Result<()> {
    let thought = &mut ctx.accounts.thought;
    let agent = &mut ctx.accounts.agent;
    let clock = Clock::get()?;

    let status = ThoughtStatus::from_u8(thought.status).ok_or(PotError::InvalidStatus)?;
    require!(status == ThoughtStatus::Pending, PotError::InvalidStatus);
    require!(
        clock.slot >= thought.challenge_deadline_slot,
        PotError::ChallengeWindowOpen
    );

    thought.status = ThoughtStatus::Finalized as u8;
    agent.active_thoughts = agent.active_thoughts.saturating_sub(1);
    agent.reputation = agent.reputation.saturating_add(1);

    emit!(ThoughtFinalized {
        thought_pda: thought.key(),
        agent: agent.key(),
        status: thought.status,
        slot: clock.slot,
    });

    Ok(())
}

/// Resolve a challenged thought with an authoritative verdict.
///
/// `verdict == true`  → agent is guilty. Stake is slashed: 60% challenger,
///                      30% burn (held by program), 10% policy.treasury.
/// `verdict == false` → challenge fails. Bond is split 90% to agent (griefing
///                      tax), 10% to policy.treasury.
///
/// MVP authority: `policy.resolver` must sign. TODO(decentralized-dispute):
/// replace with multisig / committee verdicts. See docs/future-work.md.
#[derive(Accounts)]
pub struct ResolveChallenged<'info> {
    #[account(mut)]
    pub resolver: Signer<'info>,

    #[account(
        mut,
        seeds = [b"thought", thought.agent.as_ref(), &thought.vrf_nonce_idx.to_le_bytes()],
        bump = thought.bump,
    )]
    pub thought: Account<'info, ThoughtRecord>,

    #[account(
        mut,
        seeds = [b"agent", agent.operator.as_ref()],
        bump = agent.bump,
        constraint = agent.key() == thought.agent @ PotError::WrongAgent,
    )]
    pub agent: Account<'info, AgentProfile>,

    #[account(
        seeds = [b"policy", thought.policy_id.as_ref()],
        bump = policy.bump,
        constraint = policy.resolver == resolver.key() @ PotError::Unauthorized,
    )]
    pub policy: Account<'info, Policy>,

    #[account(
        mut,
        seeds = [b"challenge", thought.key().as_ref(), challenge.challenger.as_ref()],
        bump = challenge.bump,
        constraint = challenge.thought == thought.key() @ PotError::InvalidStatus,
    )]
    pub challenge: Account<'info, Challenge>,

    /// CHECK: stake vault for the agent — receives no funds in this ix; we
    /// debit it via lamport math (vault is a system-owned PDA whose seeds we
    /// re-derive in handler code).
    #[account(
        mut,
        seeds = [b"vault", agent.key().as_ref()],
        bump
    )]
    pub stake_vault: UncheckedAccount<'info>,

    /// CHECK: bond escrow for this challenge.
    #[account(
        mut,
        seeds = [b"bond", challenge.key().as_ref()],
        bump
    )]
    pub bond_vault: UncheckedAccount<'info>,

    /// CHECK: receives challenger payouts. Equality enforced in handler.
    #[account(mut)]
    pub challenger: UncheckedAccount<'info>,

    /// CHECK: policy.treasury — verified by address constraint.
    #[account(mut, address = policy.treasury @ PotError::Unauthorized)]
    pub treasury: UncheckedAccount<'info>,

    /// CHECK: agent operator — receives the 90% bond return on innocent verdict.
    #[account(mut, address = agent.operator @ PotError::WrongAgent)]
    pub agent_operator: UncheckedAccount<'info>,
}

pub fn resolve_challenged_handler(ctx: Context<ResolveChallenged>, verdict: bool) -> Result<()> {
    let thought = &mut ctx.accounts.thought;
    let agent = &mut ctx.accounts.agent;
    let challenge = &mut ctx.accounts.challenge;

    let status = ThoughtStatus::from_u8(thought.status).ok_or(PotError::InvalidStatus)?;
    require!(status == ThoughtStatus::Challenged, PotError::InvalidStatus);
    require!(!challenge.resolved, PotError::InvalidStatus);
    require!(
        challenge.challenger == ctx.accounts.challenger.key(),
        PotError::WrongAgent
    );

    let mut slashed_amount: u64 = 0;

    if verdict {
        // Guilty. Slash the entire stake (per §5.4 the policy floor is
        // 10× max_loss_per_thought, so full slash is intentional). 60/30/10.
        let stake = agent.stake_amount;
        let to_challenger = stake
            .checked_mul(60)
            .ok_or(PotError::Overflow)?
            .checked_div(100)
            .ok_or(PotError::Overflow)?;
        let to_treasury_a = stake
            .checked_mul(10)
            .ok_or(PotError::Overflow)?
            .checked_div(100)
            .ok_or(PotError::Overflow)?;
        // 30% burn — leave in stake_vault and zero accounting.
        let _to_burn = stake
            .checked_sub(to_challenger)
            .ok_or(PotError::Overflow)?
            .checked_sub(to_treasury_a)
            .ok_or(PotError::Overflow)?;

        // Move lamports from stake_vault → challenger / treasury via direct
        // lamport math. Vault is a SystemProgram-owned PDA so we re-derive
        // signer seeds and call SystemProgram::transfer with a signed CPI.
        debit_pda_to(
            &ctx.accounts.stake_vault,
            &ctx.accounts.challenger,
            to_challenger,
        )?;
        debit_pda_to(
            &ctx.accounts.stake_vault,
            &ctx.accounts.treasury,
            to_treasury_a,
        )?;

        // Return the challenger's bond too.
        debit_pda_to(
            &ctx.accounts.bond_vault,
            &ctx.accounts.challenger,
            challenge.bond,
        )?;

        agent.stake_amount = 0;
        agent.reputation = agent.reputation.saturating_sub(10);
        thought.status = ThoughtStatus::Slashed as u8;
        slashed_amount = stake;
    } else {
        // Innocent. Griefing tax: 90% bond → agent operator, 10% → treasury.
        let bond = challenge.bond;
        let to_agent = bond
            .checked_mul(90)
            .ok_or(PotError::Overflow)?
            .checked_div(100)
            .ok_or(PotError::Overflow)?;
        let to_treasury_b = bond
            .checked_sub(to_agent)
            .ok_or(PotError::Overflow)?;

        debit_pda_to(
            &ctx.accounts.bond_vault,
            &ctx.accounts.agent_operator,
            to_agent,
        )?;
        debit_pda_to(
            &ctx.accounts.bond_vault,
            &ctx.accounts.treasury,
            to_treasury_b,
        )?;

        thought.status = ThoughtStatus::Finalized as u8;
        agent.reputation = agent.reputation.saturating_add(2);
    }

    challenge.resolved = true;
    challenge.verdict = verdict;
    agent.active_thoughts = agent.active_thoughts.saturating_sub(1);

    emit!(ChallengeResolved {
        challenge: challenge.key(),
        thought: thought.key(),
        verdict,
        slashed_amount,
    });

    Ok(())
}

/// Move lamports out of a SystemProgram-owned PDA via direct balance edits.
/// Caller is responsible for ensuring the PDA was derived correctly via the
/// `seeds = ...` constraint on the Accounts struct.
fn debit_pda_to(
    from: &UncheckedAccount<'_>,
    to: &UncheckedAccount<'_>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let from_lamports = from.lamports();
    require!(from_lamports >= amount, PotError::Overflow);
    **from.try_borrow_mut_lamports()? = from_lamports
        .checked_sub(amount)
        .ok_or(PotError::Overflow)?;
    let to_lamports = to.lamports();
    **to.try_borrow_mut_lamports()? = to_lamports
        .checked_add(amount)
        .ok_or(PotError::Overflow)?;
    Ok(())
}
