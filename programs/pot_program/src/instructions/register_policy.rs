use anchor_lang::prelude::*;

use crate::state::{EquivClass, Policy};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RegisterPolicyArgs {
    pub policy_id: [u8; 32],
    pub schema_uri_hash: [u8; 32],
    pub equiv_class: EquivClass,
    pub max_inference_slots: u32,
    pub max_action_age_slots: u64,
    pub challenge_window_slots: u64,
    pub bond_min: u64,
    pub resolver: Pubkey,
    pub treasury: Pubkey,
    pub allowed_models: Vec<[u8; 32]>,
}

#[derive(Accounts)]
#[instruction(args: RegisterPolicyArgs)]
pub struct RegisterPolicy<'info> {
    #[account(mut)]
    pub author: Signer<'info>,

    #[account(
        init,
        payer = author,
        space = Policy::LEN,
        seeds = [b"policy", args.policy_id.as_ref()],
        bump
    )]
    pub policy: Account<'info, Policy>,

    pub system_program: Program<'info, System>,
}

pub fn register_policy_handler(ctx: Context<RegisterPolicy>, args: RegisterPolicyArgs) -> Result<()> {
    require!(
        args.allowed_models.len() <= Policy::MAX_ALLOWED_MODELS,
        crate::errors::PotError::Overflow
    );

    let p = &mut ctx.accounts.policy;
    p.policy_id = args.policy_id;
    p.schema_uri_hash = args.schema_uri_hash;
    p.equiv_class = args.equiv_class as u8;
    p.max_inference_slots = args.max_inference_slots;
    p.max_action_age_slots = args.max_action_age_slots;
    p.challenge_window_slots = args.challenge_window_slots;
    p.bond_min = args.bond_min;
    p.resolver = args.resolver;
    p.treasury = args.treasury;
    p.allowed_models = args.allowed_models;
    p.bump = ctx.bumps.policy;
    Ok(())
}
