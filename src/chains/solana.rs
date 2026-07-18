//! Solana chain primitives - connection, balance check, SOL transfer,
//! and SPL token transfer support.
//!
//! Solana is a high-performance blockchain with fast finality and low fees,
//! making it suitable for agent-based transactions and DeFi operations.
//!
//! ## What this module provides
//! - `SolanaChain` - chain connection and RPC client
//! - `SolanaClient` - typed client for SOL and SPL token operations
//! - Well-known program IDs (System Program, Token Program, Associated Token)
//!
//! ## Usage
//! ```rust
//! use arka::chains::solana::{SolanaChain, SolanaClient};
//!
//! let chain = SolanaChain::new("https://api.mainnet-beta.solana.com")?;
//! let balance = chain.get_sol_balance(&pubkey).await?;
//! ```

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address,
    instruction::create_associated_token_account,
};
use spl_token::instruction as token_instruction;
use std::str::FromStr;

use crate::chain::Chain;
use crate::error::{ArkaError, Result};

/// Solana mainnet chain ID.
pub const SOLANA_MAINNET_CHAIN_ID: u64 = 101;

/// Solana devnet chain ID.
pub const SOLANA_DEVNET_CHAIN_ID: u64 = 102;

/// Solana testnet chain ID.
pub const SOLANA_TESTNET_CHAIN_ID: u64 = 103;

/// Well-known program IDs on Solana.
pub struct SolanaPrograms;

impl SolanaPrograms {
    /// System Program - handles SOL transfers and account creation.
    pub const SYSTEM_PROGRAM: &'static str = "11111111111111111111111111111111";

    /// Token Program - handles SPL token operations.
    pub const TOKEN_PROGRAM: &'static str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    /// Associated Token Account Program.
    pub const ASSOCIATED_TOKEN_PROGRAM: &'static str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

    /// Memo Program.
    pub const MEMO_PROGRAM: &'static str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
}

/// Solana chain connection wrapper.
#[derive(Debug, Clone)]
pub struct SolanaChain {
    client: RpcClient,
    commitment: CommitmentConfig,
}

impl SolanaChain {
    /// Create a new Solana chain connection.
    ///
    /// # Arguments
    /// * `rpc_url` - Solana RPC endpoint URL
    ///
    /// # Example
    /// ```rust
    /// let chain = SolanaChain::new("https://api.mainnet-beta.solana.com")?;
    /// ```
    pub fn new(rpc_url: &str) -> Result<Self> {
        let commitment = CommitmentConfig::confirmed();
        let client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);

        Ok(Self {
            client,
            commitment,
        })
    }

    /// Create a new Solana chain connection with custom commitment level.
    pub fn with_commitment(rpc_url: &str, commitment: CommitmentConfig) -> Result<Self> {
        let client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);

        Ok(Self {
            client,
            commitment,
        })
    }

    /// Get SOL balance for an address.
    ///
    /// # Arguments
    /// * `address` - Solana public key
    ///
    /// # Returns
    /// Balance in lamports (1 SOL = 1,000,000,000 lamports)
    pub fn get_sol_balance(&self, address: &Pubkey) -> Result<u64> {
        let balance = self.client
            .get_balance(address)
            .map_err(|e| ArkaError::ChainError(format!("Failed to get SOL balance: {}", e)))?;

        Ok(balance)
    }

    /// Get SOL balance in human-readable format (with decimals).
    pub fn get_sol_balance_sol(&self, address: &Pubkey) -> Result<f64> {
        let lamports = self.get_sol_balance(address)?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    /// Get SPL token balance for an address.
    ///
    /// # Arguments
    /// * `wallet` - Wallet address
    /// * `mint` - Token mint address
    ///
    /// # Returns
    /// Token balance in smallest unit
    pub fn get_token_balance(&self, wallet: &Pubkey, mint: &Pubkey) -> Result<u64> {
        let ata = get_associated_token_address(wallet, mint);

        let balance = self.client
            .get_token_account_balance(&ata)
            .map_err(|e| ArkaError::ChainError(format!("Failed to get token balance: {}", e)))?;

        let amount: u64 = balance.amount.parse()
            .map_err(|e| ArkaError::ChainError(format!("Failed to parse token balance: {}", e)))?;

        Ok(amount)
    }

    /// Transfer SOL to another address.
    ///
    /// # Arguments
    /// * `from` - Sender's keypair
    /// * `to` - Recipient's public key
    /// * `lamports` - Amount in lamports
    ///
    /// # Returns
    /// Transaction signature
    pub fn transfer_sol(
        &self,
        from: &Keypair,
        to: &Pubkey,
        lamports: u64,
    ) -> Result<Signature> {
        let instruction = system_instruction::transfer(
            &from.pubkey(),
            to,
            lamports,
        );

        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::ChainError(format!("Failed to get blockhash: {}", e)))?;

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );

        let signature = self.client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| ArkaError::ChainError(format!("Failed to send SOL transfer: {}", e)))?;

        Ok(signature)
    }

    /// Transfer SPL tokens to another address.
    ///
    /// # Arguments
    /// * `from` - Sender's keypair
    /// * `to` - Recipient's wallet address
    /// * `mint` - Token mint address
    /// * `amount` - Amount in smallest unit
    ///
    /// # Returns
    /// Transaction signature
    pub fn transfer_token(
        &self,
        from: &Keypair,
        to: &Pubkey,
        mint: &Pubkey,
        amount: u64,
    ) -> Result<Signature> {
        let from_ata = get_associated_token_address(&from.pubkey(), mint);
        let to_ata = get_associated_token_address(to, mint);

        let mut instructions = vec![];

        // Create recipient ATA if it doesn't exist
        let to_ata_exists = self.client.get_account(&to_ata).is_ok();
        if !to_ata_exists {
            let create_ata_ix = create_associated_token_account(
                &from.pubkey(),
                to,
                mint,
                &spl_token::ID,
            );
            instructions.push(create_ata_ix);
        }

        // Transfer tokens
        let transfer_ix = token_instruction::transfer(
            &spl_token::ID,
            &from_ata,
            &to_ata,
            &from.pubkey(),
            &[],
            amount,
        ).map_err(|e| ArkaError::ChainError(format!("Failed to create transfer instruction: {}", e)))?;

        instructions.push(transfer_ix);

        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::ChainError(format!("Failed to get blockhash: {}", e)))?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );

        let signature = self.client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| ArkaError::ChainError(format!("Failed to send token transfer: {}", e)))?;

        Ok(signature)
    }

    /// Get recent block height.
    pub fn get_block_height(&self) -> Result<u64> {
        let height = self.client
            .get_block_height()
            .map_err(|e| ArkaError::ChainError(format!("Failed to get block height: {}", e)))?;

        Ok(height)
    }

    /// Get slot number.
    pub fn get_slot(&self) -> Result<u64> {
        let slot = self.client
            .get_slot()
            .map_err(|e| ArkaError::ChainError(format!("Failed to get slot: {}", e)))?;

        Ok(slot)
    }

    /// Check if an account exists.
    pub fn account_exists(&self, address: &Pubkey) -> Result<bool> {
        match self.client.get_account(address) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get the underlying RPC client.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }
}

/// Solana client for agent operations.
#[derive(Debug, Clone)]
pub struct SolanaClient {
    chain: SolanaChain,
    wallet: Keypair,
}

impl SolanaClient {
    /// Create a new Solana client.
    pub fn new(rpc_url: &str, wallet: Keypair) -> Result<Self> {
        let chain = SolanaChain::new(rpc_url)?;
        Ok(Self { chain, wallet })
    }

    /// Get wallet public key.
    pub fn pubkey(&self) -> Pubkey {
        self.wallet.pubkey()
    }

    /// Get wallet SOL balance.
    pub fn balance(&self) -> Result<u64> {
        self.chain.get_sol_balance(&self.wallet.pubkey())
    }

    /// Transfer SOL from wallet.
    pub fn send_sol(&self, to: &Pubkey, lamports: u64) -> Result<Signature> {
        self.chain.transfer_sol(&self.wallet, to, lamports)
    }

    /// Transfer SPL tokens from wallet.
    pub fn send_token(&self, to: &Pubkey, mint: &Pubkey, amount: u64) -> Result<Signature> {
        self.chain.transfer_token(&self.wallet, to, mint, amount)
    }

    /// Get the underlying chain.
    pub fn chain(&self) -> &SolanaChain {
        &self.chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_programs() {
        assert_eq!(SolanaPrograms::SYSTEM_PROGRAM, "11111111111111111111111111111111");
        assert_eq!(SolanaPrograms::TOKEN_PROGRAM, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    }

    #[test]
    fn test_solana_chain_creation() {
        let chain = SolanaChain::new("https://api.devnet.solana.com");
        assert!(chain.is_ok());
    }

    #[test]
    fn test_pubkey_conversion() {
        let pubkey_str = "11111111111111111111111111111111";
        let pubkey = Pubkey::from_str(pubkey_str);
        assert!(pubkey.is_ok());
    }
}
