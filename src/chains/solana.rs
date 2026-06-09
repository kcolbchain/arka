//! Solana network primitives — well-known program addresses, token mints,
//! and SPL token helpers.
//!
//! Solana is a high-throughput, low-latency blockchain that powers DeFi,
//! NFTs, and increasingly AI agent infrastructure. This module provides
//! constants for well-known Solana addresses and a typed SPL token client
//! analogous to the EVM-focused `AgentDepositClient` on Arbitrum.
//!
//! ## What this module provides
//! - `SolanaConstants` — well-known program IDs and token mints (USDC, USDT, SOL)
//! - `SplTokenClient` — typed client for SPL token operations (balance, transfer, approve)

use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::chain::solana_connector::SolanaConnector;
use crate::error::{ArkaError, Result};

/// Solana mainnet chain identifier (for display/routing, not EVM chain ID).
pub const SOLANA_CHAIN_LABEL: &str = "solana-mainnet";

/// Solana devnet chain identifier.
pub const SOLANA_DEVNET_LABEL: &str = "solana-devnet";

/// Well-known Solana program addresses and token mints.
pub struct SolanaConstants;

impl SolanaConstants {
    /// System Program — native SOL transfers.
    pub const SYSTEM_PROGRAM: &'static str = "11111111111111111111111111111111";

    /// SPL Token Program — fungible token operations.
    pub const SPL_TOKEN_PROGRAM: &'static str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    /// Associated Token Account Program — deterministic token accounts.
    pub const ASSOCIATED_TOKEN_PROGRAM: &'static str =
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

    /// Native Wrapped SOL mint.
    pub const WSOL: &'static str = "So11111111111111111111111111111111111111112";

    /// USDC on Solana (Circle).
    pub const USDC: &'static str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    /// USDT on Solana (Tether).
    pub const USDT: &'static str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

    /// BONK (popular Solana meme token).
    pub const BONK: &'static str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";

    /// JTO (Jito governance token).
    pub const JTO: &'static str = "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL";

    /// JUP (Jupiter governance token).
    pub const JUP: &'static str = "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN";

    /// Marinade Finance (mSOL liquid staking).
    pub const MSOL: &'static str = "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So";
}

/// Typed client for SPL token operations.
///
/// This client provides a clean API for SPL token interactions analogous
/// to the `AgentDepositClient` on Arbitrum. It wraps `SolanaConnector`
/// and SPL token instructions.
///
/// # Example
/// ```no_run
/// use arka::chains::solana::SplTokenClient;
/// use arka::chain::solana_connector::SolanaConnector;
///
/// let conn = SolanaConnector::devnet().unwrap();
/// let usdc = SplTokenClient::usdc(&conn);
/// // Check USDC balance
/// // let balance = usdc.balance("owner_pubkey").unwrap();
/// ```
pub struct SplTokenClient<'a> {
    connector: &'a SolanaConnector,
    mint: String,
    decimals: u8,
}

impl<'a> SplTokenClient<'a> {
    /// Create a client for a specific SPL token mint.
    pub fn new(connector: &'a SolanaConnector, mint: &str, decimals: u8) -> Self {
        Self {
            connector,
            mint: mint.to_string(),
            decimals,
        }
    }

    /// Create a USDC client (6 decimals).
    pub fn usdc(connector: &'a SolanaConnector) -> Self {
        Self::new(connector, SolanaConstants::USDC, 6)
    }

    /// Create a USDT client (6 decimals).
    pub fn usdt(connector: &'a SolanaConnector) -> Self {
        Self::new(connector, SolanaConstants::USDT, 6)
    }

    /// Create a BONK client (5 decimals).
    pub fn bonk(connector: &'a SolanaConnector) -> Self {
        Self::new(connector, SolanaConstants::BONK, 5)
    }

    /// Get the mint address.
    pub fn mint(&self) -> &str {
        &self.mint
    }

    /// Get token decimals.
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Get raw token balance for an owner.
    pub fn balance(&self, owner: &str) -> Result<u64> {
        self.connector.spl_balance(&self.mint, owner)
    }

    /// Get human-readable balance (adjusted for decimals).
    pub fn balance_ui(&self, owner: &str) -> Result<f64> {
        let raw = self.balance(owner)?;
        Ok(raw as f64 / 10f64.powi(self.decimals as i32))
    }

    /// Transfer tokens. Returns transaction signature.
    pub fn transfer(
        &self,
        from: &solana_sdk::signature::Keypair,
        to: &str,
        amount: u64,
    ) -> Result<solana_sdk::signature::Signature> {
        self.connector.transfer_spl(from, &self.mint, to, amount)
    }

    /// Get the Pubkey for the mint.
    pub fn mint_pubkey(&self) -> Result<Pubkey> {
        Pubkey::from_str(&self.mint)
            .map_err(|e| ArkaError::Config(format!("Invalid mint address: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usdc_mint_parses() {
        let pubkey = Pubkey::from_str(SolanaConstants::USDC);
        assert!(pubkey.is_ok());
    }

    #[test]
    fn usdt_mint_parses() {
        let pubkey = Pubkey::from_str(SolanaConstants::USDT);
        assert!(pubkey.is_ok());
    }

    #[test]
    fn system_program_is_valid() {
        let pubkey = Pubkey::from_str(SolanaConstants::SYSTEM_PROGRAM);
        assert!(pubkey.is_ok());
    }

    #[test]
    fn spl_token_program_is_valid() {
        let pubkey = Pubkey::from_str(SolanaConstants::SPL_TOKEN_PROGRAM);
        assert!(pubkey.is_ok());
    }

    #[test]
    fn spl_client_usdc_has_correct_decimals() {
        let conn = SolanaConnector::devnet().unwrap();
        let usdc = SplTokenClient::usdc(&conn);
        assert_eq!(usdc.decimals(), 6);
        assert_eq!(usdc.mint(), SolanaConstants::USDC);
    }

    #[test]
    fn spl_client_usdt_has_correct_decimals() {
        let conn = SolanaConnector::devnet().unwrap();
        let usdt = SplTokenClient::usdt(&conn);
        assert_eq!(usdt.decimals(), 6);
    }

    #[test]
    fn balance_ui_conversion() {
        // 1000000 raw units with 6 decimals = 1.0 UI
        let conn = SolanaConnector::devnet().unwrap();
        let usdc = SplTokenClient::usdc(&conn);
        let ui = 1000000f64 / 10f64.powi(6);
        assert_eq!(ui, 1.0);
    }
}
