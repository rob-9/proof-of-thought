//! Watcher configuration — CLI args parsed by clap, validated, and converted
//! into a [`WatcherConfig`] handed to the pipeline.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// How aggressively the watcher commits bond capital to challenges.
///
/// - `Conservative`: only file when expected payout >= 2x bond + gas
/// - `Aggressive`: file at any EV-positive opportunity
/// - `Disabled`: never file (observer mode — useful for shadow runs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum BondStrategy {
    Conservative,
    Aggressive,
    Disabled,
}

/// Off-chain storage backend the watcher pulls traces from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum StorageBackend {
    Arweave,
    Shadow,
    /// In-memory fixture, used by integration tests only.
    Mock,
}

#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub policies: Vec<String>,
    pub models_dir: PathBuf,
    pub max_stake_at_risk_lamports: u64,
    pub bond_strategy: BondStrategy,
    pub storage: StorageBackend,
    pub keypair_path: PathBuf,
    /// On-chain program ID the watcher subscribes to. Configurable so the
    /// watcher can target devnet vs mainnet vs a local-validator deploy.
    pub program_id: String,
}

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

impl WatcherConfig {
    pub fn new(
        rpc_url: impl Into<String>,
        ws_url: impl Into<String>,
        policies: Vec<String>,
        models_dir: PathBuf,
        max_stake_at_risk_sol: f64,
        bond_strategy: BondStrategy,
        storage: StorageBackend,
        keypair_path: PathBuf,
        program_id: impl Into<String>,
    ) -> Result<Self> {
        let cfg = Self {
            rpc_url: rpc_url.into(),
            ws_url: ws_url.into(),
            policies,
            models_dir,
            max_stake_at_risk_lamports: (max_stake_at_risk_sol * LAMPORTS_PER_SOL as f64) as u64,
            bond_strategy,
            storage,
            keypair_path,
            program_id: program_id.into(),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        if self.rpc_url.is_empty() {
            return Err(anyhow!("--rpc must not be empty"));
        }
        if self.ws_url.is_empty() {
            return Err(anyhow!("--ws must not be empty"));
        }
        if self.policies.is_empty() {
            return Err(anyhow!(
                "--policies must list at least one policy id (comma-separated)"
            ));
        }
        if self.bond_strategy != BondStrategy::Disabled && self.max_stake_at_risk_lamports == 0 {
            return Err(anyhow!(
                "--max-stake-at-risk must be > 0 unless --bond-strategy=disabled"
            ));
        }
        if self.program_id.is_empty() {
            return Err(anyhow!("--program-id must not be empty"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WatcherConfig {
        WatcherConfig::new(
            "http://localhost:8899",
            "ws://localhost:8900",
            vec!["pol_test".into()],
            PathBuf::from("/tmp/models"),
            10.0,
            BondStrategy::Aggressive,
            StorageBackend::Mock,
            PathBuf::from("/tmp/key.json"),
            "PoT11111111111111111111111111111111111111",
        )
        .expect("valid config")
    }

    #[test]
    fn rejects_empty_policies() {
        let res = WatcherConfig::new(
            "http://x",
            "ws://x",
            vec![],
            PathBuf::from("/tmp"),
            1.0,
            BondStrategy::Aggressive,
            StorageBackend::Mock,
            PathBuf::from("/tmp/k"),
            "p",
        );
        assert!(res.is_err());
    }

    #[test]
    fn lamports_conversion() {
        let c = cfg();
        assert_eq!(c.max_stake_at_risk_lamports, 10 * LAMPORTS_PER_SOL);
    }

    #[test]
    fn disabled_strategy_allows_zero_stake_at_risk() {
        let c = WatcherConfig::new(
            "http://x",
            "ws://x",
            vec!["p".into()],
            PathBuf::from("/tmp"),
            0.0,
            BondStrategy::Disabled,
            StorageBackend::Mock,
            PathBuf::from("/tmp/k"),
            "p",
        );
        assert!(c.is_ok());
    }
}
