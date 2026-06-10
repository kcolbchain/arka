//! Chain definitions and RPC management.

use serde::{Deserialize, Serialize};
use std::fmt;

mod connector;
pub use connector::ChainConnector;

pub mod solana;
pub use solana::{SolanaConfig, SolanaConnector, CommitmentLevel, constants, tokens};

/// Supported blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chain {
    Ethereum,
    Arbitrum,
    Optimism,
    Base,
    Avalanche,
    Polygon,
    Bsc,
    Tempo,
    TempoTestnet,
    Solana,
}

impl Chain {
    /// Chain ID for EVM networks or Solana.
    pub fn chain_id(&self) -> u64 {
        match self {
            Chain::Ethereum => 1,
            Chain::Arbitrum => 42161,
            Chain::Optimism => 10,
            Chain::Base => 8453,
            Chain::Avalanche => 43114,
            Chain::Polygon => 137,
            Chain::Bsc => 56,
            Chain::Tempo => 4217,
            Chain::TempoTestnet => 42429,
            Chain::Solana => 101, // Solana Mainnet
        }
    }

    /// Default public RPC URL.
    pub fn default_rpc(&self) -> &'static str {
        match self {
            Chain::Ethereum => "https://eth.llamarpc.com",
            Chain::Arbitrum => "https://arb1.arbitrum.io/rpc",
            Chain::Optimism => "https://mainnet.optimism.io",
            Chain::Base => "https://mainnet.base.org",
            Chain::Avalanche => "https://api.avax.network/ext/bc/C/rpc",
            Chain::Polygon => "https://polygon-rpc.com",
            Chain::Bsc => "https://bsc-dataseed.binance.org",
            Chain::Tempo => "https://rpc.tempo.xyz",
            Chain::TempoTestnet => "https://rpc.testnet.tempo.xyz",
            Chain::Solana => "https://api.mainnet-beta.solana.com",
        }
    }

    /// Native token symbol.
    pub fn native_token(&self) -> &'static str {
        match self {
            Chain::Ethereum | Chain::Arbitrum | Chain::Optimism | Chain::Base => "ETH",
            Chain::Avalanche => "AVAX",
            Chain::Polygon => "MATIC",
            Chain::Bsc => "BNB",
            Chain::Tempo | Chain::TempoTestnet => "USDC", // Tempo has no native gas token
            Chain::Solana => "SOL",
        }
    }

    /// Whether this chain uses stablecoins for gas (Tempo).
    pub fn stablecoin_gas(&self) -> bool {
        matches!(self, Chain::Tempo | Chain::TempoTestnet)
    }

    /// Block explorer base URL.
    pub fn explorer(&self) -> &'static str {
        match self {
            Chain::Ethereum => "https://etherscan.io",
            Chain::Arbitrum => "https://arbiscan.io",
            Chain::Optimism => "https://optimistic.etherscan.io",
            Chain::Base => "https://basescan.org",
            Chain::Avalanche => "https://snowtrace.io",
            Chain::Polygon => "https://polygonscan.com",
            Chain::Bsc => "https://bscscan.com",
            Chain::Tempo => "https://explorer.tempo.xyz",
            Chain::TempoTestnet => "https://explorer.testnet.tempo.xyz",
            Chain::Solana => "https://solscan.io",
        }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Chain::Ethereum => write!(f, "ethereum"),
            Chain::Arbitrum => write!(f, "arbitrum"),
            Chain::Optimism => write!(f, "optimism"),
            Chain::Base => write!(f, "base"),
            Chain::Avalanche => write!(f, "avalanche"),
            Chain::Polygon => write!(f, "polygon"),
            Chain::Bsc => write!(f, "bsc"),
            Chain::Tempo => write!(f, "tempo"),
            Chain::TempoTestnet => write!(f, "tempo-testnet"),
            Chain::Solana => write!(f, "solana"),
        }
    }
}
