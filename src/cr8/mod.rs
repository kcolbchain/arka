// arka::cr8 — CR8Client trait, builder, errors per create-protocol/cr8/specs/arka-cr8-client.md
use crate::error::ArkaError;
use crate::wallet::switchboard::SwitchboardWallet;
use crate::chain::Chain;
use std::collections::HashMap;

/// CR8 protocol error types (spec §3)
#[derive(Debug, thiserror::Error)]
pub enum CR8Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("contract call failed: {0}")]
    ContractError(String),
    #[error("insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    #[error("recovery failed: {0}")]
    RecoveryError(String),
    #[error("chain error: {0}")]
    ChainError(#[from] ArkaError),
    #[error("unknown CR8 error")]
    Unknown,
}

/// Core CR8 client trait (spec §2)
#[async_trait::async_trait]
pub trait CR8Client: Send + Sync {
    async fn register(&self) -> Result<(), CR8Error>;
    async fn deposit(&self, amount: u64) -> Result<(), CR8Error>;
    async fn withdraw(&self, amount: u64) -> Result<(), CR8Error>;
    async fn claim(&self) -> Result<u64, CR8Error>;
    async fn complete(&self) -> Result<(), CR8Error>;
    async fn balance(&self) -> Result<u64, CR8Error>;
    async fn watch<F>(&self, callback: F) -> Result<(), CR8Error>
    where
        F: Fn(u64) + Send + Sync + 'static;
}

/// Recovery trait for CR8 clients (spec §2.5)
#[async_trait::async_trait]
pub trait CR8ClientRecovery: CR8Client {
    async fn recover_state(&self) -> Result<HashMap<String, u64>, CR8Error>;
    async fn force_complete(&self) -> Result<(), CR8Error>;
}

/// Builder for constructing CR8 clients (spec §2)
pub struct CR8ClientBuilder {
    chain: Option<Chain>,
    wallet: Option<SwitchboardWallet>,
    contract_address: Option<String>,
}

impl CR8ClientBuilder {
    pub fn new() -> Self {
        Self {
            chain: None,
            wallet: None,
            contract_address: None,
        }
    }

    pub fn with_chain(mut self, chain: Chain) -> Self {
        self.chain = Some(chain);
        self
    }

    pub fn with_wallet(mut self, wallet: SwitchboardWallet) -> Self {
        self.wallet = Some(wallet);
        self
    }

    pub fn with_contract_address(mut self, address: &str) -> Self {
        self.contract_address = Some(address.to_string());
        self
    }

    /// Build a default CR8 client implementation
    pub fn build(self) -> Result<DefaultCR8Client, CR8Error> {
        let chain = self.chain.ok_or(CR8Error::InvalidArgument("chain required".into()))?;
        let wallet = self.wallet.ok_or(CR8Error::InvalidArgument("wallet required".into()))?;
        let contract_address = self.contract_address
            .ok_or(CR8Error::InvalidArgument("contract address required".into()))?;

        Ok(DefaultCR8Client {
            chain,
            wallet,
            contract_address,
        })
    }
}

/// Default implementation of CR8Client
pub struct DefaultCR8Client {
    chain: Chain,
    wallet: SwitchboardWallet,
    contract_address: String,
}

#[async_trait::async_trait]
impl CR8Client for DefaultCR8Client {
    async fn register(&self) -> Result<(), CR8Error> {
        // TODO: actual contract call
        Ok(())
    }

    async fn deposit(&self, amount: u64) -> Result<(), CR8Error> {
        // TODO: actual contract call
        Ok(())
    }

    async fn withdraw(&self, amount: u64) -> Result<(), CR8Error> {
        // TODO: actual contract call
        Ok(())
    }

    async fn claim(&self) -> Result<u64, CR8Error> {
        // TODO: actual contract call
        Ok(0)
    }

    async fn complete(&self) -> Result<(), CR8Error> {
        // TODO: actual contract call
        Ok(())
    }

    async fn balance(&self) -> Result<u64, CR8Error> {
        // TODO: actual contract call
        Ok(0)
    }

    async fn watch<F>(&self, _callback: F) -> Result<(), CR8Error>
    where
        F: Fn(u64) + Send + Sync + 'static,
    {
        // TODO: event listener
        Ok(())
    }
}

#[async_trait::async_trait]
impl CR8ClientRecovery for DefaultCR8Client {
    async fn recover_state(&self) -> Result<HashMap<String, u64>, CR8Error> {
        // TODO: recovery logic
        Ok(HashMap::new())
    }

    async fn force_complete(&self) -> Result<(), CR8Error> {
        // TODO: force completion
        Ok(())
    }
}

#[cfg(test)]
mod tests;
