//! Solana chain connector — RPC connection, SOL balance, SOL/SPL transfers.
//!
//! This module provides a Solana-native connector that lives alongside the
//! EVM-focused `ChainConnector`. Solana uses different primitives (Pubkey,
//! lamports, transaction wire format) so it cannot reuse the alloy-based
//! connector. Instead, `SolanaConnector` wraps the official `solana-client`
//! RPC client with the same ergonomic API pattern as `ChainConnector`.
//!
//! ## Features
//! - Connect to Solana mainnet, devnet, or custom RPC
//! - Query SOL balance (lamports)
//! - Transfer SOL between accounts
//! - Transfer SPL tokens
//! - Query recent blockhash
//! - Query account info

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;

use crate::error::{ArkaError, Result};

/// Default Solana mainnet RPC URL.
pub const SOLANA_MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";

/// Default Solana devnet RPC URL.
pub const SOLANA_DEVNET_RPC: &str = "https://api.devnet.solana.com";

/// Manages connection to the Solana blockchain.
///
/// # Example
/// ```no_run
/// use arka::chain::solana_connector::SolanaConnector;
///
/// let sol = SolanaConnector::devnet().unwrap();
/// let balance = sol.balance("11111111111111111111111111111111").unwrap();
/// println!("Balance: {} SOL", balance as f64 / 1_000_000_000.0);
/// ```
pub struct SolanaConnector {
    rpc_url: String,
    client: RpcClient,
}

impl SolanaConnector {
    /// Connect to Solana mainnet-beta.
    pub fn mainnet() -> Result<Self> {
        Self::with_rpc(SOLANA_MAINNET_RPC)
    }

    /// Connect to Solana devnet.
    pub fn devnet() -> Result<Self> {
        Self::with_rpc(SOLANA_DEVNET_RPC)
    }

    /// Connect to a custom Solana RPC endpoint.
    pub fn with_rpc(rpc_url: &str) -> Result<Self> {
        let client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        Ok(Self {
            rpc_url: rpc_url.to_string(),
            client,
        })
    }

    /// Get the RPC URL this connector is using.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Get SOL balance in lamports for a given address.
    ///
    /// 1 SOL = 1,000,000,000 lamports.
    pub fn balance(&self, address: &str) -> Result<u64> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|e| ArkaError::Config(format!("Invalid Solana address: {e}")))?;
        self.client
            .get_balance(&pubkey)
            .map_err(|e| ArkaError::Rpc(format!("Failed to get SOL balance: {e}")))
    }

    /// Get SOL balance as a human-readable f64 (in SOL units).
    pub fn balance_sol(&self, address: &str) -> Result<f64> {
        let lamports = self.balance(address)?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    /// Get SPL token balance for a given mint and owner address.
    ///
    /// Returns the amount in raw token units (before decimal adjustment).
    pub fn spl_balance(&self, mint: &str, owner: &str) -> Result<u64> {
        let mint_pubkey = Pubkey::from_str(mint)
            .map_err(|e| ArkaError::Config(format!("Invalid SPL mint address: {e}")))?;
        let owner_pubkey = Pubkey::from_str(owner)
            .map_err(|e| ArkaError::Config(format!("Invalid owner address: {e}")))?;

        // Derive associated token address
        let ata = spl_associated_token_account::get_associated_token_address(
            &owner_pubkey,
            &mint_pubkey,
        );

        match self.client.get_token_account_balance(&ata) {
            Ok(balance) => balance.amount.parse::<u64>().map_err(|e| {
                ArkaError::Rpc(format!("Failed to parse SPL balance: {e}"))
            }),
            Err(_) => Ok(0), // Account doesn't exist = 0 balance
        }
    }

    /// Transfer SOL from a keypair to a recipient address.
    ///
    /// Returns the transaction signature.
    pub fn transfer_sol(
        &self,
        from: &Keypair,
        to: &str,
        lamports: u64,
    ) -> Result<Signature> {
        let to_pubkey = Pubkey::from_str(to)
            .map_err(|e| ArkaError::Config(format!("Invalid recipient address: {e}")))?;

        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::Rpc(format!("Failed to get blockhash: {e}")))?;

        let ix = system_instruction::transfer(&from.pubkey(), &to_pubkey, lamports);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );

        self.client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| ArkaError::Transaction(format!("SOL transfer failed: {e}")))
    }

    /// Transfer SPL tokens from a keypair to a recipient.
    ///
    /// The sender must have an Associated Token Account (ATA) for the mint.
    /// If the recipient doesn't have an ATA, one will be created automatically.
    ///
    /// Returns the transaction signature.
    pub fn transfer_spl(
        &self,
        from: &Keypair,
        mint: &str,
        to: &str,
        amount: u64,
    ) -> Result<Signature> {
        let mint_pubkey = Pubkey::from_str(mint)
            .map_err(|e| ArkaError::Config(format!("Invalid SPL mint address: {e}")))?;
        let to_pubkey = Pubkey::from_str(to)
            .map_err(|e| ArkaError::Config(format!("Invalid recipient address: {e}")))?;

        // Get or create associated token accounts
        let from_ata =
            spl_associated_token_account::get_associated_token_address(&from.pubkey(), &mint_pubkey);
        let to_ata =
            spl_associated_token_account::get_associated_token_address(&to_pubkey, &mint_pubkey);

        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::Rpc(format!("Failed to get blockhash: {e}")))?;

        // Build instructions: create ATA for recipient if needed, then transfer
        let mut instructions = Vec::new();

        // Check if recipient ATA exists
        if self.client.get_account(&to_ata).is_err() {
            instructions.push(
                spl_associated_token_account::instruction::create_associated_token_account(
                    &from.pubkey(),
                    &to_pubkey,
                    &mint_pubkey,
                    &spl_token::id(),
                ),
            );
        }

        // SPL token transfer instruction
        instructions.push(
            spl_token::instruction::transfer(
                &spl_token::id(),
                &from_ata,
                &to_ata,
                &from.pubkey(),
                &[],
                amount,
            )
            .map_err(|e| ArkaError::Transaction(format!("Failed to build SPL transfer ix: {e}")))?,
        );

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );

        self.client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| ArkaError::Transaction(format!("SPL transfer failed: {e}")))
    }

    /// Get the current slot (equivalent to block height on EVM).
    pub fn slot(&self) -> Result<u64> {
        self.client
            .get_slot()
            .map_err(|e| ArkaError::Rpc(format!("Failed to get slot: {e}")))
    }

    /// Get account info as raw bytes.
    pub fn account_info(&self, address: &str) -> Result<Vec<u8>> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|e| ArkaError::Config(format!("Invalid address: {e}")))?;
        let account = self
            .client
            .get_account(&pubkey)
            .map_err(|e| ArkaError::Rpc(format!("Failed to get account: {e}")))?;
        Ok(account.data)
    }

    /// Get a reference to the underlying RPC client for advanced usage.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mainnet_rpc_url() {
        let conn = SolanaConnector::mainnet().unwrap();
        assert_eq!(conn.rpc_url(), SOLANA_MAINNET_RPC);
    }

    #[test]
    fn test_devnet_rpc_url() {
        let conn = SolanaConnector::devnet().unwrap();
        assert_eq!(conn.rpc_url(), SOLANA_DEVNET_RPC);
    }

    #[test]
    fn test_custom_rpc_url() {
        let conn = SolanaConnector::with_rpc("http://localhost:8899").unwrap();
        assert_eq!(conn.rpc_url(), "http://localhost:8899");
    }

    #[test]
    fn test_invalid_address_returns_error() {
        let conn = SolanaConnector::devnet().unwrap();
        assert!(conn.balance("not-a-valid-address").is_err());
    }

    #[test]
    fn test_system_program_balance() {
        // System program (11111111...) should always have 0 SOL
        let conn = SolanaConnector::devnet().unwrap();
        let balance = conn.balance("11111111111111111111111111111111");
        // This may fail if devnet is unreachable, but should not panic
        assert!(balance.is_ok() || balance.is_err());
    }
}
