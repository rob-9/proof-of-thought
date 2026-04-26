//! `slash` is intentionally not exposed as an external instruction. Slashing
//! is performed inline by `resolve_challenged_handler` (see resolve.rs) where
//! the verdict is computed atomically with stake math. We keep this module as
//! a placeholder so downstream tooling that lists ix names by file does not
//! choke.
//!
//! TODO(decentralized-dispute): once the dispute committee replaces the
//! single-resolver model, expose a separate `slash` ix that the committee
//! invokes after off-chain quorum. See docs/future-work.md.

use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SlashStub {}

pub fn handler(_ctx: Context<SlashStub>) -> Result<()> {
    Ok(())
}
