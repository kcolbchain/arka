//! Solana chain connector for Arka.
//!
//! Uses raw JSON-RPC via `reqwest` to avoid version-conflict issues with
//! the rapidly-evolving `solana-client` / `spl-*` crate ecosystem.
//!
//! Supports:
//! - SOL balance queries
//! - SOL transfers
//! - SPL token transfer (via `spl-token-interface` instructions encoded manually)

use crate::chain::Chain;
use crate::error::{ArkaError, Result};

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::{
    message::Message,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;

// ── JSON-RPC helpers ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<RpcResult<T>>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcResult<T> {
    value: T,
    context: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcBlockhash {
    blockhash: String,
    #[serde(rename = "lastValidBlockHeight")]
    last_valid_block_height: u64,
}

// ── SolanaChain ────────────────────────────────────────────────────────────

/// Solana chain connector backed by a JSON-RPC endpoint.
pub struct SolanaChain {
    chain: Chain,
    rpc_url: String,
    http: Client,
}

impl SolanaChain {
    /// Connect to Solana mainnet using the default RPC.
    pub fn new() -> Result<Self> {
        Self::with_rpc(Chain::Solana.default_rpc())
    }

    /// Connect to a custom RPC endpoint.
    pub fn with_rpc(url: &str) -> Result<Self> {
        let http = Client::builder()
            .build()
            .map_err(|e| ArkaError::Config(format!("HTTP client error: {e}")))?;
        Ok(Self {
            chain: Chain::Solana,
            rpc_url: url.to_string(),
            http,
        })
    }

    /// The chain this connector is targeting.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    // ── Low-level RPC ───────────────────────────────────────────────────

    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ArkaError::Rpc(e.to_string()))?;

        let raw: Value = resp
            .json()
            .await
            .map_err(|e| ArkaError::Rpc(e.to_string()))?;

        if let Some(err) = raw.get("error") {
            return Err(ArkaError::Rpc(format!("RPC error: {err}")));
        }
        Ok(raw)
    }

    async fn get_balance_lamports(&self, pubkey: &Pubkey) -> Result<u64> {
        let raw = self
            .rpc_call(
                "getBalance",
                json!([pubkey.to_string(), {"commitment": "confirmed"}]),
            )
            .await?;
        raw["result"]["value"]
            .as_u64()
            .ok_or_else(|| ArkaError::Rpc("missing balance value".into()))
    }

    async fn get_latest_blockhash(&self) -> Result<(String, u64)> {
        let raw = self
            .rpc_call("getLatestBlockhash", json!([{"commitment": "confirmed"}]))
            .await?;
        let bh = raw["result"]["value"]["blockhash"]
            .as_str()
            .ok_or_else(|| ArkaError::Rpc("missing blockhash".into()))?
            .to_string();
        let height = raw["result"]["value"]["lastValidBlockHeight"]
            .as_u64()
            .unwrap_or(0);
        Ok((bh, height))
    }

    async fn send_raw_transaction(&self, tx: &Transaction) -> Result<Signature> {
        let encoded =
            bs58::encode(bincode::serialize(tx).map_err(|e| ArkaError::Rpc(e.to_string()))?)
                .into_string();

        let raw = self
            .rpc_call(
                "sendTransaction",
                json!([encoded, {"encoding": "base58", "preflightCommitment": "confirmed"}]),
            )
            .await?;

        let sig_str = raw["result"]
            .as_str()
            .ok_or_else(|| ArkaError::Rpc("missing signature".into()))?;
        Signature::from_str(sig_str).map_err(|e| ArkaError::Rpc(format!("invalid signature: {e}")))
    }

    // ── Public API ──────────────────────────────────────────────────────

    /// Get SOL balance in lamports for a public key.
    pub async fn balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.get_balance_lamports(pubkey).await
    }

    /// Get SOL balance converted to SOL (f64).
    pub async fn balance_sol(&self, pubkey: &Pubkey) -> Result<f64> {
        let lamports = self.get_balance_lamports(pubkey).await?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    /// Transfer SOL from `sender` keypair to `recipient`.
    ///
    /// Returns the transaction signature.
    pub async fn transfer_sol(
        &self,
        sender: &Keypair,
        recipient: &Pubkey,
        lamports: u64,
    ) -> Result<Signature> {
        let (blockhash_str, _) = self.get_latest_blockhash().await?;
        let blockhash: solana_sdk::hash::Hash = blockhash_str
            .parse()
            .map_err(|e| ArkaError::Rpc(format!("invalid blockhash: {e}")))?;

        let ix = system_instruction::transfer(&sender.pubkey(), recipient, lamports);
        let message = Message::new(&[ix], Some(&sender.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[sender], blockhash);

        self.send_raw_transaction(&tx).await
    }

    /// Transfer SPL tokens.
    ///
    /// `mint` is the token mint address, `decimals` the mint's decimal places.
    /// The `sender` keypair must own the source associated token account.
    pub async fn transfer_spl(
        &self,
        sender: &Keypair,
        recipient_wallet: &Pubkey,
        mint: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Result<Signature> {
        // Derive associated token accounts via the standard deterministic path.
        let source_ata = spl_ata(mint, &sender.pubkey());
        let dest_ata = spl_ata(mint, recipient_wallet);

        let (blockhash_str, _) = self.get_latest_blockhash().await?;
        let blockhash: solana_sdk::hash::Hash = blockhash_str
            .parse()
            .map_err(|e| ArkaError::Rpc(format!("invalid blockhash: {e}")))?;

        // Build the SPL Transfer instruction manually (avoids spl-token dep).
        let ix = spl_transfer_checked_ix(
            &source_ata,
            mint,
            &dest_ata,
            &sender.pubkey(),
            amount,
            decimals,
        )?;

        let message = Message::new(&[ix], Some(&sender.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[sender], blockhash);

        self.send_raw_transaction(&tx).await
    }
}

// ── SPL helpers (no spl-token crate required) ──────────────────────────────

/// SPL Token program ID.
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// SPL Associated Token Account program ID.
const SPL_ATA_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bS5";

/// Derive the associated token account address for a wallet + mint.
pub fn spl_ata(mint: &Pubkey, wallet: &Pubkey) -> Pubkey {
    let ata_program = Pubkey::from_str(SPL_ATA_PROGRAM_ID).expect("valid pubkey");
    let spl_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("valid pubkey");
    let seeds: &[&[u8]] = &[wallet.as_ref(), spl_program.as_ref(), mint.as_ref()];
    Pubkey::find_program_address(seeds, &ata_program).0
}

/// Build a `spl-token TransferChecked` instruction from raw bytes.
///
/// Discriminant layout (v3 / v4 compatible):
/// `[12, amount: u64 LE, decimals: u8]` → 10 bytes total
fn spl_transfer_checked_ix(
    source: &Pubkey,
    mint: &Pubkey,
    dest: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Result<solana_sdk::instruction::Instruction> {
    let spl_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID)
        .map_err(|e| ArkaError::Config(format!("bad program id: {e}")))?;

    let mut data = vec![12u8]; // TransferChecked discriminant
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);

    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*source, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*mint, false),
        solana_sdk::instruction::AccountMeta::new(*dest, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
    ];

    Ok(solana_sdk::instruction::Instruction {
        program_id: spl_program,
        accounts,
        data,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spl_ata_derivation() {
        // Known ATA for a dummy wallet + mint — just verifies we don't panic.
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let ata = spl_ata(&mint, &wallet);
        assert_ne!(ata, wallet);
        assert_ne!(ata, mint);
    }

    #[test]
    fn test_spl_transfer_checked_ix() {
        let source = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let ix = spl_transfer_checked_ix(&source, &mint, &dest, &authority, 1_000_000, 6)
            .expect("builds ok");
        assert_eq!(ix.data[0], 12); // TransferChecked discriminant
        assert_eq!(ix.accounts.len(), 4);
    }

    /// Requires `solana-test-validator` running on localhost:8899.
    /// Skipped automatically if the validator is not reachable.
    #[tokio::test]
    async fn test_balance_local_validator() {
        let chain = match SolanaChain::with_rpc("http://localhost:8899") {
            Ok(c) => c,
            Err(_) => return,
        };
        let kp = Keypair::new();
        // If the validator is up, balance should be 0 for a fresh keypair.
        if let Ok(bal) = chain.balance(&kp.pubkey()).await {
            assert_eq!(bal, 0);
        }
    }
}
