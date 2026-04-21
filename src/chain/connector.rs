//! Chain connector — manages RPC connections and provider state.

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};

use super::Chain;
use crate::error::{ArkaError, Result};

/// Alloy's default filled provider — the concrete type returned by
/// `ProviderBuilder::new().connect_http(...)`. Aliased here to keep
/// the public struct signature readable (and clippy quiet).
type DefaultFilledProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    alloy::providers::RootProvider,
>;

/// Manages connection to a specific chain.
pub struct ChainConnector {
    chain: Chain,
    rpc_url: String,
    provider: DefaultFilledProvider,
}

impl ChainConnector {
    /// Create a new connector with the default RPC.
    pub async fn new(chain: Chain) -> Result<Self> {
        Self::with_rpc(chain, chain.default_rpc()).await
    }

    /// Create a new connector with a custom RPC URL.
    pub async fn with_rpc(chain: Chain, rpc_url: &str) -> Result<Self> {
        let url: reqwest::Url = rpc_url
            .parse()
            .map_err(|e| ArkaError::Config(format!("Invalid RPC URL: {e}")))?;
        let provider = ProviderBuilder::new().connect_http(url);

        Ok(Self {
            chain,
            rpc_url: rpc_url.to_string(),
            provider,
        })
    }

    /// Get native token balance for an address.
    pub async fn balance(&self, address: Address) -> Result<U256> {
        self.provider
            .get_balance(address)
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get balance: {e}")))
    }

    /// Get current block number.
    pub async fn block_number(&self) -> Result<u64> {
        self.provider
            .get_block_number()
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get block number: {e}")))
    }

    /// Get transaction count (nonce) for an address.
    pub async fn nonce(&self, address: Address) -> Result<u64> {
        self.provider
            .get_transaction_count(address)
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get nonce: {e}")))
    }

    /// Get the chain this connector is for.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Get the RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Get a reference to the underlying alloy provider (for contract calls).
    pub fn provider(&self) -> &DefaultFilledProvider {
        &self.provider
    }
}
