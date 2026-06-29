//! MPP / x402 wire types — payment offers and signed payment proofs.
//!
//! These are the language-neutral envelopes exchanged over HTTP-402. They are
//! wire-compatible with `switchboard.x402_middleware` (the Python reference
//! impl) and the upstream x402 pattern (<https://www.x402.org>):
//!
//!   - A server gates a route and answers a cold request with `402 Payment
//!     Required` plus an `X-Payment-Required` header carrying an
//!     [`OfferEnvelope`] (`{ accepts: [PaymentOffer, ...] }`).
//!   - A paying agent picks an offer, settles it (on-chain or via a facilitator),
//!     and retries the request with an `X-Payment-Proof` header carrying a
//!     signed [`PaymentProof`].
//!   - The server (or its facilitator) verifies the proof binds to the offer and
//!     is signed by the declared payer before serving the resource.
//!
//! All structs serialize with `camelCase` keys to match the switchboard wire.

use std::time::{SystemTime, UNIX_EPOCH};

use alloy::primitives::{Address, Signature};
use alloy::signers::SignerSync;
use serde::{Deserialize, Serialize};

use crate::chain::Chain;
use crate::error::{ArkaError, Result};
use crate::wallet::Wallet;

/// HTTP header carrying the 402 offer envelope (server → client).
pub const HEADER_PAYMENT_REQUIRED: &str = "X-Payment-Required";

/// HTTP header carrying the signed payment proof (client → server).
pub const HEADER_PAYMENT_PROOF: &str = "X-Payment-Proof";

/// The default x402 settlement scheme: an on-chain transfer whose hash is
/// presented as the proof of payment.
pub const SCHEME_EXACT: &str = "exact";

/// Current Unix time in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A single payment option a server is willing to accept for a gated resource.
///
/// One entry of an [`OfferEnvelope`]'s `accepts[]` array. Built server-side via
/// [`PaymentOffer::builder`] and serialized into the `X-Payment-Required`
/// header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOffer {
    /// Amount owed, as a `uint256` decimal string in the currency's smallest
    /// unit (e.g. USDC has 6 decimals, so `"1000"` == 0.001 USDC).
    pub amount: String,
    /// Currency / asset identifier (e.g. `"USDC"`, or a token contract address).
    pub currency: String,
    /// Address that must receive the payment.
    pub recipient: String,
    /// EVM chain the payment must settle on.
    pub chain_id: u64,
    /// Settlement scheme (default [`SCHEME_EXACT`]).
    #[serde(default)]
    pub scheme: String,
    /// Human-readable description of the resource being paid for.
    #[serde(default)]
    pub description: String,
    /// Server-chosen nonce binding a proof to this specific offer. Replay
    /// protection: a verifier should accept a given nonce at most once.
    #[serde(default)]
    pub nonce: String,
    /// Unix timestamp (seconds) after which this offer is no longer valid.
    #[serde(default)]
    pub expires_at: Option<u64>,
}

impl PaymentOffer {
    /// Start building an offer for `amount` of `currency` to `recipient` on
    /// `chain`.
    pub fn builder(
        amount: impl Into<String>,
        currency: impl Into<String>,
        recipient: impl Into<String>,
        chain: Chain,
    ) -> PaymentOfferBuilder {
        PaymentOfferBuilder {
            amount: amount.into(),
            currency: currency.into(),
            recipient: recipient.into(),
            chain_id: chain.chain_id(),
            scheme: SCHEME_EXACT.to_string(),
            description: String::new(),
            nonce: None,
            ttl_secs: None,
        }
    }

    /// Whether this offer has expired relative to `now` (Unix seconds). An
    /// offer with no `expires_at` never expires.
    pub fn is_expired_at(&self, now: u64) -> bool {
        matches!(self.expires_at, Some(exp) if now > exp)
    }

    /// Whether this offer has expired relative to the current system clock.
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(now_secs())
    }

    /// Wrap this single offer in an [`OfferEnvelope`] ready for the
    /// `X-Payment-Required` header.
    pub fn into_envelope(self) -> OfferEnvelope {
        OfferEnvelope {
            accepts: vec![self],
        }
    }
}

/// Builder for [`PaymentOffer`]. Pick a default-protected nonce + expiry, or
/// override them explicitly.
#[derive(Debug, Clone)]
pub struct PaymentOfferBuilder {
    amount: String,
    currency: String,
    recipient: String,
    chain_id: u64,
    scheme: String,
    description: String,
    nonce: Option<String>,
    ttl_secs: Option<u64>,
}

impl PaymentOfferBuilder {
    /// Set the settlement scheme (defaults to [`SCHEME_EXACT`]).
    pub fn scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = scheme.into();
        self
    }

    /// Set the human-readable description of the gated resource.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set an explicit nonce. If unset, [`build`](Self::build) generates a
    /// random one.
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }

    /// Expire this offer `ttl_secs` from build time. If unset, the offer does
    /// not carry an expiry.
    pub fn ttl_secs(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = Some(ttl_secs);
        self
    }

    /// Finalize the offer, generating a random nonce when none was supplied and
    /// resolving the TTL against the current clock.
    pub fn build(self) -> PaymentOffer {
        let nonce = self.nonce.unwrap_or_else(|| {
            // 128 random bits, hex-encoded — collision-resistant for nonces.
            let bits: u128 = rand::random();
            format!("0x{bits:032x}")
        });
        let expires_at = self.ttl_secs.map(|ttl| now_secs() + ttl);
        PaymentOffer {
            amount: self.amount,
            currency: self.currency,
            recipient: self.recipient,
            chain_id: self.chain_id,
            scheme: self.scheme,
            description: self.description,
            nonce,
            expires_at,
        }
    }
}

/// The `X-Payment-Required` payload: the set of offers a server will accept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferEnvelope {
    pub accepts: Vec<PaymentOffer>,
}

impl OfferEnvelope {
    /// Serialize to the JSON string carried by the `X-Payment-Required` header.
    pub fn to_header(&self) -> Result<String> {
        serde_json::to_string(self).map_err(ArkaError::from)
    }

    /// Parse an envelope from an `X-Payment-Required` header value.
    pub fn from_header(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(ArkaError::from)
    }

    /// The first offer in the envelope, if any. Clients typically settle the
    /// first acceptable option.
    pub fn first(&self) -> Option<&PaymentOffer> {
        self.accepts.first()
    }
}

/// A signed claim that a payer settled a [`PaymentOffer`].
///
/// Constructed client-side by [`PaymentProof::sign`] after settlement, sent
/// back in the `X-Payment-Proof` header, and checked server-side by
/// [`PaymentProof::verify`].
///
/// The signature is an EIP-191 (`personal_sign`) signature over the
/// [canonical message](PaymentProof::canonical_message), which binds every
/// economically meaningful field. Tampering with any bound field makes the
/// recovered signer address diverge from `payer`, so verification rejects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProof {
    /// On-chain transaction hash that settled the payment.
    pub tx_hash: String,
    /// Chain the settlement transaction landed on.
    pub chain_id: u64,
    /// Address that paid (and signed this proof).
    pub payer: String,
    /// Recipient the payment was sent to (copied from the offer).
    pub recipient: String,
    /// Amount paid, decimal string (copied from the offer).
    pub amount: String,
    /// Currency paid (copied from the offer).
    pub currency: String,
    /// Offer nonce this proof answers (copied from the offer).
    pub nonce: String,
    /// Unix timestamp (seconds) when the proof was signed.
    pub timestamp: u64,
    /// `0x`-prefixed hex of the 65-byte EIP-191 signature over the canonical
    /// message.
    pub signature: String,
}

impl PaymentProof {
    /// The exact byte string that is signed and recovered. Deterministic and
    /// stable across implementations — switchboard computes the same string.
    ///
    /// Fields are joined with `\n` in a fixed order. Including the scheme tag
    /// (`x402-proof-v1`) domain-separates these signatures from any other
    /// message the same key might sign.
    pub fn canonical_message(
        tx_hash: &str,
        chain_id: u64,
        payer: &str,
        recipient: &str,
        amount: &str,
        currency: &str,
        nonce: &str,
    ) -> String {
        format!(
            "x402-proof-v1\n{tx_hash}\n{chain_id}\n{payer}\n{recipient}\n{amount}\n{currency}\n{nonce}"
        )
    }

    /// The canonical message for this proof instance.
    fn message(&self) -> String {
        Self::canonical_message(
            &self.tx_hash,
            self.chain_id,
            &self.payer,
            &self.recipient,
            &self.amount,
            &self.currency,
            &self.nonce,
        )
    }

    /// Construct and sign a proof that `wallet` settled `offer` via the on-chain
    /// transaction `tx_hash`.
    ///
    /// The payer address is taken from the wallet, and the recipient / amount /
    /// currency / nonce are copied from the offer so the proof is bound to it.
    pub fn sign(wallet: &Wallet, offer: &PaymentOffer, tx_hash: impl Into<String>) -> Result<Self> {
        let tx_hash = tx_hash.into();
        let payer = format!("{:?}", wallet.address());
        let timestamp = now_secs();

        let message = Self::canonical_message(
            &tx_hash,
            offer.chain_id,
            &payer,
            &offer.recipient,
            &offer.amount,
            &offer.currency,
            &offer.nonce,
        );

        let signature: Signature = wallet
            .signer()
            .sign_message_sync(message.as_bytes())
            .map_err(|e| ArkaError::Mpp(format!("failed to sign payment proof: {e}")))?;

        Ok(Self {
            tx_hash,
            chain_id: offer.chain_id,
            payer,
            recipient: offer.recipient.clone(),
            amount: offer.amount.clone(),
            currency: offer.currency.clone(),
            nonce: offer.nonce.clone(),
            timestamp,
            signature: format!("0x{}", hex::encode(signature.as_bytes())),
        })
    }

    /// Recover the address that signed this proof from its signature and
    /// canonical message.
    pub fn recover_signer(&self) -> Result<Address> {
        let raw = self.signature.strip_prefix("0x").unwrap_or(&self.signature);
        let bytes = hex::decode(raw)
            .map_err(|e| ArkaError::Mpp(format!("proof signature not valid hex: {e}")))?;
        let sig = Signature::from_raw(&bytes)
            .map_err(|e| ArkaError::Mpp(format!("proof signature malformed: {e}")))?;
        sig.recover_address_from_msg(self.message().as_bytes())
            .map_err(|e| ArkaError::Mpp(format!("could not recover proof signer: {e}")))
    }

    /// Serialize to the JSON string carried by the `X-Payment-Proof` header.
    pub fn to_header(&self) -> Result<String> {
        serde_json::to_string(self).map_err(ArkaError::from)
    }

    /// Parse a proof from an `X-Payment-Proof` header value.
    pub fn from_header(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(ArkaError::from)
    }

    /// Verify this proof satisfies `offer` as of `now` (Unix seconds).
    ///
    /// Checks, in order:
    ///   1. the proof's offer-bound fields (chain, recipient, amount, currency,
    ///      nonce) match the offer it claims to answer;
    ///   2. the offer has not expired;
    ///   3. the signature recovers to exactly the declared `payer` address.
    ///
    /// On any failure returns an [`ArkaError::Mpp`] describing the rejection.
    pub fn verify_at(&self, offer: &PaymentOffer, now: u64) -> Result<()> {
        if self.chain_id != offer.chain_id {
            return Err(ArkaError::Mpp(format!(
                "chain mismatch: proof on {} but offer wants {}",
                self.chain_id, offer.chain_id
            )));
        }
        if !addr_eq(&self.recipient, &offer.recipient) {
            return Err(ArkaError::Mpp(format!(
                "recipient mismatch: proof paid {} but offer wants {}",
                self.recipient, offer.recipient
            )));
        }
        if self.amount != offer.amount {
            return Err(ArkaError::Mpp(format!(
                "amount mismatch: proof claims {} but offer wants {}",
                self.amount, offer.amount
            )));
        }
        if self.currency != offer.currency {
            return Err(ArkaError::Mpp(format!(
                "currency mismatch: proof in {} but offer wants {}",
                self.currency, offer.currency
            )));
        }
        if self.nonce != offer.nonce {
            return Err(ArkaError::Mpp(format!(
                "nonce mismatch: proof has {} but offer issued {}",
                self.nonce, offer.nonce
            )));
        }
        if offer.is_expired_at(now) {
            return Err(ArkaError::Mpp(format!(
                "offer expired at {:?} (now {now})",
                offer.expires_at
            )));
        }

        let recovered = self.recover_signer()?;
        if !addr_eq(&format!("{recovered:?}"), &self.payer) {
            return Err(ArkaError::Mpp(format!(
                "signature does not match payer: recovered {recovered:?}, claimed {}",
                self.payer
            )));
        }
        Ok(())
    }

    /// Verify this proof against `offer` using the current system clock.
    pub fn verify(&self, offer: &PaymentOffer) -> Result<()> {
        self.verify_at(offer, now_secs())
    }
}

/// Case-insensitive comparison of two hex addresses (EVM addresses are
/// case-insensitive aside from EIP-55 checksumming).
fn addr_eq(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_offer() -> PaymentOffer {
        PaymentOffer::builder(
            "1000",
            "USDC",
            "0x000000000000000000000000000000000000dEaD",
            Chain::Base,
        )
        .description("agent-only resource")
        .nonce("0xabc123")
        .ttl_secs(300)
        .build()
    }

    #[test]
    fn offer_builds_with_expected_fields() {
        let offer = test_offer();
        assert_eq!(offer.amount, "1000");
        assert_eq!(offer.currency, "USDC");
        assert_eq!(offer.chain_id, Chain::Base.chain_id());
        assert_eq!(offer.scheme, SCHEME_EXACT);
        assert_eq!(offer.nonce, "0xabc123");
        assert!(offer.expires_at.is_some());
        assert!(!offer.is_expired_at(now_secs()));
    }

    #[test]
    fn offer_builder_generates_unique_nonces() {
        let a = PaymentOffer::builder("1", "USDC", "0xdead", Chain::Base).build();
        let b = PaymentOffer::builder("1", "USDC", "0xdead", Chain::Base).build();
        assert_ne!(a.nonce, b.nonce, "auto nonces must be unique");
        assert!(a.nonce.starts_with("0x"));
    }

    #[test]
    fn offer_envelope_roundtrips_through_header() {
        let env = test_offer().into_envelope();
        let header = env.to_header().unwrap();
        // camelCase wire keys (switchboard compatibility).
        assert!(header.contains("\"chainId\""));
        assert!(header.contains("\"expiresAt\""));
        let parsed = OfferEnvelope::from_header(&header).unwrap();
        assert_eq!(parsed, env);
        assert_eq!(parsed.first().unwrap().nonce, "0xabc123");
    }

    #[test]
    fn proof_signs_and_verifies_against_offer() {
        let wallet = Wallet::generate().unwrap();
        let offer = test_offer();
        let proof = PaymentProof::sign(&wallet, &offer, "0xdeadbeef").unwrap();

        assert_eq!(proof.payer, format!("{:?}", wallet.address()));
        assert_eq!(proof.amount, offer.amount);
        assert_eq!(proof.nonce, offer.nonce);
        assert!(proof.signature.starts_with("0x"));

        // Recovered signer is exactly the paying wallet.
        assert_eq!(proof.recover_signer().unwrap(), wallet.address());
        // Full verification passes.
        proof.verify(&offer).unwrap();
    }

    #[test]
    fn proof_roundtrips_through_header() {
        let wallet = Wallet::generate().unwrap();
        let offer = test_offer();
        let proof = PaymentProof::sign(&wallet, &offer, "0xfeed").unwrap();
        let header = proof.to_header().unwrap();
        assert!(header.contains("\"txHash\""));
        let parsed = PaymentProof::from_header(&header).unwrap();
        assert_eq!(parsed, proof);
        parsed.verify(&offer).unwrap();
    }

    #[test]
    fn verify_rejects_tampered_amount() {
        let wallet = Wallet::generate().unwrap();
        let offer = test_offer();
        let mut proof = PaymentProof::sign(&wallet, &offer, "0x01").unwrap();
        // Attacker inflates the claimed amount after signing.
        proof.amount = "999999".to_string();
        let err = proof.verify(&offer).unwrap_err();
        assert!(
            matches!(err, ArkaError::Mpp(_)),
            "expected Mpp error, got {err:?}"
        );
        assert!(err.to_string().contains("amount mismatch"));
    }

    #[test]
    fn verify_rejects_wrong_signer() {
        let payer = Wallet::generate().unwrap();
        let attacker = Wallet::generate().unwrap();
        let offer = test_offer();
        let mut proof = PaymentProof::sign(&payer, &offer, "0x02").unwrap();
        // Attacker claims a different (their own) payer address but keeps the
        // payer's signature.
        proof.payer = format!("{:?}", attacker.address());
        let err = proof.verify(&offer).unwrap_err();
        assert!(
            err.to_string().contains("does not match payer"),
            "got {err}"
        );
    }

    #[test]
    fn verify_rejects_forged_signature() {
        let payer = Wallet::generate().unwrap();
        let attacker = Wallet::generate().unwrap();
        let offer = test_offer();
        // Attacker forges a proof for a payer address they don't control.
        let mut proof = PaymentProof::sign(&attacker, &offer, "0x03").unwrap();
        proof.payer = format!("{:?}", payer.address());
        let err = proof.verify(&offer).unwrap_err();
        assert!(
            err.to_string().contains("does not match payer"),
            "got {err}"
        );
    }

    #[test]
    fn verify_rejects_expired_offer() {
        let wallet = Wallet::generate().unwrap();
        let offer = PaymentOffer::builder("1000", "USDC", "0xdead", Chain::Base)
            .nonce("0xn")
            .ttl_secs(60)
            .build();
        let proof = PaymentProof::sign(&wallet, &offer, "0x04").unwrap();
        let exp = offer.expires_at.unwrap();
        // Verify well past expiry.
        let err = proof.verify_at(&offer, exp + 1).unwrap_err();
        assert!(err.to_string().contains("expired"), "got {err}");
        // Still valid before expiry.
        proof.verify_at(&offer, exp).unwrap();
    }

    #[test]
    fn verify_rejects_nonce_mismatch() {
        let wallet = Wallet::generate().unwrap();
        let offer = test_offer();
        let proof = PaymentProof::sign(&wallet, &offer, "0x05").unwrap();
        // Server checks the proof against a different (fresh) offer.
        let other = PaymentOffer::builder("1000", "USDC", offer.recipient.clone(), Chain::Base)
            .nonce("0xdifferent")
            .build();
        let err = proof.verify(&other).unwrap_err();
        assert!(err.to_string().contains("nonce mismatch"), "got {err}");
    }
}
