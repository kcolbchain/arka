//! MPP (Machine Payments Protocol) client.
//!
//! Implements the HTTP 402 payment flow for agent-to-service payments.
//! Spec: https://mpp.dev

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::error::{ArkaError, Result};

/// MPP payment client for autonomous agent payments.
pub struct MppClient {
    http: reqwest::Client,
}

/// Payment options returned by a server via HTTP 402.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOptions {
    pub methods: Vec<PaymentMethod>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub memo: Option<String>,
}

/// A supported payment method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub method_type: String, // "tempo", "stripe", "lightning"
    pub details: serde_json::Value,
}

/// Receipt from a completed payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    pub tx_hash: Option<String>,
    pub amount: String,
    pub currency: String,
    pub timestamp: u64,
}

impl MppClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Make a request to an MPP-enabled endpoint.
    /// If the server returns 402, parse payment options.
    pub async fn request(&self, url: &str) -> Result<MppResponse> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ArkaError::Mpp(format!("Request failed: {e}")))?;

        match resp.status() {
            StatusCode::PAYMENT_REQUIRED => {
                // Parse payment options from 402 response
                let body = resp
                    .text()
                    .await
                    .map_err(|e| ArkaError::Mpp(format!("Failed to read 402 body: {e}")))?;

                let options: PaymentOptions =
                    serde_json::from_str(&body).unwrap_or(PaymentOptions {
                        methods: vec![],
                        amount: None,
                        currency: None,
                        memo: Some(body),
                    });

                Ok(MppResponse::PaymentRequired(options))
            }
            StatusCode::OK => {
                let body = resp
                    .text()
                    .await
                    .map_err(|e| ArkaError::Mpp(format!("Failed to read response: {e}")))?;
                Ok(MppResponse::Success(body))
            }
            status => Err(ArkaError::Mpp(format!("Unexpected status: {status}"))),
        }
    }

    // TODO: pay() — execute payment based on PaymentOptions
    // TODO: session() — create an MPP session ("OAuth for money")
    // TODO: pay_and_retry() — full 402 flow: request → pay → retry with credential
}

impl Default for MppClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from an MPP-enabled endpoint.
pub enum MppResponse {
    /// Server returned 200 OK with content.
    Success(String),
    /// Server returned 402 Payment Required with options.
    PaymentRequired(PaymentOptions),
}
