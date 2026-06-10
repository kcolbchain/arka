//! Solana chain module — connection, balance checks, SOL and SPL token transfers.
//!
//! This module provides Solana-specific functionality for the arka SDK.
//! Unlike EVM chains that use `alloy`, Solana uses its own SDK with
//! the account model and Program Derived Addresses (PDAs).
//!
//! ## Features
//! - Connection to Solana RPC endpoints
//! - SOL balance checks and transfers
//! - SPL token balance checks and transfers
//! - Support for test validator

use solana_client::client_error::ClientError;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_token::instruction::transfer_checked;
use std::str::FromStr;

use crate::error::{ArkaError, Result};

/// Solana chain configuration and constants.
pub struct SolanaConfig {
    /// RPC endpoint URL.
    pub rpc_url: String,
    /// Commitment level for RPC requests.
    pub commitment: CommitmentLevel,
}

/// Solana commitment levels.
#[derive(Debug, Clone, Copy)]
pub enum CommitmentLevel {
    Processed,
    Confirmed,
    Finalized,
}

impl Default for CommitmentLevel {
    fn default() -> Self {
        CommitmentLevel::Finalized
    }
}

impl From<CommitmentLevel> for solana_sdk::commitment_config::CommitmentConfig {
    fn from(level: CommitmentLevel) -> Self {
        match level {
            CommitmentLevel::Processed => {
                solana_sdk::commitment_config::CommitmentConfig::processed()
            }
            CommitmentLevel::Confirmed => {
                solana_sdk::commitment_config::CommitmentConfig::confirmed()
            }
            CommitmentLevel::Finalized => {
                solana_sdk::commitment_config::CommitmentConfig::finalized()
            }
        }
    }
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            commitment: CommitmentLevel::default(),
        }
    }
}

/// Solana chain connector for RPC operations.
///
/// Provides methods for querying and interacting with the Solana blockchain.
#[derive(Debug, Clone)]
pub struct SolanaConnector {
    rpc_client: RpcClient,
    config: SolanaConfig,
}

impl SolanaConnector {
    /// Create a new connector with the default mainnet RPC.
    pub fn new() -> Self {
        Self::with_rpc(SolanaConfig::default())
    }

    /// Create a new connector with a custom RPC URL.
    pub fn with_url(rpc_url: &str) -> Result<Self> {
        let config = SolanaConfig {
            rpc_url: rpc_url.to_string(),
            commitment: CommitmentLevel::default(),
        };
        Self::with_rpc(config)
    }

    /// Create a new connector with custom configuration.
    pub fn with_rpc(config: SolanaConfig) -> Result<Self> {
        let commitment: solana_sdk::commitment_config::CommitmentConfig =
            config.commitment.into();
        let rpc_client = RpcClient::new_with_commitment(&config.rpc_url, commitment);

        Ok(Self {
            rpc_client,
            config,
        })
    }

    /// Get the configured RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.config.rpc_url
    }

    /// Get SOL balance for a wallet address.
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|e| ArkaError::Config(format!("Invalid Solana address: {e}")))?;

        self.rpc_client
            .get_balance(&pubkey)
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get SOL balance: {e}")))
    }

    /// Get SOL balance in lamports (smallest unit).
    pub async fn get_lamports(&self, address: &str) -> Result<u64> {
        self.get_balance(address).await
    }

    /// Get the current slot number.
    pub async fn get_slot(&self) -> Result<u64> {
        self.rpc_client
            .get_slot()
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get slot: {e}")))
    }

    /// Get the current block height.
    pub async fn get_block_height(&self) -> Result<u64> {
        self.rpc_client
            .get_block_height()
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get block height: {e}")))
    }

    /// Check if an address exists on-chain (has been initialized).
    pub async fn account_exists(&self, address: &str) -> Result<bool> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|e| ArkaError::Config(format!("Invalid Solana address: {e}")))?;

        self.rpc_client
            .get_account(&pubkey)
            .await
            .map(|_| true)
            .map_err(|e| {
                if let ClientError::TransportError(_) = e {
                    ArkaError::Rpc(format!("RPC connection error: {e}"))
                } else {
                    ArkaError::Rpc(format!("Failed to check account: {e}"))
                }
            })
            .or_else(|e| {
                if e.to_string().contains("Account not found") {
                    Ok(false)
                } else {
                    Err(e)
                }
            })
    }

    /// Get SPL token balance for a wallet and mint.
    pub async fn get_token_balance(&self, wallet: &str, mint: &str) -> Result<u64> {
        let wallet_pubkey = Pubkey::from_str(wallet)
            .map_err(|e| ArkaError::Config(format!("Invalid wallet address: {e}")))?;
        let mint_pubkey = Pubkey::from_str(mint)
            .map_err(|e| ArkaError::Config(format!("Invalid mint address: {e}")))?;

        let token_accounts = self
            .rpc_client
            .get_token_accounts_by_owner(&wallet_pubkey, &spl_token::state::AccountType)
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get token accounts: {e}")))?;

        for account in token_accounts {
            if let Some(mint_str) = account.account.data.parsed.as_ref() {
                if let Some(account_mint) = mint_str.get("mint").and_then(|v| v.as_str()) {
                    if account_mint == mint {
                        if let Some(token_amount) = mint_str.get("tokenAmount").and_then(|v| v.get("amount"))
                        {
                            if let Some(amount_str) = token_amount.as_str() {
                                return Ok(u64::from_str(amount_str).unwrap_or(0));
                            }
                        }
                    }
                }
            }
        }

        Ok(0)
    }

    /// Build a SOL transfer instruction.
    pub fn build_transfer_ix(
        from: &Pubkey,
        to: &Pubkey,
        lamports: u64,
    ) -> Instruction {
        system_instruction::transfer(from, to, lamports)
    }

    /// Build an SPL token transfer instruction.
    pub fn build_token_transfer_ix(
        source: &Pubkey,
        mint: &Pubkey,
        destination: &Pubkey,
        owner: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Result<Instruction> {
        Ok(transfer_checked(
            &spl_token::id(),
            source,
            mint,
            destination,
            owner,
            &[],
            amount,
            decimals,
        ))
    }

    /// Send a signed transaction and return the signature.
    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<String> {
        self.rpc_client
            .send_and_confirm_transaction(transaction)
            .await
            .map(|sig| sig.to_string())
            .map_err(|e| ArkaError::Transaction(format!("Failed to send transaction: {e}")))
    }

    /// Airdrop SOL to a wallet (devnet/testnet only).
    #[allow(dead_code)]
    pub async fn airdrop(&self, address: &str, lamports: u64) -> Result<String> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|e| ArkaError::Config(format!("Invalid Solana address: {e}")))?;

        self.rpc_client
            .request_airdrop(&pubkey, lamports)
            .await
            .map(|sig| sig.to_string())
            .map_err(|e| ArkaError::Transaction(format!("Airdrop failed: {e}")))
    }
}

impl Default for SolanaConnector {
    fn default() -> Self {
        Self::new().expect("Failed to create default Solana connector")
    }
}

/// Well-known Solana token mints.
pub mod tokens {
    /// USDC on Solana (Circle)
    pub const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    /// USDT on Solana
    pub const USDT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

    /// Wrapped SOL (Solana's WETH equivalent)
    pub const WSOL: &str = "So11111111111111111111111111111111111111112";
}

/// Solana mainnet constants.
pub mod constants {
    use super::*;

    /// Solana mainnet cluster RPC.
    pub const MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";

    /// Solana devnet cluster RPC.
    pub const DEVNET_RPC: &str = "https://api.devnet.solana.com";

    /// Solana testnet cluster RPC.
    pub const TESTNET_RPC: &str = "https://api.testnet.solana.com";

    /// Local validator RPC (for testing).
    pub const LOCAL_RPC: &str = "http://127.0.0.1:8899";

    /// Create a mainnet connector.
    pub fn mainnet() -> Result<SolanaConnector> {
        SolanaConnector::with_url(MAINNET_RPC)
    }

    /// Create a devnet connector.
    pub fn devnet() -> Result<SolanaConnector> {
        SolanaConnector::with_url(DEVNET_RPC)
    }

    /// Create a testnet connector.
    pub fn testnet() -> Result<SolanaConnector> {
        SolanaConnector::with_url(TESTNET_RPC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ADDRESS: &str = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTspzyhU6MrLuAFbaf";

    #[tokio::test]
    async fn test_get_balance_mainnet() {
        let connector = SolanaConnector::new();
        let result = connector.get_balance(TEST_ADDRESS).await;
        // Mainnet should return a valid balance
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_slot() {
        let connector = SolanaConnector::new();
        let slot = connector.get_slot().await.unwrap();
        // Solana slot should be a large number
        assert!(slot > 100_000_000);
    }

    #[tokio::test]
    async fn test_account_exists() {
        let connector = SolanaConnector::new();
        // Valid Solana address
        let exists = connector.account_exists(TEST_ADDRESS).await.unwrap();
        assert!(exists);
    }

    #[tokio::test]
    async fn test_invalid_address() {
        let connector = SolanaConnector::new();
        let result = connector.get_balance("invalid").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_token_mints() {
        // Verify well-known token mints
        assert_eq!(tokens::USDC.len(), 44);
        assert_eq!(tokens::USDT.len(), 44);
    }

    #[test]
    fn test_constants() {
        assert!(constants::MAINNET_RPC.contains("solana.com"));
        assert!(constants::DEVNET_RPC.contains("devnet"));
    }
}
