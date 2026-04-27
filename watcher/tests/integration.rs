//! End-to-end pipeline tests with mocked I/O.
//!
//! Each test wires a synthetic `ThoughtSubmittedEvent` through the real
//! `ByteCompareVerifier` and `ChallengeFiler`, with `MockFetcher` and
//! `MockChallengeSubmitter` substituted for I/O. This exercises the
//! full decision path:
//!
//!     event → fetch trace → verify → decide(EV) → file challenge?

use solana_sdk::pubkey::Pubkey;

use pot_watcher::{
    challenge::{ChallengeFiler, FileDecision, MockChallengeSubmitter},
    config::BondStrategy,
    trace_fetch::{TraceBundle, TraceFetcher},
    types::{ChallengeClaim, ThoughtSubmittedEvent},
    verify::{ByteCompareVerifier, VerifyOutcome, Verifier},
    MockFetcher,
};

const SOL: u64 = 1_000_000_000;

fn dummy_pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn make_bundle(canonical_output: &[u8]) -> TraceBundle {
    TraceBundle {
        raw_canonical_output: canonical_output.to_vec(),
        raw_canonical_input: b"canonical-input-bytes".to_vec(),
        attestation: None,
        manifest: ciborium::Value::Null,
    }
}

fn make_event(
    output_commitment: [u8; 32],
    trace_uri: &str,
    trace_uri_hash: [u8; 32],
) -> ThoughtSubmittedEvent {
    ThoughtSubmittedEvent {
        agent: dummy_pubkey(1),
        thought_pda: dummy_pubkey(2),
        model_id: [3u8; 32],
        input_commitment: [4u8; 32],
        output_commitment,
        trace_uri_hash,
        vrf_seed: [5u8; 32],
        policy_id: [6u8; 32],
        slot: 100,
        trace_uri: trace_uri.into(),
    }
}

#[tokio::test]
async fn inconsistent_commitments_trigger_challenge() {
    let canonical_output = b"the-real-canonical-output-bytes";
    let bundle = make_bundle(canonical_output);

    // Adversary: claims a different output_commitment than what's in the bundle.
    let lying_commitment = [0xFFu8; 32];
    let trace_uri = "ar://adversary-trace";
    let trace_uri_hash: [u8; 32] = blake3::hash(trace_uri.as_bytes()).into();

    let event = make_event(lying_commitment, trace_uri, trace_uri_hash);

    let fetcher = MockFetcher::new();
    fetcher.insert(trace_uri, bundle.clone());

    let bundle_returned = fetcher
        .fetch(trace_uri, trace_uri_hash)
        .await
        .expect("MockFetcher should return inserted bundle");

    let verifier = ByteCompareVerifier::new();
    let outcome = verifier.verify(&event, &bundle_returned).await;

    assert!(
        matches!(
            outcome,
            VerifyOutcome::Mismatch {
                claim: ChallengeClaim::InconsistentCommitments,
                ..
            }
        ),
        "expected Mismatch for byte-mismatched commitments, got {outcome:?}"
    );

    let submitter = MockChallengeSubmitter::new();
    let filer = ChallengeFiler::new(submitter.clone(), BondStrategy::Conservative, 50 * SOL);
    let sig = filer
        .decide_and_file(&outcome, 100 * SOL, event.thought_pda, 500_000_000)
        .await
        .expect("decide_and_file should not error in EV-positive case");
    assert!(sig.is_some(), "expected a challenge to be filed");
    assert_eq!(submitter.count(), 1, "submitter should have one filing");
    let filed = submitter.submitted();
    assert_eq!(filed[0].claim, ChallengeClaim::InconsistentCommitments);
    assert_eq!(filed[0].thought_pda, event.thought_pda);
}

#[tokio::test]
async fn matching_commitments_no_challenge() {
    let canonical_output = b"honest-canonical-output";
    let bundle = make_bundle(canonical_output);

    let true_commitment: [u8; 32] = blake3::hash(canonical_output).into();
    let trace_uri = "ar://honest-trace";
    let trace_uri_hash: [u8; 32] = blake3::hash(trace_uri.as_bytes()).into();

    let event = make_event(true_commitment, trace_uri, trace_uri_hash);

    let fetcher = MockFetcher::new();
    fetcher.insert(trace_uri, bundle.clone());

    let bundle_returned = fetcher
        .fetch(trace_uri, trace_uri_hash)
        .await
        .expect("MockFetcher should return inserted bundle");

    let verifier = ByteCompareVerifier::new();
    let outcome = verifier.verify(&event, &bundle_returned).await;
    assert!(
        matches!(outcome, VerifyOutcome::Match),
        "honest agent should produce VerifyOutcome::Match"
    );

    let submitter = MockChallengeSubmitter::new();
    let filer = ChallengeFiler::new(submitter.clone(), BondStrategy::Conservative, 50 * SOL);
    let sig = filer
        .decide_and_file(&outcome, 100 * SOL, event.thought_pda, 500_000_000)
        .await
        .expect("decide_and_file should not error on match");
    assert!(sig.is_none(), "match should NOT trigger a filing");
    assert_eq!(submitter.count(), 0);
}

#[tokio::test]
async fn ev_negative_skips_filing() {
    // Same fraud setup as test 1, but agent stake is tiny — bond + tx fee
    // dwarf the expected payout, so the watcher correctly walks away.
    let canonical_output = b"victim-bytes";
    let bundle = make_bundle(canonical_output);
    let lying_commitment = [0xFFu8; 32];
    let trace_uri = "ar://small-fish";
    let trace_uri_hash: [u8; 32] = blake3::hash(trace_uri.as_bytes()).into();

    let event = make_event(lying_commitment, trace_uri, trace_uri_hash);

    let fetcher = MockFetcher::new();
    fetcher.insert(trace_uri, bundle);

    let bundle_returned = fetcher.fetch(trace_uri, trace_uri_hash).await.unwrap();
    let outcome = ByteCompareVerifier::new().verify(&event, &bundle_returned).await;

    let submitter = MockChallengeSubmitter::new();
    let filer = ChallengeFiler::new(submitter.clone(), BondStrategy::Conservative, 50 * SOL);

    // Agent has 0.001 SOL stake; bond is 1 SOL. EV catastrophically negative.
    let decision = filer.decide(&outcome, 1_000_000, event.thought_pda, SOL);
    assert!(
        matches!(decision, FileDecision::EvNegative { .. }),
        "expected EvNegative, got {decision:?}"
    );

    let sig = filer
        .decide_and_file(&outcome, 1_000_000, event.thought_pda, SOL)
        .await
        .unwrap();
    assert!(sig.is_none());
    assert_eq!(submitter.count(), 0);
}

#[tokio::test]
async fn disabled_strategy_never_files() {
    let canonical_output = b"any-bytes";
    let bundle = make_bundle(canonical_output);
    let lying_commitment = [0xFFu8; 32];
    let trace_uri = "ar://observer-mode";
    let trace_uri_hash: [u8; 32] = blake3::hash(trace_uri.as_bytes()).into();

    let event = make_event(lying_commitment, trace_uri, trace_uri_hash);
    let fetcher = MockFetcher::new();
    fetcher.insert(trace_uri, bundle);
    let bundle_returned = fetcher.fetch(trace_uri, trace_uri_hash).await.unwrap();
    let outcome = ByteCompareVerifier::new().verify(&event, &bundle_returned).await;

    let submitter = MockChallengeSubmitter::new();
    let filer = ChallengeFiler::new(submitter.clone(), BondStrategy::Disabled, u64::MAX);
    let sig = filer
        .decide_and_file(&outcome, 100 * SOL, event.thought_pda, 500_000_000)
        .await
        .unwrap();
    assert!(sig.is_none());
    assert_eq!(submitter.count(), 0);
}

#[tokio::test]
async fn trace_uri_hash_mismatch_is_a_fetch_error() {
    // If the URI in the event was tampered with (different from what was
    // committed on-chain), the fetcher refuses to return a bundle.
    let bundle = make_bundle(b"whatever");
    let trace_uri = "ar://swapped-uri";
    let real_hash: [u8; 32] = blake3::hash(trace_uri.as_bytes()).into();

    let fetcher = MockFetcher::new();
    fetcher.insert(trace_uri, bundle);

    // Caller provides a different expected hash (simulating an event that
    // claimed a different URI than the one that was logged).
    let wrong_hash = [0u8; 32];
    let result = fetcher.fetch(trace_uri, wrong_hash).await;
    assert!(
        result.is_err(),
        "fetcher should reject when URI hash doesn't match"
    );
}
