//! Wallet management — key generation, signing, multi-wallet support.

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

use crate::error::{ArkaError, Result};

/// An agent wallet that can sign transactions.
#[derive(Clone)]
pub struct Wallet {
    signer: PrivateKeySigner,
    label: String,
}

impl Wallet {
    /// Generate a new random wallet.
    pub fn generate() -> Result<Self> {
        let signer = PrivateKeySigner::random();
        Ok(Self {
            signer,
            label: String::from("default"),
        })
    }

    /// Import a wallet from a hex-encoded private key.
    pub fn from_private_key(key: &str) -> Result<Self> {
        let key = key.strip_prefix("0x").unwrap_or(key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| ArkaError::Wallet(format!("Invalid private key: {e}")))?;
        Ok(Self {
            signer,
            label: String::from("imported"),
        })
    }

    /// Import from environment variable.
    pub fn from_env(var_name: &str) -> Result<Self> {
        let key = std::env::var(var_name)
            .map_err(|_| ArkaError::Wallet(format!("Environment variable {var_name} not set")))?;
        Self::from_private_key(&key)
    }

    /// Set a label for this wallet.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Get the wallet's address.
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Get the wallet's label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Get a reference to the signer.
    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print private key
        f.debug_struct("Wallet")
            .field("address", &self.address())
            .field("label", &self.label)
            .finish()
    }
}

/// Manages multiple wallets with rotation support.
pub struct WalletManager {
    wallets: Vec<Wallet>,
    current: usize,
}

impl WalletManager {
    pub fn new() -> Self {
        Self {
            wallets: Vec::new(),
            current: 0,
        }
    }

    pub fn add(&mut self, wallet: Wallet) {
        self.wallets.push(wallet);
    }

    /// Get the next wallet in rotation. Named `next_wallet` (not `next`)
    /// to avoid ambiguity with `Iterator::next`.
    pub fn next_wallet(&mut self) -> Option<&Wallet> {
        if self.wallets.is_empty() {
            return None;
        }
        let wallet = &self.wallets[self.current % self.wallets.len()];
        self.current += 1;
        Some(wallet)
    }

    /// Get a wallet by label.
    pub fn by_label(&self, label: &str) -> Option<&Wallet> {
        self.wallets.iter().find(|w| w.label() == label)
    }

    /// Get all wallets.
    pub fn all(&self) -> &[Wallet] {
        &self.wallets
    }

    /// Number of wallets.
    pub fn count(&self) -> usize {
        self.wallets.len()
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}
