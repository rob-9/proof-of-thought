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

    pub system_program: Program<'info, System>,
}

pub fn withdraw_handler(ctx: Context<WithdrawStake>, amount: u64) -> Result<()> {
    let agent_key = ctx.accounts.agent.key();
    let clock = Clock::get()?;

    {
        let agent = &ctx.accounts.agent;
        require!(
            agent.active_thoughts == 0,
            PotError::ActiveThoughtsExist
        );
        require!(clock.slot >= agent.cooldown_until, PotError::CooldownActive);
        require!(amount <= agent.stake_amount, PotError::InsufficientStake);
    }

    let vault = &ctx.accounts.stake_vault;
    require!(vault.lamports() >= amount, PotError::InsufficientStake);

    // The vault is System-owned, so we cannot debit its lamports directly
    // (`try_borrow_mut_lamports` would fail at runtime). Instead, sign for
    // the PDA via its seeds and CPI to System::transfer.
    let bump = ctx.bumps.stake_vault;
    let vault_seeds: &[&[u8]] = &[b"vault", agent_key.as_ref(), std::slice::from_ref(&bump)];
    let signer_seeds: &[&[&[u8]]] = &[vault_seeds];
    let cpi = CpiContext::new_with_signer(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.stake_vault.to_account_info(),
            to: ctx.accounts.operator.to_account_info(),
        },
        signer_seeds,
    );
    system_program::transfer(cpi, amount)?;

    let agent = &mut ctx.accounts.agent;
    agent.stake_amount = agent
        .stake_amount
        .checked_sub(amount)
        .ok_or(PotError::Overflow)?;
    Ok(())
}
