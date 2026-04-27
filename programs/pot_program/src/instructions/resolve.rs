use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

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

    pub system_program: Program<'info, System>,
}

pub fn resolve_challenged_handler(ctx: Context<ResolveChallenged>, verdict: bool) -> Result<()> {
    let agent_key = ctx.accounts.agent.key();
    let challenge_key = ctx.accounts.challenge.key();
    let stake_vault_key = ctx.accounts.stake_vault.key();
    let bond_vault_key = ctx.accounts.bond_vault.key();
    let stake_vault_bump = ctx.bumps.stake_vault;
    let bond_vault_bump = ctx.bumps.bond_vault;

    let status = ThoughtStatus::from_u8(ctx.accounts.thought.status)
        .ok_or(PotError::InvalidStatus)?;
    require!(status == ThoughtStatus::Challenged, PotError::InvalidStatus);
    require!(!ctx.accounts.challenge.resolved, PotError::InvalidStatus);
    require!(
        ctx.accounts.challenge.challenger == ctx.accounts.challenger.key(),
        PotError::WrongAgent
    );
    // I8: the resolver could try to alias `challenger` to one of the vaults,
    // turning a payout into a self-credit no-op. Forbid it explicitly.
    require!(
        ctx.accounts.challenger.key() != stake_vault_key
            && ctx.accounts.challenger.key() != bond_vault_key,
        PotError::Unauthorized
    );

    let challenge_bond = ctx.accounts.challenge.bond;
    let agent_stake = ctx.accounts.agent.stake_amount;
    let mut slashed_amount: u64 = 0;

    if verdict {
        // Guilty. Slash the entire stake (per §5.4 the policy floor is
        // 10× max_loss_per_thought, so full slash is intentional). 60/30/10.
        let stake = agent_stake;
        let to_challenger = (stake as u128 * 60 / 100) as u64;
        let to_treasury_a = (stake as u128 * 10 / 100) as u64;
        // 30% remains in the stake_vault and is never debited — effectively
        // burned for the lifetime of the protocol upgrade authority. See
        // future-work.md § "incinerator pubkey" for the upgrade path.
        let _to_burn = stake
            .checked_sub(to_challenger)
            .ok_or(PotError::Overflow)?
            .checked_sub(to_treasury_a)
            .ok_or(PotError::Overflow)?;

        let stake_signer: &[&[u8]] = &[
            b"vault",
            agent_key.as_ref(),
            std::slice::from_ref(&stake_vault_bump),
        ];
        let stake_signers: &[&[&[u8]]] = &[stake_signer];
        transfer_signed(
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.challenger.to_account_info(),
            to_challenger,
            stake_signers,
        )?;
        transfer_signed(
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.treasury.to_account_info(),
            to_treasury_a,
            stake_signers,
        )?;

        // Return the challenger's bond.
        let bond_signer: &[&[u8]] = &[
            b"bond",
            challenge_key.as_ref(),
            std::slice::from_ref(&bond_vault_bump),
        ];
        let bond_signers: &[&[&[u8]]] = &[bond_signer];
        transfer_signed(
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.bond_vault.to_account_info(),
            ctx.accounts.challenger.to_account_info(),
            challenge_bond,
            bond_signers,
        )?;

        let agent = &mut ctx.accounts.agent;
        agent.stake_amount = 0;
        agent.reputation = agent.reputation.saturating_sub(10);
        ctx.accounts.thought.status = ThoughtStatus::Slashed as u8;
        slashed_amount = stake;
    } else {
        // Innocent. Griefing tax: 90% bond → agent operator, 10% → treasury.
        let bond = challenge_bond;
        let to_agent = (bond as u128 * 90 / 100) as u64;
        let to_treasury_b = bond
            .checked_sub(to_agent)
            .ok_or(PotError::Overflow)?;

        let bond_signer: &[&[u8]] = &[
            b"bond",
            challenge_key.as_ref(),
            std::slice::from_ref(&bond_vault_bump),
        ];
        let bond_signers: &[&[&[u8]]] = &[bond_signer];
        transfer_signed(
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.bond_vault.to_account_info(),
            ctx.accounts.agent_operator.to_account_info(),
            to_agent,
            bond_signers,
        )?;
        transfer_signed(
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.bond_vault.to_account_info(),
            ctx.accounts.treasury.to_account_info(),
            to_treasury_b,
            bond_signers,
        )?;

        ctx.accounts.thought.status = ThoughtStatus::Finalized as u8;
        ctx.accounts.agent.reputation = ctx.accounts.agent.reputation.saturating_add(2);
    }

    let challenge = &mut ctx.accounts.challenge;
    challenge.resolved = true;
    challenge.verdict = verdict;
    let agent = &mut ctx.accounts.agent;
    agent.active_thoughts = agent.active_thoughts.saturating_sub(1);

    emit!(ChallengeResolved {
        challenge: challenge.key(),
        thought: ctx.accounts.thought.key(),
        verdict,
        slashed_amount,
    });

    Ok(())
}

/// Sign-and-transfer SOL from a SystemProgram-owned PDA via a signed CPI.
///
/// Direct lamport mutation (`try_borrow_mut_lamports`) fails at runtime on
/// System-owned accounts; the program must invoke SystemProgram::transfer
/// with the PDA's seeds as signer.
fn transfer_signed<'info>(
    system_program: AccountInfo<'info>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let cpi = CpiContext::new_with_signer(
        system_program,
        Transfer { from, to },
        signer_seeds,
    );
    system_program::transfer(cpi, amount)
}
