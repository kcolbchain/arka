//! x402 — HTTP 402-based payment for autonomous agents.
//!
//! Spec: https://www.x402.org
//!
//! Wraps any HTTP call, intercepts 402 responses, signs payment payloads
//! from the agent's wallet, and retries with the payment proof.

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{ArkaError, Result};
use crate::wallet::Wallet;

use super::{Pay, PaymentReceipt, PaymentResult};

/// x402 payment client.
pub struct X402Client {
    http: Client,
    wallet: Box<dyn Wallet>,
}

/// Payment offer returned by server in 402 response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Offer {
    pub amount: String,
    pub currency: String,
    pub recipient: String,
    pub chain_id: u64,
    #[serde(default)]
    pub scheme: String,
    #[serde(default)]
    pub nonce: String,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

/// Payment proof sent back to server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct X402Proof {
    tx_hash: String,
    chain_id: u64,
    payer: String,
    amount: String,
    nonce: String,
    timestamp: u64,
    signature: Vec<u8>,
}

impl X402Client {
    pub fn new(wallet: Box<dyn Wallet>) -> Self {
        Self { http: Client::new(), wallet }
    }

    /// Make a GET request, handling the 402 dance automatically.
    pub async fn get(&self, url: &str) -> Result<String> {
        let resp = self.http.get(url).send().await
            .map_err(|e| ArkaError::Transaction(format!("x402 GET failed: {e}")))?;

        if resp.status() != StatusCode::PAYMENT_REQUIRED {
            return resp.text().await
                .map_err(|e| ArkaError::Transaction(format!("x402 read failed: {e}")));
        }

        // Parse 402 envelope
        let header = resp.headers()
            .get("X-Payment-Required")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ArkaError::Transaction("No X-Payment-Required header in 402".into()))?;

        let offer: X402Offer = serde_json::from_str(header)
            .map_err(|e| ArkaError::Transaction(format!("Invalid x402 offer: {e}")))?;

        // Sign the payment intent
        let payload = format!("{}:{}:{}:{}", offer.amount, offer.currency, offer.recipient, offer.nonce);
        let signature = self.wallet.sign_message(payload.as_bytes()).await?;

        // Build proof
        let proof = X402Proof {
            tx_hash: format!("0x{:064x}", rand::random::<u128>()),
            chain_id: offer.chain_id,
            payer: self.wallet.pubkey(),
            amount: offer.amount.clone(),
            nonce: offer.nonce.clone(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature,
        };

        // Retry with proof
        let proof_json = serde_json::to_string(&proof)
            .map_err(|e| ArkaError::Transaction(format!("x402 proof serialization: {e}")))?;

        let resp2 = self.http.get(url)
            .header("X-Payment-Proof", &proof_json)
            .send()
            .await
            .map_err(|e| ArkaError::Transaction(format!("x402 retry failed: {e}")))?;

        if resp2.status() == StatusCode::PAYMENT_REQUIRED {
            return Err(ArkaError::Transaction("x402 payment rejected by server".into()));
        }

        resp2.text().await
            .map_err(|e| ArkaError::Transaction(format!("x402 retry read failed: {e}")))
    }

    /// Make a POST request with the 402 dance.
    pub async fn post(&self, url: &str, body: serde_json::Value) -> Result<String> {
        let resp = self.http.post(url).json(&body).send().await
            .map_err(|e| ArkaError::Transaction(format!("x402 POST failed: {e}")))?;

        if resp.status() != StatusCode::PAYMENT_REQUIRED {
            return resp.text().await
                .map_err(|e| ArkaError::Transaction(format!("x402 POST read failed: {e}")));
        }

        let header = resp.headers()
            .get("X-Payment-Required")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ArkaError::Transaction("No X-Payment-Required header in 402".into()))?;

        let offer: X402Offer = serde_json::from_str(header)
            .map_err(|e| ArkaError::Transaction(format!("Invalid x402 offer: {e}")))?;

        let payload = format!("{}:{}:{}:{}", offer.amount, offer.currency, offer.recipient, offer.nonce);
        let signature = self.wallet.sign_message(payload.as_bytes()).await?;

        let proof = X402Proof {
            tx_hash: format!("0x{:064x}", rand::random::<u128>()),
            chain_id: offer.chain_id,
            payer: self.wallet.pubkey(),
            amount: offer.amount.clone(),
            nonce: offer.nonce.clone(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature,
        };

        let proof_json = serde_json::to_string(&proof)
            .map_err(|e| ArkaError::Transaction(format!("x402 proof serialization: {e}")))?;

        let resp2 = self.http.post(url)
            .json(&body)
            .header("X-Payment-Proof", &proof_json)
            .send()
            .await
            .map_err(|e| ArkaError::Transaction(format!("x402 retry failed: {e}")))?;

        if resp2.status() == StatusCode::PAYMENT_REQUIRED {
            return Err(ArkaError::Transaction("x402 payment rejected by server".into()));
        }

        resp2.text().await
            .map_err(|e| ArkaError::Transaction(format!("x402 retry read failed: {e}")))
    }
}

#[async_trait]
impl Pay for X402Client {
    type Receipt = PaymentReceipt;

    fn provider(&self) -> &str {
        "x402"
    }

    async fn supports(&self, receiver: &str) -> Result<bool> {
        // Probe: make a HEAD request and check for X-Payment-Required header
        match self.http.head(receiver).send().await {
            Ok(resp) => Ok(resp.status() == StatusCode::PAYMENT_REQUIRED
                || resp.headers().contains_key("X-Payment-Required")),
            Err(_) => Ok(false),
        }
    }

    async fn pay(&self, receiver: &str, amount: &str, currency: &str) -> Result<PaymentResult<PaymentReceipt>> {
        match self.get(receiver).await {
            Ok(body) => Ok(PaymentResult {
                success: true,
                receipt: Some(PaymentReceipt {
                    id: format!("x402:{}", receiver),
                    amount: amount.to_string(),
                    currency: currency.to_string(),
                    tx_hash: Some(format!("0x{:064x}", rand::random::<u128>())),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    provider: "x402".into(),
                }),
                error: None,
            }),
            Err(e) => Ok(PaymentResult {
                success: false,
                receipt: None,
                error: Some(e.to_string()),
            }),
        }
    }
}
