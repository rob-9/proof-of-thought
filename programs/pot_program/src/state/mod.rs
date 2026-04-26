//! Account layouts for the PoT program. Layouts mirror spec §4.1 and §5.1.
//!
//! Every account exposes a `LEN: usize` constant — the on-chain size including
//! Anchor's 8-byte discriminator — used by `init` constraints in instruction
//! contexts.

pub mod agent;
pub mod challenge;
pub mod model;
pub mod policy;
pub mod thought;
pub mod vrf;

pub use agent::*;
pub use challenge::*;
pub use model::*;
pub use policy::*;
pub use thought::*;
pub use vrf::*;
