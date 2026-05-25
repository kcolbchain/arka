//! Ritual Infernet integration.
//!
//! arka agents can request verifiable on-chain AI inference via
//! [Ritual's Infernet](https://ritual.net/) — a decentralised oracle
//! network of 8,000+ nodes for AI workloads.
//!
//! An agent calls `infernet.request_inference(container_id, payload)`
//! and gets back a `ComputeResult` with the output + cryptographic proof.
//!
//! Payment can be composed with `arka::pay::x402` so the agent pays
//! for the inference in USDC.
//!
//! References:
//! - Infernet SDK: https://github.com/ritual-net/infernet-sdk
//! - Consumer pattern: https://www.ritualfoundation.org/docs/architecture/infernet-to-chain

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{ArkaError, Result};

/// Result of an Infernet computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResult {
    pub container_id: String,
    pub output: serde_json::Value,
    pub proof: Option<Vec<u8>>,
    pub node_id: String,
    pub timestamp: u64,
}

/// A request to the Infernet network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub container_id: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_gas_price_gwei: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_contract: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<serde_json::Value>,
}

/// Client for interacting with Ritual Infernet.
pub struct InfernetClient {
    /// Infernet node HTTP endpoint.
    endpoint: String,
    /// Optional API key for authenticated access.
    api_key: Option<String>,
    http: reqwest::Client,
}

impl InfernetClient {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            api_key: None,
            http: reqwest::Client::new(),
        }
    }

    pub fn with_api_key(endpoint: &str, api_key: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            api_key: Some(api_key.to_string()),
            http: reqwest::Client::new(),
        }
    }

    /// Request AI inference from an Infernet container.
    pub async fn request_inference(
        &self,
        container_id: &str,
        payload: serde_json::Value,
    ) -> Result<ComputeResult> {
        let request = InferenceRequest {
            container_id: container_id.to_string(),
            payload,
            max_gas_price_gwei: None,
            callback_contract: None,
            callback_data: None,
        };

        let mut req = self.http.post(&format!("{}/api/v1/compute", self.endpoint))
            .json(&request);

        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await
            .map_err(|e| ArkaError::Chain(format!("Infernet request failed: {e}")))?;

        let result: ComputeResult = resp.json().await
            .map_err(|e| ArkaError::Chain(format!("Infernet response parse failed: {e}")))?;

        Ok(result)
    }

    /// Request inference and wait for on-chain delivery (poll-based).
    pub async fn request_and_await(
        &self,
        container_id: &str,
        payload: serde_json::Value,
        poll_interval_ms: u64,
        max_attempts: u32,
    ) -> Result<ComputeResult> {
        let request = InferenceRequest {
            container_id: container_id.to_string(),
            payload,
            max_gas_price_gwei: None,
            callback_contract: None,
            callback_data: None,
        };

        let mut req = self.http.post(&format!("{}/api/v1/compute/await", self.endpoint))
            .json(&request);

        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        // Request with auto-polling on the Infernet side
        let resp = req
            .query(&[
                ("poll_interval_ms", poll_interval_ms.to_string()),
                ("max_attempts", max_attempts.to_string()),
            ])
            .send()
            .await
            .map_err(|e| ArkaError::Chain(format!("Infernet await failed: {e}")))?;

        let result: ComputeResult = resp.json().await
            .map_err(|e| ArkaError::Chain(format!("Infernet await parse failed: {e}")))?;

        Ok(result)
    }

    /// List available containers on the Infernet node.
    pub async fn list_containers(&self) -> Result<HashMap<String, serde_json::Value>> {
        let resp = self.http.get(&format!("{}/api/v1/containers", self.endpoint))
            .send()
            .await
            .map_err(|e| ArkaError::Chain(format!("Infernet list containers failed: {e}")))?;

        let containers: HashMap<String, serde_json::Value> = resp.json().await
            .map_err(|e| ArkaError::Chain(format!("Infernet containers parse failed: {e}")))?;

        Ok(containers)
    }
}
