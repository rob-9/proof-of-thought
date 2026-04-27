//! `pot-watcher` binary entry point.
//!
//! Wires the actor-style pipeline:
//!
//!   LogStream  →  TraceFetcher  →  Verifier  →  ChallengeFiler
//!     (mpsc)         (mpsc)          (mpsc)
//!
//! In a release build with `--storage arweave`, the live `WebsocketLogStream`
//! + `ArweaveFetcher` + `ByteCompareVerifier` + `RpcChallengeSubmitter` are
//! plumbed. With `--storage mock`, the binary is a smoke test for the
//! pipeline wiring (no RPC, no fraud — useful for verifying tracing /
//! config validation in CI before issuing real RPC subscriptions).

#![deny(unsafe_code)]

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use pot_watcher::{
    challenge::{ChallengeFiler, MockChallengeSubmitter, RpcChallengeSubmitter},
    config::{BondStrategy, StorageBackend, WatcherConfig},
    subscribe::{LogStream, MockLogStream, WebsocketLogStream},
    trace_fetch::{ArweaveFetcher, MockFetcher, ShadowFetcher, TraceBundle, TraceFetcher},
    types::ThoughtSubmittedEvent,
    verify::{ByteCompareVerifier, NoopEngine, StrictVerifier, VerifyOutcome, Verifier},
};

#[derive(Parser, Debug)]
#[command(
    name = "pot-watcher",
    version,
    about = "Proof of Thought watcher daemon"
)]
struct Cli {
    /// Solana JSON-RPC endpoint.
    #[arg(long, default_value = "https://api.devnet.solana.com")]
    rpc: String,

    /// Solana websocket endpoint for log subscription.
    #[arg(long, default_value = "wss://api.devnet.solana.com")]
    ws: String,

    /// Comma-separated list of policy IDs to attend to.
    /// Empty = subscribe to all events.
    #[arg(long, value_delimiter = ',', default_value = "")]
    policies: Vec<String>,

    /// Directory containing model weights for re-execution. Optional;
    /// re-execution is currently a no-op pending vLLM integration.
    #[arg(long, default_value = "/var/lib/pot/models")]
    models: PathBuf,

    /// Maximum SOL the watcher will hold in active bonds at once.
    #[arg(long, default_value_t = 5.0)]
    max_stake_at_risk: f64,

    /// Bond bidding strategy.
    #[arg(long, value_enum, default_value_t = BondStrategy::Conservative)]
    bond_strategy: BondStrategy,

    /// Off-chain storage backend.
    #[arg(long, value_enum, default_value_t = StorageBackend::Arweave)]
    storage: StorageBackend,

    /// Watcher keypair (used to sign challenge transactions).
    #[arg(long, default_value = "~/.config/solana/id.json")]
    keypair: PathBuf,

    /// On-chain PoT program ID.
    #[arg(long, default_value = "Pot1111111111111111111111111111111111111111")]
    program_id: String,

    /// Channel capacity between actors. Higher = more in-flight tolerance,
    /// lower = faster backpressure on slow stages.
    #[arg(long, default_value_t = 256)]
    channel_capacity: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cfg = WatcherConfig::new(
        cli.rpc,
        cli.ws,
        cli.policies,
        cli.models,
        cli.max_stake_at_risk,
        cli.bond_strategy,
        cli.storage,
        cli.keypair,
        cli.program_id.clone(),
    )
    .context("invalid watcher config")?;

    info!(
        rpc = %cfg.rpc_url,
        ws = %cfg.ws_url,
        storage = ?cfg.storage,
        bond = ?cfg.bond_strategy,
        program_id = %cfg.program_id,
        "starting pot-watcher"
    );

    let program_id = Pubkey::from_str(&cfg.program_id)
        .with_context(|| format!("invalid program ID: {}", cfg.program_id))?;

    let (event_tx, event_rx) = mpsc::channel::<ThoughtSubmittedEvent>(cli.channel_capacity);

    // ---- LogStream ----
    let stream: Arc<dyn LogStream> = match cfg.storage {
        StorageBackend::Mock => Arc::new(MockLogStream::new(Vec::new())),
        _ => Arc::new(WebsocketLogStream::new(cfg.ws_url.clone(), program_id)),
    };
    let stream_handle = tokio::spawn({
        let s = Arc::clone(&stream);
        let tx = event_tx.clone();
        async move {
            if let Err(e) = s.run(tx).await {
                error!("log stream exited with error: {e:?}");
            }
        }
    });

    // ---- Fetcher ----
    let fetcher: Arc<dyn TraceFetcher> = match cfg.storage {
        StorageBackend::Arweave => Arc::new(ArweaveFetcher::new()),
        StorageBackend::Shadow => Arc::new(ShadowFetcher::new()),
        StorageBackend::Mock => Arc::new(MockFetcher::new()),
    };

    // ---- Verifier ----
    // MVP: only the byte-compare strict path is wired. Soft + Attested are
    // composed downstream by policy lookup; that's the next milestone.
    let byte_compare = Arc::new(ByteCompareVerifier::new());
    let strict = Arc::new(StrictVerifier::new(NoopEngine));

    // ---- Filer ----
    // Real submitter is stubbed pending program IDL merge; in the meantime
    // a Mock submitter drops decisions as audit log entries.
    let filer = if matches!(cfg.bond_strategy, BondStrategy::Disabled) {
        warn!("bond strategy is Disabled — running as observer (no challenges filed)");
        Arc::new(ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            cfg.bond_strategy,
            cfg.max_stake_at_risk_lamports,
        ))
    } else {
        // Wrap the Mock submitter for now; swap to RpcChallengeSubmitter
        // once ChallengeSubmitter::submit is fully implemented.
        let _real = RpcChallengeSubmitter {
            program_id,
            rpc_url: cfg.rpc_url.clone(),
        };
        Arc::new(ChallengeFiler::new(
            MockChallengeSubmitter::new(),
            cfg.bond_strategy,
            cfg.max_stake_at_risk_lamports,
        ))
    };

    // ---- Pipeline driver ----
    let pipeline = run_pipeline(event_rx, fetcher, byte_compare, strict, filer);

    tokio::select! {
        _ = pipeline => warn!("pipeline exited"),
        _ = stream_handle => warn!("log stream task exited"),
        _ = tokio::signal::ctrl_c() => info!("shutdown signal received"),
    }

    Ok(())
}

/// Drive events through the verify+file stages.
///
/// Each event spawns a per-thought async task so a slow trace fetch doesn't
/// stall the whole pipeline. The stage functions are passed by `Arc` so the
/// task can hold them across `.await` boundaries.
async fn run_pipeline(
    mut event_rx: mpsc::Receiver<ThoughtSubmittedEvent>,
    fetcher: Arc<dyn TraceFetcher>,
    byte_compare: Arc<ByteCompareVerifier>,
    _strict: Arc<StrictVerifier<NoopEngine>>,
    filer: Arc<
        ChallengeFiler<MockChallengeSubmitter>,
    >,
) {
    while let Some(event) = event_rx.recv().await {
        let f = Arc::clone(&fetcher);
        let bc = Arc::clone(&byte_compare);
        let fl = Arc::clone(&filer);
        tokio::spawn(async move { handle_event(event, f, bc, fl).await });
    }
}

async fn handle_event(
    event: ThoughtSubmittedEvent,
    fetcher: Arc<dyn TraceFetcher>,
    verifier: Arc<ByteCompareVerifier>,
    filer: Arc<ChallengeFiler<MockChallengeSubmitter>>,
) {
    let span = tracing::info_span!(
        "thought",
        thought_pda = %event.thought_pda,
        agent = %event.agent,
    );
    let _enter = span.enter();

    let bundle: TraceBundle = match fetcher.fetch(&event.trace_uri, event.trace_uri_hash).await {
        Ok(b) => b,
        Err(e) => {
            warn!("trace fetch failed: {e:?}");
            return;
        }
    };

    let outcome = verifier.verify(&event, &bundle).await;
    match &outcome {
        VerifyOutcome::Match => info!("verified: match"),
        VerifyOutcome::Mismatch { claim, .. } => warn!(?claim, "verified: mismatch"),
        VerifyOutcome::Inconclusive { reason } => info!(%reason, "verified: inconclusive"),
    }

    // The MVP doesn't fetch live agent stake; use a placeholder until the
    // RPC account-fetch path lands. Real flow: `solana_client::rpc_client::
    // RpcClient::get_account(stake_vault_pda)` and decode lamports.
    const PLACEHOLDER_AGENT_STAKE_LAMPORTS: u64 = 100_000_000_000; // 100 SOL
    const PLACEHOLDER_BOND_MIN: u64 = 500_000_000; // 0.5 SOL

    if let Err(e) = filer
        .decide_and_file(
            &outcome,
            PLACEHOLDER_AGENT_STAKE_LAMPORTS,
            event.thought_pda,
            PLACEHOLDER_BOND_MIN,
        )
        .await
    {
        error!("challenge filing failed: {e:?}");
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
