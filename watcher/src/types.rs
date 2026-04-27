//! On-chain and on-the-wire types for the PoT protocol.
//!
//! These mirror spec sections §4.1 (ThoughtRecord), §4.5 (trace bundle),
//! and §5.1 (Policy / Challenge accounts).
//!
//! TODO(post-merge): the canonical source of these types is the Anchor
//! program in `.worktrees/program` on `feat/program`. Once that branch
//! merges, replace these hand-rolled structs with the IDL-generated ones
//! and import the real discriminators rather than recomputing them.

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Anchor event discriminator for `ThoughtSubmitted` (first 8 bytes of
/// `sha256("event:ThoughtSubmitted")`). Recomputed at runtime via blake3-free
/// SHA-256 to keep the parser self-contained — see [`event_discriminator`].
pub const THOUGHT_SUBMITTED_DISCRIMINATOR: [u8; 8] = compute_anchor_discriminator(b"event:ThoughtSubmitted");

/// Anchor instruction discriminator for `challenge`.
pub const CHALLENGE_IX_DISCRIMINATOR: [u8; 8] = compute_anchor_discriminator(b"global:challenge");

/// Compute Anchor's discriminator (first 8 bytes of SHA-256(prefix)).
///
/// `const fn` so the discriminators above are evaluated at compile time and
/// can be matched against in the log parser without a runtime SHA-256 dep.
const fn compute_anchor_discriminator(prefix: &[u8]) -> [u8; 8] {
    // Compile-time SHA-256 for short inputs. We implement a minimal SHA-256 here
    // to avoid pulling a new dep into types.rs. For runtime callers needing the
    // same hash, see helpers in subscribe.rs.
    let digest = const_sha256(prefix);
    [
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]
}

// --- minimal const-time SHA-256 ---------------------------------------------
// Used purely for compile-time discriminator computation. NOT used at runtime
// — production code uses the well-audited `sha2` crate where needed; here we
// avoid that extra dep because we only need 8 bytes of one short string at
// build time.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const fn const_sha256(input: &[u8]) -> [u8; 32] {
    // Pad input. Allocate a fixed buffer big enough for the discriminator
    // strings used in this crate (max ~64 bytes input).
    let mut buf = [0u8; 128];
    let len = input.len();
    let mut i = 0;
    while i < len {
        buf[i] = input[i];
        i += 1;
    }
    buf[len] = 0x80;
    let bit_len = (len as u64) * 8;
    // Choose padded length: 64 bytes if len < 56, else 128 bytes.
    let padded_len = if len < 56 { 64 } else { 128 };
    // Append big-endian 64-bit length in the last 8 bytes.
    let mut j = 0;
    while j < 8 {
        buf[padded_len - 1 - j] = ((bit_len >> (j * 8)) & 0xff) as u8;
        j += 1;
    }

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut block = 0;
    while block < padded_len {
        let mut w = [0u32; 64];
        let mut t = 0;
        while t < 16 {
            let b0 = buf[block + t * 4] as u32;
            let b1 = buf[block + t * 4 + 1] as u32;
            let b2 = buf[block + t * 4 + 2] as u32;
            let b3 = buf[block + t * 4 + 3] as u32;
            w[t] = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
            t += 1;
        }
        let mut t = 16;
        while t < 64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
            t += 1;
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        let mut t = 0;
        while t < 64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
            t += 1;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);

        block += 64;
    }

    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 8 {
        out[i * 4] = (h[i] >> 24) as u8;
        out[i * 4 + 1] = (h[i] >> 16) as u8;
        out[i * 4 + 2] = (h[i] >> 8) as u8;
        out[i * 4 + 3] = h[i] as u8;
        i += 1;
    }
    out
}

/// Lifecycle states for a thought (mirrors program enum order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ThoughtStatus {
    Pending = 0,
    Challenged = 1,
    Finalized = 2,
    Slashed = 3,
}

/// On-chain account layout (spec §4.1). Watcher reads this via `get_account`
/// when it needs to inspect status, but most of the time the event is enough.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtRecord {
    pub agent: Pubkey,
    pub model_id: [u8; 32],
    pub input_commitment: [u8; 32],
    pub output_commitment: [u8; 32],
    pub trace_uri_hash: [u8; 32],
    pub vrf_seed: [u8; 32],
    pub policy_id: [u8; 32],
    pub slot: u64,
    pub action_pda: Pubkey,
    pub status: u8,
}

/// Anchor event emitted by `submit_thought`. The trace_uri itself rides in
/// the event log (not the account) per spec §4.5.
///
/// Wire layout (after the 8-byte event discriminator):
///   agent: Pubkey (32)
///   thought_pda: Pubkey (32)
///   model_id: [u8;32]
///   input_commitment: [u8;32]
///   output_commitment: [u8;32]
///   trace_uri_hash: [u8;32]
///   vrf_seed: [u8;32]
///   policy_id: [u8;32]
///   slot: u64
///   trace_uri: String (4-byte LE length prefix, then UTF-8 bytes)  -- Borsh
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThoughtSubmittedEvent {
    pub agent: Pubkey,
    pub thought_pda: Pubkey,
    pub model_id: [u8; 32],
    pub input_commitment: [u8; 32],
    pub output_commitment: [u8; 32],
    pub trace_uri_hash: [u8; 32],
    pub vrf_seed: [u8; 32],
    pub policy_id: [u8; 32],
    pub slot: u64,
    pub trace_uri: String,
}

/// Equivalence-class regimes from spec §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquivClass {
    Strict,
    StructuralJSON,
    SemanticCommittee,
    AnyOfN,
}

/// Off-chain projection of the on-chain `Policy` PDA (spec §5.1).
/// This is what the watcher loads from the policy registry to drive
/// regime selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: [u8; 32],
    pub schema_uri_hash: [u8; 32],
    pub equiv_class: EquivClass,
    pub max_inference_ms: u32,
    pub allowed_models: Vec<[u8; 32]>,
    pub challenge_window_slots: u64,
    pub bond_min: u64,
}

/// `ChallengeClaim` mirrors the on-chain enum (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChallengeClaim {
    ModelMismatch = 0,
    OutputMismatch = 1,
    InputOmission = 2,
    Replay = 3,
    StaleVRF = 4,
    /// Inconsistent commitments — agent's claimed output_commitment doesn't
    /// match blake3 of canonical_output bytes in the trace.
    InconsistentCommitments = 5,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminators_are_8_bytes_and_distinct() {
        assert_ne!(THOUGHT_SUBMITTED_DISCRIMINATOR, CHALLENGE_IX_DISCRIMINATOR);
        // The first byte should differ between an event and an instruction
        // discriminator simply because the prefix strings differ.
    }

    #[test]
    fn const_sha256_matches_known_vector() {
        // SHA-256 of empty string
        let empty = const_sha256(b"");
        let expected: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(empty, expected);

        // SHA-256 of "abc"
        let abc = const_sha256(b"abc");
        let expected_abc: [u8; 32] = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(abc, expected_abc);
    }
}
