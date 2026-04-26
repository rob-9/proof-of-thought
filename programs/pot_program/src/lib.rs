//! # pot_program — Proof of Thought protocol
//!
//! On-chain primitive that lets autonomous agents cryptographically attest to
//! having performed reasoning before taking on-chain actions.
//!
//! Lifecycle (see spec §5.3):
//!
//! 1. `register_agent` — operator stakes lamports.
//! 2. `register_policy` — anyone publishes a policy a consumer can trust.
//! 3. `register_model` — governance whitelists models the protocol can verify.
//! 4. `request_vrf` — agent allocates a fresh VRF seed.
//! 5. `submit_thought` — agent commits a hash of (input, output, model, seed),
//!    binding the thought to a specific slot. Status = Pending.
//! 6. `challenge` — any watcher locks a bond and disputes during the window.
//! 7. `resolve` — past the window with no challenge, status = Finalized.
//!    With a challenge, the policy resolver writes a verdict and slashing /
//!    griefing tax flows.
//! 8. `consume_thought` — a downstream program CPIs into PoT to gate an action.
//! 9. `stake` / `withdraw_stake` — gated on cooldown and 0 active thoughts.
//!
//! Every instruction's invariants are documented in its module. Errors are in
//! `crate::errors`.

#![deny(unsafe_code)]
#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;
use state::{ChallengeClaim, ModelClass, ThoughtRecordArgs};

declare_id!("Pot11111111111111111111111111111111111111111");

/// Governance authority allowed to call `register_model`.
///
/// MVP: hardcoded to the system program id placeholder. In production this
/// is replaced with a multisig PDA. TODO(governance-multisig): swap for
/// SPL governance integration. See docs/future-work.md.
pub const GOVERNANCE: Pubkey = pubkey!("Gov1111111111111111111111111111111111111111");

#[program]
pub mod pot_program {
    use super::*;

    pub fn register_agent(ctx: Context<RegisterAgent>, stake: u64) -> Result<()> {
        instructions::register_agent::handler(ctx, stake)
    }

    pub fn register_model(
        ctx: Context<RegisterModel>,
        model_id: [u8; 32],
        class: ModelClass,
        verifier_pubkey: Pubkey,
        tee_root_ca: Pubkey,
    ) -> Result<()> {
        instructions::register_model::handler(ctx, model_id, class, verifier_pubkey, tee_root_ca)
    }

    pub fn register_policy(
        ctx: Context<RegisterPolicy>,
        args: RegisterPolicyArgs,
    ) -> Result<()> {
        instructions::register_policy::handler(ctx, args)
    }

    pub fn request_vrf(
        ctx: Context<RequestVrf>,
        nonce_idx: u64,
        seed: [u8; 32],
    ) -> Result<()> {
        instructions::request_vrf::handler(ctx, nonce_idx, seed)
    }

    pub fn submit_thought(
        ctx: Context<SubmitThought>,
        args: ThoughtRecordArgs,
        trace_uri: String,
    ) -> Result<()> {
        instructions::submit_thought::handler(ctx, args, trace_uri)
    }

    pub fn consume_thought(ctx: Context<ConsumeThought>) -> Result<()> {
        instructions::consume_thought::handler(ctx)
    }

    pub fn challenge(
        ctx: Context<OpenChallenge>,
        claim: ChallengeClaim,
        bond: u64,
        evidence_uri_hash: [u8; 32],
    ) -> Result<()> {
        instructions::challenge::handler(ctx, claim, bond, evidence_uri_hash)
    }

    pub fn resolve_unchallenged(ctx: Context<ResolveUnchallenged>) -> Result<()> {
        instructions::resolve::resolve_unchallenged_handler(ctx)
    }

    pub fn resolve_challenged(
        ctx: Context<ResolveChallenged>,
        verdict: bool,
    ) -> Result<()> {
        instructions::resolve::resolve_challenged_handler(ctx, verdict)
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        instructions::stake::stake_handler(ctx, amount)
    }

    pub fn withdraw_stake(ctx: Context<WithdrawStake>, amount: u64) -> Result<()> {
        instructions::stake::withdraw_handler(ctx, amount)
    }
}
