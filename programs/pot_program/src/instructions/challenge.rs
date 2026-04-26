use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::errors::PotError;
use crate::state::{
    Challenge, ChallengeClaim, ChallengeOpened, Policy, ThoughtRecord, ThoughtStatus,
};

#[derive(Accounts)]
pub struct OpenChallenge<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,

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

    #[account(
        init,
        payer = challenger,
        space = Challenge::LEN,
        seeds = [b"challenge", thought.key().as_ref(), challenger.key().as_ref()],
        bump
    )]
    pub challenge: Account<'info, Challenge>,

    /// CHECK: PDA holding the challenger bond. SystemProgram-owned.
    #[account(
        mut,
        seeds = [b"bond", challenge.key().as_ref()],
        bump
    )]
    pub bond_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn open_challenge_handler(
    ctx: Context<OpenChallenge>,
    claim: ChallengeClaim,
    bond: u64,
    evidence_uri_hash: [u8; 32],
) -> Result<()> {
    let thought = &mut ctx.accounts.thought;
    let policy = &ctx.accounts.policy;
    let clock = Clock::get()?;

    let status = ThoughtStatus::from_u8(thought.status).ok_or(PotError::InvalidStatus)?;
    require!(status == ThoughtStatus::Pending, PotError::InvalidStatus);
    require!(
        clock.slot < thought.challenge_deadline_slot,
        PotError::ChallengeWindowClosed
    );
    require!(bond >= policy.bond_min, PotError::BondTooLow);

    // Lock the bond.
    let cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.challenger.to_account_info(),
            to: ctx.accounts.bond_vault.to_account_info(),
        },
    );
    system_program::transfer(cpi, bond)?;

    let c = &mut ctx.accounts.challenge;
    c.thought = thought.key();
    c.challenger = ctx.accounts.challenger.key();
    c.bond = bond;
    c.claim = claim as u8;
    c.evidence_uri_hash = evidence_uri_hash;
    c.opened_at_slot = clock.slot;
    c.resolved = false;
    c.verdict = false;
    c.bump = ctx.bumps.challenge;

    thought.status = ThoughtStatus::Challenged as u8;

    emit!(ChallengeOpened {
        challenge: c.key(),
        thought: thought.key(),
        challenger: c.challenger,
        claim: c.claim,
        bond,
    });

    Ok(())
}
