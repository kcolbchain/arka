//! Switchboard x402 client — Rust ↔ Python interop.
//!
//! This example shows an arka-based agent calling a paid endpoint served
//! by a Python switchboard middleware. The wire is HTTP-402 + signed
//! payment proofs — a language-neutral protocol that both sides speak.
//!
//! Why this matters: switchboard is the Python reference impl of the
//! agent-payment substrate. arka is the Rust agent SDK. They don't share
//! any code, but they share the wire format. This file is the
//! cross-language conformance demo for it.
//!
//! # Running the demo
//!
//! Spin up the switchboard middleware on a paid route (Python side):
//!
//! ```bash
//! # In the switchboard repo:
//! pip install -e '.[dev]'
//! python -m switchboard.x402_middleware.serve --port 8402 --price 1000
//! # Now `http://localhost:8402/agent-only` returns 402 on first hit.
//! ```
//!
//! Then run this example (Rust side):
//!
//! ```bash
//! cargo run --example switchboard_x402_client -- \
//!     --endpoint http://localhost:8402/agent-only
//! ```
//!
//! On a cold call you'll see:
//!   - GET → 402 Payment Required + `X-Payment-Required` header
//!   - parse the offer, build a payment proof, retry with `X-Payment-Proof`
//!   - GET → 200 + body
//!
//! # Wire compatibility
//!
//! The HTTP envelopes here MUST match `switchboard.x402_middleware`'s
//! `PaymentOffer.to_header()` and `PaymentProof.from_header()` byte for
//! byte. If switchboard ever changes the envelope, this client breaks
//! and the cross-repo conformance test (planned in switchboard
//! `tests/conformance/`) catches it.
//!
//! # Related
//!
//! - switchboard docs:        https://github.com/kcolbchain/switchboard
//! - switchboard lab:         https://kcolbchain.github.io/switchboard/agents-demo.html
//! - x402 upstream spec:      https://www.x402.org

use std::time::{SystemTime, UNIX_EPOCH};

use arka::prelude::*;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// One entry of the server's `accepts[]` envelope. Wire-compatible with
/// `switchboard.x402_middleware.PaymentOffer.to_header()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentOffer {
    amount: String, // uint256 as decimal string
    currency: String,
    recipient: String,
    chain_id: u64,
    #[serde(default)]
    scheme: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    expires_at: Option<u64>,
}

/// What we send back. Wire-compatible with
/// `switchboard.x402_middleware.PaymentProof.from_header()`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentProof {
    tx_hash: String,
    chain_id: u64,
    payer: String,
    amount: String,
    nonce: String,
    timestamp: u64,
}

fn parse_args() -> String {
    let mut args = std::env::args().skip(1);
    let mut endpoint = String::from("http://localhost:8402/agent-only");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--endpoint" => {
                if let Some(v) = args.next() {
                    endpoint = v;
                }
            }
            "-h" | "--help" => {
                eprintln!("Usage: switchboard_x402_client [--endpoint URL]");
                std::process::exit(0);
            }
            _ => {}
        }
    }
    endpoint
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let endpoint = parse_args();

    // Fresh ephemeral wallet for this demo — in production this would be
    // a long-lived key fetched from arka's wallet manager.
    let wallet = Wallet::generate()?;
    let agent = Agent::builder()
        .chain(Chain::Base)
        .wallet(wallet)
        .build()
        .await?;
    let payer_addr = format!("{:?}", agent.address());
    println!("Agent ready on {} at {}", agent.chain(), payer_addr);

    let client = reqwest::Client::new();

    // ── First attempt — expect 402 ────────────────────────────────────
    let resp = client.get(&endpoint).send().await?;

    if resp.status() != StatusCode::PAYMENT_REQUIRED {
        // Switchboard returned something else — either 200 already (the
        // server doesn't gate this route), or some other error. Print and
        // exit.
        println!("Unexpected status (no 402 dance needed): {}", resp.status());
        let body = resp.text().await.unwrap_or_default();
        println!("body: {body}");
        return Ok(());
    }

    // Parse the 402 envelope. Switchboard ships it via the
    // `X-Payment-Required` header as a JSON object with an `accepts: []`
    // array — we pick the first entry.
    let header = resp
        .headers()
        .get("X-Payment-Required")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    println!("402 envelope: {header}");

    #[derive(Deserialize)]
    struct Envelope {
        accepts: Vec<PaymentOffer>,
    }
    let env: Envelope = serde_json::from_str(&header)?;
    let offer = env
        .accepts
        .into_iter()
        .next()
        .ok_or_else(|| ArkaError::Mpp("empty accepts[]".into()))?;
    println!(
        "Server wants {} {} on chain_id={} to {}\n  scheme={} description={:?} expires_at={:?}",
        offer.amount, offer.currency, offer.chain_id, offer.recipient,
        offer.scheme, offer.description, offer.expires_at
    );

    // ── Settle on-chain (stubbed in this demo) ────────────────────────
    // In a real flow this is where arka would issue the actual ETH /
    // ERC-20 transfer via `agent.send_value(...)` or the AgentEscrow
    // contract from switchboard. For a demo against a local middleware
    // running in "mock-settle" mode, an ephemeral tx-hash is sufficient.
    let tx_hash = format!("0x{:064x}", rand::random::<u128>() as u128);

    let proof = PaymentProof {
        tx_hash,
        chain_id: offer.chain_id,
        payer: payer_addr,
        amount: offer.amount.clone(),
        nonce: offer.nonce.clone(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // ── Retry with the proof ──────────────────────────────────────────
    let mut headers = HeaderMap::new();
    let proof_json = serde_json::to_string(&proof)?;
    headers.insert(
        "X-Payment-Proof",
        HeaderValue::from_str(&proof_json)
            .map_err(|e| ArkaError::Mpp(format!("invalid header value: {e}")))?,
    );

    let resp2 = client.get(&endpoint).headers(headers).send().await?;
    let status = resp2.status();
    let body = resp2.text().await.unwrap_or_default();
    println!("After proof: {status} body={body}");

    Ok(())
}
