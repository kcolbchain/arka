//! DEX types — swap parameters, results, fee tiers.

use alloy::primitives::U256;
use serde::{Deserialize, Serialize};

use crate::chain::Chain;

/// Uniswap V3 fee tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeeTier {
    /// 0.01% — stablecoin pairs
    Lowest = 100,
    /// 0.05% — correlated pairs
    Low = 500,
    /// 0.3% — standard pairs
    Medium = 3000,
    /// 1% — exotic pairs
    High = 10000,
}

impl FeeTier {
    pub fn as_u24(&self) -> u32 {
        *self as u32
    }
}

/// Parameters for a swap (built by SwapBuilder, executed by router).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapParams {
    pub chain: Chain,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: U256,
    pub slippage_bps: u16,
    pub fee_tier: FeeTier,
    pub deadline_secs: u64,
}

/// Result of an executed swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResult {
    pub tx_hash: String,
    pub chain: Chain,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: U256,
    pub amount_out: U256,
    pub gas_used: u64,
    pub effective_price: f64,
    pub slippage_actual_bps: u16,
}

/// Quote result (no execution, just price estimation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResult {
    pub chain: Chain,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: U256,
    pub amount_out: U256,
    pub price: f64,
    pub fee_tier: FeeTier,
    pub price_impact_bps: u16,
}

/// Well-known token addresses per chain.
pub struct Tokens;

impl Tokens {
    pub fn weth(chain: Chain) -> &'static str {
        match chain {
            Chain::Ethereum => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            Chain::Arbitrum => "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
            Chain::Optimism => "0x4200000000000000000000000000000000000006",
            Chain::Base => "0x4200000000000000000000000000000000000006",
            Chain::Avalanche => "0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB", // WETH.e
            Chain::Polygon => "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619",
            _ => "0x0000000000000000000000000000000000000000",
        }
    }

    pub fn usdc(chain: Chain) -> &'static str {
        match chain {
            Chain::Ethereum => "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            Chain::Arbitrum => "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
            Chain::Optimism => "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85",
            Chain::Base => "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            Chain::Avalanche => "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E",
            Chain::Polygon => "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
            Chain::Tempo | Chain::TempoTestnet => "0x0000000000000000000000000000000000000001", // native USDC on Tempo
            _ => "0x0000000000000000000000000000000000000000",
        }
    }

    pub fn usdt(chain: Chain) -> &'static str {
        match chain {
            Chain::Ethereum => "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            Chain::Arbitrum => "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9",
            Chain::Optimism => "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58",
            Chain::Polygon => "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
            _ => "0x0000000000000000000000000000000000000000",
        }
    }
}
