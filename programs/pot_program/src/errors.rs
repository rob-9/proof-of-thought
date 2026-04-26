use anchor_lang::prelude::*;

/// PoT program errors.
///
/// Codes are stable across releases — downstream consumers (CPI callers)
/// should match on the variant name, not the discriminator.
#[error_code]
pub enum PotError {
    #[msg("Agent stake is below the policy bond minimum")]
    InsufficientStake = 6000,

    #[msg("Model is not registered with the protocol")]
    ModelNotRegistered = 6001,

    #[msg("ThoughtRecord.action_pda does not match the action being executed")]
    WrongAction = 6002,

    #[msg("Policy id mismatch between thought and consumer")]
    WrongPolicy = 6003,

    #[msg("Thought is not yet finalized or attestation-verified")]
    ThoughtNotReady = 6004,

    #[msg("Thought is older than policy.max_action_age_slots")]
    ThoughtStale = 6005,

    #[msg("VRF request is older than policy.max_inference_slots")]
    VRFTooStale = 6006,

    #[msg("VRF request has already been consumed by a previous thought")]
    VRFAlreadyConsumed = 6007,

    #[msg("Challenge window has closed; cannot open a dispute")]
    ChallengeWindowClosed = 6008,

    #[msg("Challenge window is still open; cannot finalize yet")]
    ChallengeWindowOpen = 6009,

    #[msg("Challenger bond is below the policy bond minimum")]
    BondTooLow = 6010,

    #[msg("Signer is not the operator of this agent")]
    WrongAgent = 6011,

    #[msg("ThoughtRecord status is invalid for this transition")]
    InvalidStatus = 6012,

    #[msg("Stake withdrawal blocked: cooldown is still active")]
    CooldownActive = 6013,

    #[msg("Stake withdrawal blocked: agent has active thoughts in flight")]
    ActiveThoughtsExist = 6014,

    #[msg("Provided trace_uri does not hash to ThoughtRecord.trace_uri_hash")]
    EvidenceMismatch = 6015,

    #[msg("Arithmetic overflow")]
    Overflow = 6016,

    #[msg("Caller is not authorized for this instruction")]
    Unauthorized = 6017,
}
