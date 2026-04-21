//! # arka
//!
//! Rust AI agent SDK for blockchain. Chain-agnostic wallets, DEX interaction,
//! MPP payments, on-chain state reading.

pub mod agent;
pub mod chain;
pub mod chains;
pub mod dex;
pub mod mpp;
pub mod oracle;
pub mod tx;
pub mod wallet;

mod error;
pub use error::{ArkaError, Result};

/// Convenience re-exports.
pub mod prelude {
    pub use crate::agent::account::{AgentAccount, InMemoryAgentAccount, TaskReceipt};
    pub use crate::agent::{Agent, AgentBuilder};
    pub use crate::chain::Chain;
    pub use crate::chains::arbitrum::{AgentDepositClient, ArbitrumContracts};
    pub use crate::error::{ArkaError, Result};
    pub use crate::wallet::Wallet;
    pub use alloy::primitives::{Address, U256};
}
