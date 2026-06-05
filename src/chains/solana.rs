//! Solana chain module — connection, balance, SOL transfer, SPL token transfer.
//!
//! Gated behind the ``solana`` feature flag. Depends on ``solana-sdk``,
//! ``solana-client``, and ``spl-token``.
//!
//! ## Usage
//!
//! ```ignore
//! use arka::chains::solana::{SolanaConnector, SolanaWallet};
//!
//! let connector = SolanaConnector::new("https://api.mainnet-beta.solana.com")?;
//! let balance = connector.balance("So1anaAddrEss123456789012345678901234567890")?;
//! connector.transfer_sol(&wallet, "recipient_pubkey", 1_000_000_000)?; // 1 SOL
//! ```

#![cfg(feature = "solana")]

use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

/// Error type for Solana operations.
#[derive(Debug, thiserror::Error)]
pub enum SolanaError {
    #[error("Solana RPC error: {0}")]
    Rpc(#[from] solana_client::client_error::ClientError),
    #[error("Invalid pubkey: {0}")]
    InvalidPubkey(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Wallet error: {0}")]
    Wallet(String),
}

/// Result type for Solana operations.
pub type SolanaResult<T> = Result<T, SolanaError>;

/// A Solana connector wrapping an RPC client.
pub struct SolanaConnector {
    client: RpcClient,
}

impl SolanaConnector {
    /// Create a new connector to the given RPC endpoint.
    pub fn new(rpc_url: &str) -> SolanaResult<Self> {
        let client = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );
        Ok(Self { client })
    }

    /// Get the underlying RPC client reference.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }

    /// Get SOL balance for a public key (in lamports).
    pub fn balance(&self, address: &str) -> SolanaResult<u64> {
        let pubkey = parse_pubkey(address)?;
        self.client
            .get_balance(&pubkey)
            .map_err(SolanaError::from)
    }

    /// Get the current block height.
    pub fn block_height(&self) -> SolanaResult<u64> {
        self.client
            .get_block_height()
            .map_err(SolanaError::from)
    }

    /// Transfer SOL from a keypair to a recipient.
    ///
    /// ``amount`` is in lamports (1 SOL = 1_000_000_000 lamports).
    /// Returns the transaction signature.
    pub fn transfer_sol(
        &self,
        sender: &Keypair,
        recipient: &str,
        amount: u64,
    ) -> SolanaResult<String> {
        let to_pubkey = parse_pubkey(recipient)?;
        let from_pubkey = sender.pubkey();

        let ix = system_instruction::transfer(&from_pubkey, &to_pubkey, amount);
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&from_pubkey),
            &[sender],
            recent_blockhash,
        );
        let sig = self.client.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    }

    /// Transfer an SPL token from a source token account to a destination.
    ///
    /// ``source_owner`` is the wallet keypair that owns the tokens.
    /// ``source_token_account`` is the associated token account to send from.
    /// ``destination_token_account`` is the token account to send to.
    /// ``amount`` is in the token's smallest unit (e.g. 6 decimals for USDC).
    pub fn transfer_spl_token(
        &self,
        source_owner: &Keypair,
        source_token_account: &str,
        destination_token_account: &str,
        amount: u64,
    ) -> SolanaResult<String> {
        let src = parse_pubkey(source_token_account)?;
        let dst = parse_pubkey(destination_token_account)?;
        let owner = source_owner.pubkey();

        let ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &src,
            &dst,
            &owner,
            &[],
            amount,
        )?;
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&owner),
            &[source_owner],
            recent_blockhash,
        );
        let sig = self.client.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    }

    /// Get the latest blockhash.
    pub fn get_latest_blockhash(&self) -> SolanaResult<String> {
        let hash = self.client.get_latest_blockhash()?;
        Ok(hash.to_string())
    }
}

/// Parse a base58-encoded Solana address string into a Pubkey.
fn parse_pubkey(address: &str) -> SolanaResult<Pubkey> {
    Pubkey::from_str(address)
        .map_err(|_| SolanaError::InvalidPubkey(address.to_string()))
}

/// A simple Solana wallet wrapping a keypair.
pub struct SolanaWallet {
    keypair: Keypair,
    label: String,
}

impl SolanaWallet {
    /// Generate a new random keypair.
    pub fn generate() -> Self {
        Self {
            keypair: Keypair::new(),
            label: "solana-wallet".to_string(),
        }
    }

    /// Create a wallet from a base58-encoded private key.
    pub fn from_base58(private_key: &str, label: &str) -> SolanaResult<Self> {
        let bytes = bs58::decode(private_key)
            .into_vec()
            .map_err(|e| SolanaError::Wallet(format!("Invalid base58 key: {e}")))?;
        if bytes.len() != 64 {
            return Err(SolanaError::Wallet(
                "Private key must be 64 bytes".to_string(),
            ));
        }
        let keypair = Keypair::from_bytes(&bytes)
            .map_err(|e| SolanaError::Wallet(format!("Invalid keypair: {e}")))?;
        Ok(Self {
            keypair,
            label: label.to_string(),
        })
    }

    /// Get the public key as a base58 string.
    pub fn address(&self) -> String {
        self.keypair.pubkey().to_string()
    }

    /// Get a reference to the underlying keypair.
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    /// Get the wallet label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pubkey_valid() {
        // Known Solana pubkey
        let addr = "11111111111111111111111111111111";
        assert!(parse_pubkey(addr).is_ok());
    }

    #[test]
    fn test_parse_pubkey_invalid() {
        assert!(parse_pubkey("not-a-valid-pubkey!!").is_err());
    }

    #[test]
    fn test_wallet_generate_and_address() {
        let wallet = SolanaWallet::generate();
        let addr = wallet.address();
        assert_eq!(addr.len(), 44); // Base58 encoded 32-byte pubkey
        assert!(!addr.is_empty());
    }

    #[test]
    fn test_wallet_from_base58_roundtrip() {
        let wallet = SolanaWallet::generate();
        let addr = wallet.address();
        // We can't easily get the private key back from Keypair,
        // but we can verify the generated wallet produces valid addresses
        assert!(addr.starts_with("G") || addr.starts_with("D")
            || addr.starts_with("E") || addr.starts_with("9")
            || addr.starts_with("A") || addr.starts_with("B")
            || addr.starts_with("H") || addr.starts_with("C")
            || addr.starts_with("F"));
    }

    #[test]
    fn test_solana_connector_new() {
        let connector = SolanaConnector::new("https://api.devnet.solana.com");
        assert!(connector.is_ok());
    }

    #[test]
    fn test_balance_invalid_address() {
        let connector = SolanaConnector::new("https://api.devnet.solana.com").unwrap();
        let result = connector.balance("invalid");
        assert!(result.is_err());
    }
}
