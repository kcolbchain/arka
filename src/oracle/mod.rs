//! Oracle module — price feeds and real-world data for agent decisions.
//!
//! Supports Chainlink price feeds and on-chain TWAP from DEX pools.

use alloy::primitives::Address;
use alloy::sol;

use crate::chain::Chain;
use crate::error::{ArkaError, Result};

// Chainlink AggregatorV3Interface
sol! {
    #[derive(Debug)]
    function latestRoundData()
        external
        view
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        );
}

/// Well-known Chainlink price feed addresses.
fn chainlink_feed(chain: Chain, pair: &str) -> Option<&'static str> {
    match (chain, pair) {
        (Chain::Ethereum, "ETH/USD") => Some("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
        (Chain::Ethereum, "BTC/USD") => Some("0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c"),
        (Chain::Arbitrum, "ETH/USD") => Some("0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612"),
        (Chain::Optimism, "ETH/USD") => Some("0x13e3Ee699D1909E989722E753853AE30b17e08c5"),
        (Chain::Base, "ETH/USD") => Some("0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"),
        (Chain::Polygon, "ETH/USD") => Some("0xF9680D99D6C9589e2a93a78A04A279e509205945"),
        (Chain::Avalanche, "ETH/USD") => Some("0x976B3D034E162d8bD72D6b9C989d545b839003b0"),
        _ => None,
    }
}

/// Oracle module for an agent.
pub struct OracleModule {
    chain: Chain,
}

impl OracleModule {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }

    /// Get the chain this oracle operates on.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Get the Chainlink feed address for a given pair on this chain.
    pub fn feed_address(&self, pair: &str) -> Result<Address> {
        let addr_str = chainlink_feed(self.chain, pair).ok_or_else(|| {
            ArkaError::Oracle(format!("No Chainlink feed for {pair} on {}", self.chain))
        })?;
        addr_str
            .parse()
            .map_err(|e| ArkaError::Oracle(format!("Invalid feed address: {e}")))
    }

    /// Check if a Chainlink feed exists for a pair on this chain.
    pub fn has_feed(&self, pair: &str) -> bool {
        chainlink_feed(self.chain, pair).is_some()
    }

    /// List available feeds for this chain.
    pub fn available_feeds(&self) -> Vec<&'static str> {
        let pairs = ["ETH/USD", "BTC/USD", "USDC/USD", "DAI/USD", "LINK/USD"];
        pairs
            .into_iter()
            .filter(|p| chainlink_feed(self.chain, p).is_some())
            .collect()
    }
}
