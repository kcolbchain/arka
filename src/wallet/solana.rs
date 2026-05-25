//! Solana wallet using ed25519-dalek for key generation and signing.

use async_trait::async_trait;
use ed25519_dalek::{Signature, Signer, SigningKey};
use rand::rngs::OsRng;

use crate::error::{ArkaError, Result};

use super::Wallet;

/// Solana wallet backed by an ed25519 keypair.
pub struct SolanaWallet {
    keypair: SigningKey,
    label: String,
}

impl SolanaWallet {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let keypair = SigningKey::generate(&mut csprng);
        Self { keypair, label: String::from("solana-default") }
    }

    pub fn from_secret_key(secret: &[u8]) -> Result<Self> {
        let bytes: [u8; 32] = secret.try_into()
            .map_err(|_| ArkaError::Wallet("Solana secret key must be 32 bytes".into()))?;
        let keypair = SigningKey::from_bytes(&bytes);
        Ok(Self { keypair, label: String::from("solana-imported") })
    }

    pub fn from_env(var_name: &str) -> Result<Self> {
        let key = std::env::var(var_name)
            .map_err(|_| ArkaError::Wallet(format!("Environment variable {var_name} not set")))?;
        let decoded = bs58::decode(&key)
            .into_vec()
            .map_err(|e| ArkaError::Wallet(format!("Invalid base58 key: {e}")))?;
        Self::from_secret_key(&decoded)
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Get the Ed25519 public key bytes.
    pub fn pubkey_bytes(&self) -> [u8; 32] {
        self.keypair.verifying_key().to_bytes()
    }

    /// Get the Solana base58-encoded address.
    pub fn address(&self) -> String {
        bs58::encode(self.pubkey_bytes()).into_string()
    }
}

#[async_trait]
impl Wallet for SolanaWallet {
    fn pubkey(&self) -> String {
        self.address()
    }

    async fn sign_message(&self, msg: &[u8]) -> Result<Vec<u8>> {
        let sig: Signature = self.keypair.sign(msg);
        Ok(sig.to_bytes().to_vec())
    }

    async fn sign_transaction(&self, tx_data: &[u8]) -> Result<Vec<u8>> {
        // Solana transaction signing: sign the message hash
        let sig: Signature = self.keypair.sign(tx_data);
        Ok(sig.to_bytes().to_vec())
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn chain_family(&self) -> &str {
        "solana"
    }
}

impl std::fmt::Debug for SolanaWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaWallet")
            .field("address", &self.address())
            .field("label", &self.label)
            .finish()
    }
}
