use anchor_lang::prelude::*;

use crate::errors::PotError;
use crate::state::{ModelClass, ModelRegistry};
use crate::GOVERNANCE;

#[derive(Accounts)]
#[instruction(model_id: [u8; 32])]
pub struct RegisterModel<'info> {
    #[account(mut, address = GOVERNANCE @ PotError::Unauthorized)]
    pub governance: Signer<'info>,

    #[account(
        init,
        payer = governance,
        space = ModelRegistry::LEN,
        seeds = [b"model", model_id.as_ref()],
        bump
    )]
    pub model: Account<'info, ModelRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn register_model_handler(
    ctx: Context<RegisterModel>,
    model_id: [u8; 32],
    class: ModelClass,
    verifier_pubkey: Pubkey,
    tee_root_ca: Pubkey,
) -> Result<()> {
    let m = &mut ctx.accounts.model;
    m.model_id = model_id;
    m.class = class as u8;
    m.verifier_pubkey = verifier_pubkey;
    m.tee_root_ca = tee_root_ca;
    m.registered_by = ctx.accounts.governance.key();
    m.bump = ctx.bumps.model;
    Ok(())
}
