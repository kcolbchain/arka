//! MPP (Machine Payments Protocol) — the HTTP-402 / x402 payment flow.
//!
//! This module is arka's machine-payments core: the types and flow an
//! autonomous agent uses to pay for a gated HTTP resource, and that a server
//! uses to gate one.
//!
//! ## The flow
//!
//! ```text
//!   client                              server (gated route)
//!     │  GET /resource                       │
//!     │ ───────────────────────────────────► │
//!     │                                       │  build a PaymentOffer
//!     │  402 + X-Payment-Required: {accepts}  │
//!     │ ◄─────────────────────────────────── │
//!     │  settle on-chain → tx_hash            │
//!     │  sign PaymentProof over the offer     │
//!     │  GET /resource                        │
//!     │  X-Payment-Proof: {signed proof}      │
//!     │ ───────────────────────────────────► │
//!     │                                       │  PaymentProof::verify(&offer)
//!     │  200 + body                           │
//!     │ ◄─────────────────────────────────── │
//! ```
//!
//! - [`PaymentOffer`] / [`OfferEnvelope`] — the server side (build a 402).
//! - [`PaymentProof`] — the client side (construct + sign), and the verifier
//!   side ([`PaymentProof::verify`]).
//!
//! The wire envelopes are camelCase JSON in the `X-Payment-Required` /
//! `X-Payment-Proof` headers, wire-compatible with the switchboard Python
//! reference impl and the upstream x402 pattern (<https://www.x402.org>).
//! See `examples/switchboard_x402_client.rs` for the end-to-end interop demo.

mod types;

pub use types::{
    OfferEnvelope, PaymentOffer, PaymentOfferBuilder, PaymentProof, HEADER_PAYMENT_PROOF,
    HEADER_PAYMENT_REQUIRED, SCHEME_EXACT,
};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::error::{ArkaError, Result};
use crate::wallet::Wallet;

/// MPP payment client for autonomous agent payments over HTTP-402.
pub struct MppClient {
    http: reqwest::Client,
}

/// A settlement function: given the offer the server presented, perform the
/// actual on-chain payment and return the settling transaction hash.
///
/// arka keeps settlement pluggable so the 402 flow is agnostic to *how* a
/// payment is made — a native transfer, an ERC-20 transfer, an escrow contract,
/// or (in tests / demos) a mock that returns a synthetic hash.
pub trait Settle {
    /// Settle `offer`, returning the transaction hash that paid it.
    fn settle(&self, offer: &PaymentOffer) -> Result<String>;
}

impl<F> Settle for F
where
    F: Fn(&PaymentOffer) -> Result<String>,
{
    fn settle(&self, offer: &PaymentOffer) -> Result<String> {
        (self)(offer)
    }
}

impl MppClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Make a request to an MPP-enabled endpoint.
    ///
    /// On `402 Payment Required`, parses the offer envelope from the
    /// `X-Payment-Required` header (falling back to the JSON body for servers
    /// that put it there). On `200 OK`, returns the body.
    pub async fn request(&self, url: &str) -> Result<MppResponse> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ArkaError::Mpp(format!("request failed: {e}")))?;

        match resp.status() {
            StatusCode::PAYMENT_REQUIRED => {
                let header = resp
                    .headers()
                    .get(HEADER_PAYMENT_REQUIRED)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);

                let envelope = match header {
                    Some(h) => OfferEnvelope::from_header(&h)?,
                    None => {
                        // Some servers carry the envelope in the body instead.
                        let body = resp
                            .text()
                            .await
                            .map_err(|e| ArkaError::Mpp(format!("failed to read 402 body: {e}")))?;
                        OfferEnvelope::from_header(&body).map_err(|_| {
                            ArkaError::Mpp(
                                "402 response carried no parseable payment offer".to_string(),
                            )
                        })?
                    }
                };
                Ok(MppResponse::PaymentRequired(envelope))
            }
            StatusCode::OK => {
                let body = resp
                    .text()
                    .await
                    .map_err(|e| ArkaError::Mpp(format!("failed to read response: {e}")))?;
                Ok(MppResponse::Success(body))
            }
            status => Err(ArkaError::Mpp(format!("unexpected status: {status}"))),
        }
    }

    /// The full 402 dance: request `url`; if the server demands payment, pick
    /// the first offer, settle it via `settle`, sign a [`PaymentProof`] with
    /// `wallet`, and retry with the `X-Payment-Proof` header.
    ///
    /// Returns the resource body on success. If the first request already
    /// returns `200`, settlement is skipped and the body is returned directly.
    pub async fn pay_and_retry(
        &self,
        url: &str,
        wallet: &Wallet,
        settle: &impl Settle,
    ) -> Result<String> {
        let envelope = match self.request(url).await? {
            MppResponse::Success(body) => return Ok(body),
            MppResponse::PaymentRequired(env) => env,
        };

        let offer = envelope
            .first()
            .ok_or_else(|| ArkaError::Mpp("server offered no payment options".to_string()))?;

        if offer.is_expired() {
            return Err(ArkaError::Mpp("payment offer already expired".to_string()));
        }

        let tx_hash = settle.settle(offer)?;
        let proof = PaymentProof::sign(wallet, offer, tx_hash)?;

        // Belt-and-suspenders: a client should never send a proof it cannot
        // itself verify against the offer.
        proof.verify(offer)?;

        let resp = self
            .http
            .get(url)
            .header(HEADER_PAYMENT_PROOF, proof.to_header()?)
            .send()
            .await
            .map_err(|e| ArkaError::Mpp(format!("retry-with-proof failed: {e}")))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ArkaError::Mpp(format!("failed to read paid response: {e}")))?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(ArkaError::Mpp(format!(
                "payment rejected by server ({status}): {body}"
            )))
        }
    }
}

impl Default for MppClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from an MPP-enabled endpoint.
#[derive(Debug, Clone)]
pub enum MppResponse {
    /// Server returned `200 OK` with content.
    Success(String),
    /// Server returned `402 Payment Required` with an offer envelope.
    PaymentRequired(OfferEnvelope),
}

/// Receipt from a completed payment — what a verifier records once a proof
/// checks out, suitable for logging / accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentReceipt {
    pub tx_hash: String,
    pub payer: String,
    pub recipient: String,
    pub amount: String,
    pub currency: String,
    pub nonce: String,
    pub timestamp: u64,
}

impl PaymentReceipt {
    /// Build a receipt from a proof that has already been verified.
    pub fn from_proof(proof: &PaymentProof) -> Self {
        Self {
            tx_hash: proof.tx_hash.clone(),
            payer: proof.payer.clone(),
            recipient: proof.recipient.clone(),
            amount: proof.amount.clone(),
            currency: proof.currency.clone(),
            nonce: proof.nonce.clone(),
            timestamp: proof.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;

    /// End-to-end of the in-process pieces of the 402 flow: a server builds an
    /// offer, a client settles + signs a proof, the server verifies it and
    /// issues a receipt. No network involved.
    #[test]
    fn offer_settle_prove_verify_roundtrip() {
        // ── server: build a 402 offer ──
        let offer = PaymentOffer::builder(
            "1000",
            "USDC",
            "0x000000000000000000000000000000000000dEaD",
            Chain::Base,
        )
        .description("premium feed")
        .ttl_secs(120)
        .build();
        let envelope = offer.clone().into_envelope();
        let header = envelope.to_header().unwrap();

        // ── client: parse offer, settle, sign proof ──
        let parsed = OfferEnvelope::from_header(&header).unwrap();
        let client_offer = parsed.first().unwrap();
        let wallet = Wallet::generate().unwrap();

        let settle = |o: &PaymentOffer| -> Result<String> {
            assert_eq!(o.amount, "1000");
            Ok("0xc0ffee".to_string())
        };
        let tx_hash = settle.settle(client_offer).unwrap();
        let proof = PaymentProof::sign(&wallet, client_offer, tx_hash).unwrap();

        // ── server: verify proof against the offer it issued ──
        proof.verify(&offer).unwrap();
        let receipt = PaymentReceipt::from_proof(&proof);
        assert_eq!(receipt.amount, "1000");
        assert_eq!(receipt.payer, format!("{:?}", wallet.address()));
        assert_eq!(receipt.tx_hash, "0xc0ffee");
    }

    #[test]
    fn settle_closure_implements_settle_trait() {
        let offer = PaymentOffer::builder("5", "USDC", "0xdead", Chain::Base).build();
        let settle = |_: &PaymentOffer| Ok("0xabc".to_string());
        assert_eq!(settle.settle(&offer).unwrap(), "0xabc");
    }
}
