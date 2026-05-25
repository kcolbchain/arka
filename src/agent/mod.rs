//! Agent — the core abstraction. An autonomous entity with a wallet, chain connection,
//! and modules for DEX, MPP, and oracle interaction.

pub mod account;

use alloy::primitives::{Address, U256};

use crate::chain::{Chain, ChainConnector};
use crate::dex::DexModule;
use crate::error::{ArkaError, Result};
use crate::mpp::MppClient;
use crate::oracle::OracleModule;
use crate::wallet::{EvmWallet, Wallet};

/// An autonomous blockchain agent.
pub struct Agent {
    wallet: Box<dyn Wallet>,
    chain: Chain,
    connector: ChainConnector,
    dex: DexModule,
    mpp: MppClient,
    oracle: OracleModule,
}

impl Agent {
    /// Create a new agent builder.
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Get the agent's wallet address (EVM).
    pub fn address(&self) -> Address {
        // Try to downcast to EvmWallet for EVM address
        if let Some(evm) = self.wallet.downcast_ref::<EvmWallet>() {
            return evm.address();
        }
        Address::ZERO
    }

    /// Get the agent's wallet pubkey string.
    pub fn pubkey(&self) -> String {
        self.wallet.pubkey()
    }

    /// Get the agent's chain.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Get native token balance.
    pub async fn balance(&self) -> Result<U256> {
        self.connector.balance(self.address()).await
    }

    /// Get current block number.
    pub async fn block_number(&self) -> Result<u64> {
        self.connector.block_number().await
    }

    /// Get the agent's nonce.
    pub async fn nonce(&self) -> Result<u64> {
        self.connector.nonce(self.address()).await
    }

    /// Access the DEX module.
    pub fn dex(&self) -> &DexModule {
        &self.dex
    }

    /// Access the MPP client.
    pub fn mpp(&self) -> &MppClient {
        &self.mpp
    }

    /// Access the oracle module.
    pub fn oracle(&self) -> &OracleModule {
        &self.oracle
    }

    /// Access the wallet.
    pub fn wallet(&self) -> &Box<dyn Wallet> {
        &self.wallet
    }

    /// Access the chain connector.
    pub fn connector(&self) -> &ChainConnector {
        &self.connector
    }
}

/// Builder for constructing agents.
#[derive(Default)]
pub struct AgentBuilder {
    wallet: Option<Box<dyn Wallet>>,
    chain: Option<Chain>,
    rpc_url: Option<String>,
}

impl AgentBuilder {
    /// Set the wallet for this agent.
    pub fn wallet(mut self, wallet: Box<dyn Wallet>) -> Self {
        self.wallet = Some(wallet);
        self
    }

    /// Set the chain for this agent.
    pub fn chain(mut self, chain: Chain) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Set a custom RPC URL (overrides chain default).
    pub fn rpc_url(mut self, url: &str) -> Self {
        self.rpc_url = Some(url.to_string());
        self
    }

    /// Build the agent.
    pub async fn build(self) -> Result<Agent> {
        let wallet = self
            .wallet
            .ok_or_else(|| ArkaError::Config("Wallet is required".into()))?;
        let chain = self
            .chain
            .ok_or_else(|| ArkaError::Config("Chain is required".into()))?;

        let connector = match &self.rpc_url {
            Some(url) => ChainConnector::with_rpc(chain, url).await?,
            None => ChainConnector::new(chain).await?,
        };

        let dex = DexModule::new(chain);
        let mpp = MppClient::new();
        let oracle = OracleModule::new(chain);

        Ok(Agent {
            wallet,
            chain,
            connector,
            dex,
            mpp,
            oracle,
        })
    }
}
