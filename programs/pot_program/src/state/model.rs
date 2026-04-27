use anchor_lang::prelude::*;

/// Model identity classes from spec §4.4.
#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Copy, Clone, Debug)]
pub enum ModelClass {
    /// Open weights, deterministic re-execution. `model_id` = blake3(safetensors).
    OpenWeights = 0,
    /// Hosted closed model. `verifier_pubkey` is the provider's signing key.
    Hosted = 1,
    /// TEE-fronted inference. `tee_root_ca` validates attestation quotes.
    TeeAttested = 2,
}

/// Registry entry for a model class. PDA: ["model", model_id].
///
/// Created by governance only (see GOVERNANCE in lib.rs).
#[account]
pub struct ModelRegistry {
    pub model_id: [u8; 32],
    pub class: u8, // ModelClass; stored as u8 to keep layout flat
    pub verifier_pubkey: Pubkey,
    pub tee_root_ca: Pubkey,
    pub registered_by: Pubkey,
    pub bump: u8,
}

impl ModelRegistry {
    // disc + 32 + 1 + 32 + 32 + 32 + 1 = 138
    pub const LEN: usize = 8 + 32 + 1 + 32 + 32 + 32 + 1;
}
