//! Wallet management — chain-agnostic signing trait with EVM and Solana implementations.

use alloy::primitives::{Address, FixedBytes};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use async_trait::async_trait;

use crate::error::{ArkaError, Result};

/// A chain-agnostic wallet that can sign messages and transactions.
#[async_trait]
pub trait Wallet: Send + Sync {
    /// Get the wallet's public key / address as a hex string.
    fn pubkey(&self) -> String;

    /// Sign an arbitrary message.
    async fn sign_message(&self, msg: &[u8]) -> Result<Vec<u8>>;

    /// Sign a raw transaction.
    async fn sign_transaction(&self, tx_data: &[u8]) -> Result<Vec<u8>>;

    /// Get a human-readable label for this wallet.
    fn label(&self) -> &str;

    /// Which chain family this wallet is for.
    fn chain_family(&self) -> &str;
}

/// EVM wallet using an alloy PrivateKeySigner.
#[derive(Clone)]
pub struct EvmWallet {
    signer: PrivateKeySigner,
    label: String,
}

impl EvmWallet {
    pub fn generate() -> Result<Self> {
        let signer = PrivateKeySigner::random();
        Ok(Self { signer, label: String::from("default") })
    }

    pub fn from_private_key(key: &str) -> Result<Self> {
        let key = key.strip_prefix("0x").unwrap_or(key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| ArkaError::Wallet(format!("Invalid EVM private key: {e}")))?;
        Ok(Self { signer, label: String::from("imported") })
    }

    pub fn from_env(var_name: &str) -> Result<Self> {
        let key = std::env::var(var_name)
            .map_err(|_| ArkaError::Wallet(format!("Environment variable {var_name} not set")))?;
        Self::from_private_key(&key)
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    pub fn address(&self) -> Address {
        self.signer.address()
    }

    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }
}

#[async_trait]
impl Wallet for EvmWallet {
    fn pubkey(&self) -> String {
        self.signer.address().to_string()
    }

    async fn sign_message(&self, msg: &[u8]) -> Result<Vec<u8>> {
        let sig = self.signer.sign_message(msg).await
            .map_err(|e| ArkaError::Wallet(format!("EVM sign_message failed: {e}")))?;
        Ok(sig.as_bytes().to_vec())
    }

    async fn sign_transaction(&self, tx_data: &[u8]) -> Result<Vec<u8>> {
        let hash = alloy::primitives::keccak256(tx_data);
        let sig = self.signer.sign_hash(&hash).await
            .map_err(|e| ArkaError::Wallet(format!("EVM sign_transaction failed: {e}")))?;
        Ok(sig.as_bytes().to_vec())
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn chain_family(&self) -> &str {
        "evm"
    }
}

impl std::fmt::Debug for EvmWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmWallet")
            .field("address", &self.address())
            .field("label", &self.label)
            .finish()
    }
}

#[cfg(feature = "solana")]
pub mod solana;

/// Manages multiple wallets with rotation support.
pub struct WalletManager {
    wallets: Vec<Box<dyn Wallet>>,
    current: usize,
}

impl WalletManager {
    pub fn new() -> Self {
        Self { wallets: Vec::new(), current: 0 }
    }

    pub fn add(&mut self, wallet: Box<dyn Wallet>) {
        self.wallets.push(wallet);
    }

    pub fn next_wallet(&mut self) -> Option<&Box<dyn Wallet>> {
        if self.wallets.is_empty() {
            return None;
        }
        let wallet = &self.wallets[self.current % self.wallets.len()];
        self.current += 1;
        Some(wallet)
    }

    pub fn by_label(&self, label: &str) -> Option<&Box<dyn Wallet>> {
        self.wallets.iter().find(|w| w.label() == label)
    }

    pub fn all(&self) -> &[Box<dyn Wallet>] {
        &self.wallets
    }

    pub fn count(&self) -> usize {
        self.wallets.len()
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}
