use anchor_lang::prelude::*;

use crate::errors::PotError;
use crate::state::{
    AgentProfile, ModelRegistry, Policy, ThoughtRecord, ThoughtRecordArgs, ThoughtStatus,
    ThoughtSubmitted, VrfRequest,
};

#[derive(Accounts)]
#[instruction(args: ThoughtRecordArgs, _trace_uri: String)]
pub struct SubmitThought<'info> {
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
        seeds = [b"model", args.model_id.as_ref()],
        bump = model.bump,
        constraint = model.model_id == args.model_id @ PotError::ModelNotRegistered,
    )]
    pub model: Account<'info, ModelRegistry>,

    #[account(
        seeds = [b"policy", args.policy_id.as_ref()],
        bump = policy.bump,
        constraint = policy.policy_id == args.policy_id @ PotError::WrongPolicy,
    )]
    pub policy: Account<'info, Policy>,

    #[account(
        mut,
        seeds = [b"vrf", agent.key().as_ref(), &args.vrf_nonce_idx.to_le_bytes()],
        bump = vrf_request.bump,
        constraint = vrf_request.agent == agent.key() @ PotError::WrongAgent,
    )]
    pub vrf_request: Account<'info, VrfRequest>,

    #[account(
        init,
        payer = operator,
        space = ThoughtRecord::LEN,
        seeds = [b"thought", agent.key().as_ref(), &args.vrf_nonce_idx.to_le_bytes()],
        bump
    )]
    pub thought: Account<'info, ThoughtRecord>,

    pub system_program: Program<'info, System>,
}

pub fn submit_thought_handler(
    ctx: Context<SubmitThought>,
    args: ThoughtRecordArgs,
    trace_uri: String,
) -> Result<()> {
    let agent = &mut ctx.accounts.agent;
    let policy = &ctx.accounts.policy;
    let vrf = &mut ctx.accounts.vrf_request;
    let clock = Clock::get()?;

    // 1. Stake floor.
    require!(agent.stake_amount >= policy.bond_min, PotError::InsufficientStake);

    // 2. VRF freshness + single-use.
    require!(!vrf.consumed, PotError::VRFAlreadyConsumed);
    require!(vrf.seed == args.vrf_seed, PotError::VRFTooStale);
    let age = clock
        .slot
        .checked_sub(vrf.request_slot)
        .ok_or(PotError::Overflow)?;
    require!(
        age <= policy.max_inference_slots as u64,
        PotError::VRFTooStale
    );

    // 3. Model whitelisted by policy (if list is non-empty — empty == any).
    if !policy.allowed_models.is_empty() {
        require!(
            policy.allowed_models.iter().any(|m| m == &args.model_id),
            PotError::ModelNotRegistered
        );
    }

    vrf.consumed = true;

    // 4. Persist commitment.
    let t = &mut ctx.accounts.thought;
    t.agent = agent.key();
    t.model_id = args.model_id;
    t.input_commitment = args.input_commitment;
    t.output_commitment = args.output_commitment;
    t.trace_uri_hash = args.trace_uri_hash;
    t.vrf_seed = args.vrf_seed;
    t.policy_id = args.policy_id;
    t.slot = clock.slot;
    t.action_pda = args.action_pda;
    t.status = ThoughtStatus::Pending as u8;
    t.attestation_verified = false;
    t.challenge_deadline_slot = clock
        .slot
        .checked_add(policy.challenge_window_slots)
        .ok_or(PotError::Overflow)?;
    t.consumed_count = 0;
    t.vrf_nonce_idx = args.vrf_nonce_idx;
    t.bump = ctx.bumps.thought;
    t._pad = [0; 7];

    // 5. Bookkeeping on the agent.
    agent.active_thoughts = agent
        .active_thoughts
        .checked_add(1)
        .ok_or(PotError::Overflow)?;

    emit!(ThoughtSubmitted {
        thought_pda: t.key(),
        agent: agent.key(),
        model_id: args.model_id,
        policy_id: args.policy_id,
        slot: clock.slot,
        trace_uri,
    });

    Ok(())
}
