//! Instruction modules. Each file declares one Accounts context and one
//! handler. lib.rs re-exports them and wires the `#[program]` mod.

pub mod challenge;
pub mod consume_thought;
pub mod register_agent;
pub mod register_model;
pub mod register_policy;
pub mod request_vrf;
pub mod resolve;
pub mod slash;
pub mod stake;
pub mod submit_thought;

pub use challenge::*;
pub use consume_thought::*;
pub use register_agent::*;
pub use register_model::*;
pub use register_policy::*;
pub use request_vrf::*;
pub use resolve::*;
pub use slash::*;
pub use stake::*;
pub use submit_thought::*;
