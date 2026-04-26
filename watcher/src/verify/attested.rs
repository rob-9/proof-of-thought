//! Hardware-attested regime (spec §6.2).
//!
//! Validates the structure of an Intel TDX quote attached to the trace.
//! On success, finalization can short-circuit the challenge window.
//!
//! Scope of this MVP:
//! - Parse and structurally validate the TDX_QUOTE header so a malformed
//!   quote is rejected.
//! - Compare the embedded `report_data` against
//!   `H(input_commitment ∥ output_commitment ∥ vrf_seed)` per spec §6.2.
//! - Match `tee_root_ca` against a hardcoded test CA list (used by demo).
//!
//! What this MVP does NOT do, and the production version MUST:
//!   TODO(tee-cert-chain): Verify the quote's signature against Intel's
//!   real DCAP attestation collateral (PCK cert chain rooted at Intel SGX
//!   Root CA). The dcap-quote-verification-rs crate is the obvious
//!   integration point. Without this, the quote is structurally valid
//!   but not cryptographically verified against the real Intel root.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::trace_fetch::TraceBundle;
use crate::types::{ChallengeClaim, ThoughtSubmittedEvent};

use super::{VerifyOutcome, Verifier};

/// Intel TDX quote header layout (spec §6.2 + Intel TDX 1.0 spec).
/// All fields are little-endian where multi-byte.
///
/// Total header = 48 bytes; report body follows; signature trails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdxQuoteHeader {
    pub version: u16,                   // offset 0,  size 2
    pub attestation_key_type: u16,      // offset 2,  size 2
    pub tee_type: u32,                  // offset 4,  size 4
    pub qe_svn: u16,                    // offset 8,  size 2
    pub pce_svn: u16,                   // offset 10, size 2
    pub qe_vendor_id: [u8; 16],         // offset 12, size 16
    pub user_data: [u8; 20],            // offset 28, size 20
    // body and signature follow but are validated below by length only
}

pub const TDX_HEADER_LEN: usize = 48;
pub const TDX_BODY_LEN: usize = 584; // TDX 1.0 report body
pub const TDX_REPORT_DATA_OFFSET_IN_BODY: usize = 520; // 64-byte report_data within body
pub const TDX_REPORT_DATA_LEN: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum TdxParseError {
    #[error("quote shorter than header ({0} bytes)")]
    TooShort(usize),
    #[error("unsupported TDX quote version: {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported attestation key type: {0}")]
    UnsupportedAkt(u16),
    #[error("unsupported tee_type (expected TDX = 0x81)")]
    UnsupportedTeeType,
    #[error("missing report body")]
    MissingBody,
    #[error("missing signature")]
    MissingSignature,
}

/// Parse the header and return the parsed header + the remaining body+signature.
pub fn parse_tdx_header(raw: &[u8]) -> Result<(TdxQuoteHeader, &[u8]), TdxParseError> {
    if raw.len() < TDX_HEADER_LEN {
        return Err(TdxParseError::TooShort(raw.len()));
    }
    let version = u16::from_le_bytes([raw[0], raw[1]]);
    let attestation_key_type = u16::from_le_bytes([raw[2], raw[3]]);
    let tee_type = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let qe_svn = u16::from_le_bytes([raw[8], raw[9]]);
    let pce_svn = u16::from_le_bytes([raw[10], raw[11]]);
    let mut qe_vendor_id = [0u8; 16];
    qe_vendor_id.copy_from_slice(&raw[12..28]);
    let mut user_data = [0u8; 20];
    user_data.copy_from_slice(&raw[28..48]);

    // TDX quote versions: 4 = ECDSA-P256, 5 = ECDSA-P384.
    if version != 4 && version != 5 {
        return Err(TdxParseError::UnsupportedVersion(version));
    }
    // attestation_key_type 2 = ECDSA-P256, 3 = ECDSA-P384.
    if attestation_key_type != 2 && attestation_key_type != 3 {
        return Err(TdxParseError::UnsupportedAkt(attestation_key_type));
    }
    // TDX tee_type = 0x81. (SGX = 0x00.)
    if tee_type != 0x81 {
        return Err(TdxParseError::UnsupportedTeeType);
    }

    Ok((
        TdxQuoteHeader {
            version,
            attestation_key_type,
            tee_type,
            qe_svn,
            pce_svn,
            qe_vendor_id,
            user_data,
        },
        &raw[TDX_HEADER_LEN..],
    ))
}

/// Extract the 64-byte `report_data` from the body. Returns an error if the
/// body is too short.
pub fn extract_report_data(body_and_sig: &[u8]) -> Result<[u8; TDX_REPORT_DATA_LEN], TdxParseError> {
    if body_and_sig.len() < TDX_BODY_LEN {
        return Err(TdxParseError::MissingBody);
    }
    let body = &body_and_sig[..TDX_BODY_LEN];
    let mut out = [0u8; TDX_REPORT_DATA_LEN];
    out.copy_from_slice(
        &body[TDX_REPORT_DATA_OFFSET_IN_BODY..TDX_REPORT_DATA_OFFSET_IN_BODY + TDX_REPORT_DATA_LEN],
    );
    Ok(out)
}

/// Compute the expected `report_data` = H(input_commitment ∥ output_commitment ∥ vrf_seed),
/// padded to 64 bytes (TDX report_data is 64 bytes wide; we put the 32-byte
/// blake3 in the first half and zero the rest, matching the agent SDK's
/// canonical layout).
pub fn expected_report_data(
    input_commitment: &[u8; 32],
    output_commitment: &[u8; 32],
    vrf_seed: &[u8; 32],
) -> [u8; TDX_REPORT_DATA_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(input_commitment);
    hasher.update(output_commitment);
    hasher.update(vrf_seed);
    let h: [u8; 32] = hasher.finalize().into();
    let mut out = [0u8; TDX_REPORT_DATA_LEN];
    out[..32].copy_from_slice(&h);
    out
}

/// Validate quote structure end-to-end. Does NOT verify cert chain.
pub fn validate_quote_structure(raw: &[u8]) -> Result<TdxQuoteHeader, TdxParseError> {
    let (header, rest) = parse_tdx_header(raw)?;
    if rest.len() < TDX_BODY_LEN {
        return Err(TdxParseError::MissingBody);
    }
    if rest.len() == TDX_BODY_LEN {
        return Err(TdxParseError::MissingSignature);
    }
    Ok(header)
}

// ---------------------------------------------------------------------------
// Verifier
// ---------------------------------------------------------------------------

/// AttestedVerifier — checks the trace's TEE quote.
///
/// `trusted_root_cas` is consulted when matching an on-chain `tee_root_ca`
/// field. For the MVP, we accept a hardcoded list seeded by the demo. Real
/// deployments must use Intel's published DCAP root CA.
pub struct AttestedVerifier {
    pub trusted_root_cas: Vec<[u8; 32]>,
}

impl AttestedVerifier {
    pub fn new(trusted_root_cas: Vec<[u8; 32]>) -> Self {
        Self { trusted_root_cas }
    }

    /// Default test CA list — a single all-zero CA used by demo fixtures.
    /// Replace with Intel's real CA fingerprint before deploying.
    pub fn with_test_cas() -> Self {
        Self {
            trusted_root_cas: vec![[0u8; 32]],
        }
    }
}

#[async_trait]
impl Verifier for AttestedVerifier {
    async fn verify(
        &self,
        event: &ThoughtSubmittedEvent,
        bundle: &TraceBundle,
    ) -> VerifyOutcome {
        let Some(quote) = bundle.attestation.as_ref() else {
            return VerifyOutcome::Inconclusive {
                reason: "no attestation present in trace bundle".to_string(),
            };
        };

        let header = match validate_quote_structure(quote) {
            Ok(h) => h,
            Err(e) => {
                warn!(thought_pda = %event.thought_pda, error = %e, "malformed TDX quote");
                return VerifyOutcome::Mismatch {
                    claim: ChallengeClaim::OutputMismatch,
                    evidence_uri: event.trace_uri.clone(),
                };
            }
        };
        debug!(?header, "TDX quote header parsed");

        let body_and_sig = &quote[TDX_HEADER_LEN..];
        let report_data = match extract_report_data(body_and_sig) {
            Ok(rd) => rd,
            Err(e) => {
                warn!(error = %e, "could not extract report_data");
                return VerifyOutcome::Mismatch {
                    claim: ChallengeClaim::OutputMismatch,
                    evidence_uri: event.trace_uri.clone(),
                };
            }
        };

        let expected = expected_report_data(
            &event.input_commitment,
            &event.output_commitment,
            &event.vrf_seed,
        );

        if report_data != expected {
            warn!(
                thought_pda = %event.thought_pda,
                "TDX report_data does not bind to the on-chain commitments"
            );
            return VerifyOutcome::Mismatch {
                claim: ChallengeClaim::OutputMismatch,
                evidence_uri: event.trace_uri.clone(),
            };
        }

        // TODO(tee-cert-chain): cryptographically verify the signature against
        // a PCK cert chain rooted at Intel's real attestation root CA. Without
        // this we are only checking structural validity + binding.
        if self.trusted_root_cas.is_empty() {
            return VerifyOutcome::Inconclusive {
                reason: "no trusted TEE root CAs configured".to_string(),
            };
        }

        VerifyOutcome::Match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_quote(report_data: [u8; 64]) -> Vec<u8> {
        let mut q = Vec::new();
        // Header
        q.extend_from_slice(&4u16.to_le_bytes()); // version 4
        q.extend_from_slice(&2u16.to_le_bytes()); // ECDSA-P256
        q.extend_from_slice(&0x81u32.to_le_bytes()); // TDX
        q.extend_from_slice(&1u16.to_le_bytes()); // qe_svn
        q.extend_from_slice(&1u16.to_le_bytes()); // pce_svn
        q.extend_from_slice(&[0xCC; 16]); // qe_vendor_id (Intel)
        q.extend_from_slice(&[0u8; 20]); // user_data
        // Body (584 bytes). Place report_data at the right offset.
        let mut body = vec![0u8; TDX_BODY_LEN];
        body[TDX_REPORT_DATA_OFFSET_IN_BODY..TDX_REPORT_DATA_OFFSET_IN_BODY + 64]
            .copy_from_slice(&report_data);
        q.extend_from_slice(&body);
        // Signature placeholder
        q.extend_from_slice(&[0u8; 64]);
        q
    }

    #[test]
    fn header_parses() {
        let q = build_quote([0u8; 64]);
        let (h, rest) = parse_tdx_header(&q).unwrap();
        assert_eq!(h.version, 4);
        assert_eq!(h.tee_type, 0x81);
        assert!(rest.len() > TDX_BODY_LEN);
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut q = build_quote([0u8; 64]);
        q[0] = 99;
        let res = parse_tdx_header(&q);
        assert!(matches!(res, Err(TdxParseError::UnsupportedVersion(_))));
    }
}
