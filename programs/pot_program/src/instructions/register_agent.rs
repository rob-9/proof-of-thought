use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::errors::PotError;
use crate::state::AgentProfile;

#[derive(Accounts)]
pub struct RegisterAgent<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        init,
        payer = operator,
        space = AgentProfile::LEN,
        seeds = [b"agent", operator.key().as_ref()],
        bump
    )]
    pub agent: Account<'info, AgentProfile>,

    /// CHECK: vault is a system-owned PDA that holds stake lamports. We
    /// validate it via seeds, never read its data, and only move lamports
    /// using SystemProgram::Transfer (deposit) or direct lamport math
    /// (withdrawal — vault is owned by SystemProgram so a normal transfer
    /// requires the vault to sign as PDA). The first deposit funds rent-exempt
    /// reserve; we never close it.
    #[account(
        mut,
        seeds = [b"vault", agent.key().as_ref()],
        bump
    )]
    pub stake_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn register_agent_handler(ctx: Context<RegisterAgent>, stake: u64) -> Result<()> {
    require!(stake > 0, PotError::InsufficientStake);

    let agent = &mut ctx.accounts.agent;
    agent.operator = ctx.accounts.operator.key();
    agent.stake_vault = ctx.accounts.stake_vault.key();
    agent.stake_amount = stake;
    agent.reputation = 0;
    agent.active_thoughts = 0;
    agent.cooldown_until = 0;
    agent.vrf_nonce = 0;
    agent.bump = ctx.bumps.agent;

    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.operator.to_account_info(),
            to: ctx.accounts.stake_vault.to_account_info(),
        },
    );
    system_program::transfer(cpi_ctx, stake)?;

    Ok(())
}
