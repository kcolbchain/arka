//! Payment module — x402 and MPP payment support for autonomous agents.
//!
//! Provides a unified `Pay` trait and implementations for two protocols:
//! - `x402`: HTTP 402-based pay-per-call (agent ↔ HTTP server)
//! - `mpp`: Machine Payments Protocol (agent ↔ agent / merchant)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub mod x402;

/// A payment provider that can charge and verify payments.
#[async_trait]
pub trait Pay: Send + Sync {
    /// The type of receipt returned by this provider.
    type Receipt: std::fmt::Debug + Send;

    /// Get the provider name.
    fn provider(&self) -> &str;

    /// Check if a receiver address/url supports this payment method.
    async fn supports(&self, receiver: &str) -> Result<bool>;

    /// Pay a specific amount to a receiver.
    async fn pay(&self, receiver: &str, amount: &str, currency: &str) -> Result<PaymentResult<Self::Receipt>>;
}

/// Result of a payment operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult<R> {
    pub success: bool,
    pub receipt: Option<R>,
    pub error: Option<String>,
}

/// A unified receipt type covering x402 and MPP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    pub id: String,
    pub amount: String,
    pub currency: String,
    pub tx_hash: Option<String>,
    pub timestamp: u64,
    pub provider: String,
}
