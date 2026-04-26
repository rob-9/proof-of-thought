use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::errors::PotError;
use crate::state::AgentProfile;

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"agent", operator.key().as_ref()],
        bump = agent.bump,
        has_one = operator @ PotError::WrongAgent,
    )]
    pub agent: Account<'info, AgentProfile>,

    /// CHECK: vault PDA (seeded). System-owned, lamport-only.
    #[account(
        mut,
        seeds = [b"vault", agent.key().as_ref()],
        bump
    )]
    pub stake_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn stake_handler(ctx: Context<Stake>, amount: u64) -> Result<()> {
    require!(amount > 0, PotError::InsufficientStake);

    let cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.operator.to_account_info(),
            to: ctx.accounts.stake_vault.to_account_info(),
        },
    );
    system_program::transfer(cpi, amount)?;

    let agent = &mut ctx.accounts.agent;
    agent.stake_amount = agent
        .stake_amount
        .checked_add(amount)
        .ok_or(PotError::Overflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawStake<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"agent", operator.key().as_ref()],
        bump = agent.bump,
        has_one = operator @ PotError::WrongAgent,
    )]
    pub agent: Account<'info, AgentProfile>,

    /// CHECK: vault PDA.
    #[account(
        mut,
        seeds = [b"vault", agent.key().as_ref()],
        bump
    )]
    pub stake_vault: UncheckedAccount<'info>,
}

pub fn withdraw_handler(ctx: Context<WithdrawStake>, amount: u64) -> Result<()> {
    let agent = &mut ctx.accounts.agent;
    let clock = Clock::get()?;

    require!(
        agent.active_thoughts == 0,
        PotError::ActiveThoughtsExist
    );
    require!(clock.slot >= agent.cooldown_until, PotError::CooldownActive);
    require!(amount <= agent.stake_amount, PotError::InsufficientStake);

    let vault = &ctx.accounts.stake_vault;
    let vault_lamports = vault.lamports();
    require!(vault_lamports >= amount, PotError::InsufficientStake);

    **vault.try_borrow_mut_lamports()? = vault_lamports
        .checked_sub(amount)
        .ok_or(PotError::Overflow)?;
    let op_lamports = ctx.accounts.operator.lamports();
    **ctx.accounts.operator.try_borrow_mut_lamports()? = op_lamports
        .checked_add(amount)
        .ok_or(PotError::Overflow)?;

    agent.stake_amount = agent
        .stake_amount
        .checked_sub(amount)
        .ok_or(PotError::Overflow)?;
    Ok(())
}
