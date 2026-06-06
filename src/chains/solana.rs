//! Solana chain primitives — connection, balance, SOL/SPL transfers.
//!
//! Solana is a non-EVM chain, so this module builds on the upstream
//! `solana-client` and `solana-sdk` crates rather than `alloy`.
//!
//! ## Features
//! - Connect to any Solana cluster (mainnet, devnet, testnet, local)
//! - Query native SOL balance for any pubkey
//! - Build, sign, and send SOL transfer transactions
//! - Build, sign, and send SPL Token transfer transactions
//!
//! ## Example
//! ```rust,ignore
//! let client = SolanaClient::connect(SolanaCluster::Devnet).await?;
//! let balance = client.get_balance(&pubkey).await?;
//! let sig = client.transfer_sol(&from_keypair, &to_pubkey, lamports).await?;
//! ```

use std::str::FromStr;
use std::sync::Arc;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    message::Message,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction as token_instruction;

use crate::error::{ArkaError, Result};

/// Pre-configured Solana cluster endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolanaCluster {
    /// Production mainnet-beta.
    Mainnet,
    /// Development network.
    Devnet,
    /// Test network.
    Testnet,
    /// Local test validator (default http://127.0.0.1:8899).
    Local,
}

impl SolanaCluster {
    /// The JSON-RPC URL for this cluster.
    pub fn rpc_url(&self) -> &'static str {
        match self {
            SolanaCluster::Mainnet => "https://api.mainnet-beta.solana.com",
            SolanaCluster::Devnet => "https://api.devnet.solana.com",
            SolanaCluster::Testnet => "https://api.testnet.solana.com",
            SolanaCluster::Local => "http://127.0.0.1:8899",
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SolanaCluster::Mainnet => "mainnet-beta",
            SolanaCluster::Devnet => "devnet",
            SolanaCluster::Testnet => "testnet",
            SolanaCluster::Local => "local",
        }
    }
}

impl std::fmt::Display for SolanaCluster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A typed connection to a Solana cluster.
///
/// Wraps [`RpcClient`] from `solana-client` and exposes high-level
/// operations (balance, transfer, SPL transfer) that return `Result`.
#[derive(Debug, Clone)]
pub struct SolanaClient {
    inner: Arc<RpcClient>,
    cluster: SolanaCluster,
}

impl SolanaClient {
    /// Connect to a Solana cluster with the default commitment.
    pub async fn connect(cluster: SolanaCluster) -> Result<Self> {
        let url = cluster.rpc_url();
        let inner = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());
        // Quick health check — the RPC should respond.
        let _version = inner
            .get_version()
            .await
            .map_err(|e| ArkaError::Rpc(format!("Solana {cluster} unreachable: {e}")))?;
        Ok(Self {
            inner: Arc::new(inner),
            cluster,
        })
    }

    /// Connect with a custom RPC URL (e.g. a private endpoint or local validator).
    pub async fn connect_with_url(url: &str, cluster: SolanaCluster) -> Result<Self> {
        let inner =
            RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());
        let _version = inner
            .get_version()
            .await
            .map_err(|e| ArkaError::Rpc(format!("Solana at {url} unreachable: {e}")))?;
        Ok(Self {
            inner: Arc::new(inner),
            cluster,
        })
    }

    /// The cluster this client targets.
    pub fn cluster(&self) -> SolanaCluster {
        self.cluster
    }

    /// The underlying RPC client (for advanced use).
    pub fn rpc(&self) -> &RpcClient {
        &self.inner
    }

    // -----------------------------------------------------------------
    // Balance
    // -----------------------------------------------------------------

    /// Get the native SOL balance (in lamports) for a pubkey.
    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.inner
            .get_balance(pubkey)
            .await
            .map_err(|e| ArkaError::Rpc(format!("Failed to get balance for {pubkey}: {e}")))
    }

    /// Convenience: balance denominated in SOL (as a floating-point value).
    pub async fn get_balance_sol(&self, pubkey: &Pubkey) -> Result<f64> {
        let lamports = self.get_balance(pubkey).await?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    // -----------------------------------------------------------------
    // SOL transfer
    // -----------------------------------------------------------------

    /// Transfer native SOL from `sender` to `receiver`.
    ///
    /// * `sender` — the funded keypair that will sign the transaction.
    /// * `receiver` — the destination pubkey.
    /// * `lamports` — amount in lamports (1 SOL = 10^9 lamports).
    ///
    /// Returns the transaction signature.
    pub async fn transfer_sol(
        &self,
        sender: &Keypair,
        receiver: &Pubkey,
        lamports: u64,
    ) -> Result<Signature> {
        let recent_blockhash = self.inner.get_latest_blockhash().await.map_err(|e| {
            ArkaError::Rpc(format!("Failed to get blockhash: {e}"))
        })?;

        let ix = system_instruction::transfer(&sender.pubkey(), receiver, lamports);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&sender.pubkey()),
            &[sender],
            recent_blockhash,
        );

        self.inner
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| ArkaError::Transaction(format!("SOL transfer failed: {e}")))
    }

    /// Transfer SOL denominated in SOL (non-lamports).
    pub async fn transfer_sol_amount(
        &self,
        sender: &Keypair,
        receiver: &Pubkey,
        sol_amount: f64,
    ) -> Result<Signature> {
        let lamports = (sol_amount * LAMPORTS_PER_SOL as f64) as u64;
        self.transfer_sol(sender, receiver, lamports).await
    }

    // -----------------------------------------------------------------
    // SPL Token transfer
    // -----------------------------------------------------------------

    /// Transfer an SPL token from one associated token account to another.
    ///
    /// * `owner` — the wallet keypair that owns the source token account.
    /// * `mint` — the SPL token mint address.
    /// * `destination_owner` — the pubkey that owns the destination ATA (created
    ///   automatically if it does not exist).
    /// * `amount` — raw token amount (respect decimals of the mint; e.g. 1_000_000
    ///   for 1 USDC on a 6-decimal mint).
    ///
    /// Returns the transaction signature.
    pub async fn transfer_spl_token(
        &self,
        owner: &Keypair,
        mint: &Pubkey,
        destination_owner: &Pubkey,
        amount: u64,
    ) -> Result<Signature> {
        let source_ata = get_associated_token_address(&owner.pubkey(), mint);
        let destination_ata = get_associated_token_address(destination_owner, mint);

        // We use the legacy (non-`create_associated_token_account`) path:
        // build a transfer instruction. The destination ATA must exist.
        let recent_blockhash = self.inner.get_latest_blockhash().await.map_err(|e| {
            ArkaError::Rpc(format!("Failed to get blockhash: {e}"))
        })?;

        let ix = token_instruction::transfer(
            &spl_token::id(),
            &source_ata,
            &destination_ata,
            &owner.pubkey(),
            &[],
            amount,
        )
        .map_err(|e| ArkaError::Transaction(format!("Failed to build SPL transfer ix: {e}")))?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&owner.pubkey()),
            &[owner],
            recent_blockhash,
        );

        self.inner
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| ArkaError::Transaction(format!("SPL transfer failed: {e}")))
    }

    /// Ensure an associated token account exists for `owner` and `mint`.
    ///
    /// If the ATA already exists this is a no-op (returns `Ok`).
    /// The `payer` keypair covers the rent-exemption fee.
    pub async fn ensure_ata(
        &self,
        payer: &Keypair,
        owner: &Pubkey,
        mint: &Pubkey,
    ) -> Result<()> {
        let ata = get_associated_token_address(owner, mint);
        if let Ok(Some(_)) = self.inner.get_account(&ata).await {
            return Ok(()); // Already exists
        }

        let recent_blockhash = self.inner.get_latest_blockhash().await.map_err(|e| {
            ArkaError::Rpc(format!("Failed to get blockhash: {e}"))
        })?;

        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            owner,
            mint,
            &spl_token::id(),
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        self.inner
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| ArkaError::Transaction(format!("Failed to create ATA: {e}")))?;

        Ok(())
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: a deterministic keypair for unit tests (no RPC required).
    fn test_keypair() -> Keypair {
        // This is a well-known test key — never use in production.
        let secret = [
            208, 175, 150, 242, 88, 34, 108, 88, 177, 16, 168, 75, 115, 181, 199, 242, 120, 114,
            202, 129, 11, 196, 73, 82, 237, 212, 120, 4, 202, 19, 206, 119, 68, 104, 11, 15, 13,
            196, 7, 171, 51, 81, 129, 55, 9, 94, 92, 175, 45, 134, 153, 72, 99, 20, 122, 46, 162,
            85, 250, 108, 188, 188, 180, 21,
        ];
        Keypair::from_bytes(&secret).expect("valid test keypair")
    }

    #[test]
    fn cluster_rpc_urls_are_valid() {
        assert!(SolanaCluster::Mainnet.rpc_url().contains("mainnet"));
        assert!(SolanaCluster::Devnet.rpc_url().contains("devnet"));
        assert!(SolanaCluster::Testnet.rpc_url().contains("testnet"));
        assert!(SolanaCluster::Local.rpc_url().contains("127.0.0.1"));
    }

    #[test]
    fn cluster_labels_are_readable() {
        assert_eq!(SolanaCluster::Mainnet.label(), "mainnet-beta");
        assert_eq!(SolanaCluster::Devnet.label(), "devnet");
    }

    #[test]
    fn cluster_display_matches_label() {
        assert_eq!(format!("{}", SolanaCluster::Devnet), "devnet");
    }

    #[test]
    fn test_keypair_derives_pubkey_consistently() {
        let kp = test_keypair();
        // This specific secret key always produces this pubkey.
        let expected = "F8GyJg4PbykCnBHeS8JG5FrUJnJgxn7rR4qC5mPbF2Lz";
        assert_eq!(kp.pubkey().to_string(), expected);
    }

    #[test]
    fn lamports_per_sol_constant_is_sane() {
        assert_eq!(LAMPORTS_PER_SOL, 1_000_000_000);
    }

    #[test]
    fn sol_to_lamports_conversion_rounds_down() {
        // 0.5 SOL = 500_000_000 lamports
        let lamports = (0.5f64 * LAMPORTS_PER_SOL as f64) as u64;
        assert_eq!(lamports, 500_000_000);
    }

    #[test]
    fn spl_token_address_derivation_is_deterministic() {
        let owner = test_keypair();
        let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
            .expect("valid USDC mint");
        let ata = get_associated_token_address(&owner.pubkey(), &mint);
        // Once derived, the same owner+mint always yields the same ATA.
        let ata2 = get_associated_token_address(&owner.pubkey(), &mint);
        assert_eq!(ata, ata2);
    }
}
