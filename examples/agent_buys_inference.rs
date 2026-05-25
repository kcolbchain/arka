//! x402 example — agent buys inference via HTTP 402 payment.
//!
//! Run with: `cargo run --features x402 --example agent_buys_inference`
//!
//! Requires an x402-enabled inference endpoint.

use arka::prelude::*;
use arka::wallet::EvmWallet;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = EvmWallet::generate()?;
    println!("Agent wallet: {}", wallet.pubkey());

    #[cfg(feature = "x402")]
    {
        use arka::pay::x402::X402Client;

        let x402 = X402Client::new(Box::new(wallet));

        // Probe an inference endpoint
        let endpoint = std::env::var("INFERENCE_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:8402/agent-only".into());

        let supported = x402.supports(&endpoint).await?;
        println!("Endpoint supports x402: {supported}");

        if supported {
            let result = x402.pay(&endpoint, "1000", "USDC").await?;
            println!("Payment result: success={}", result.success);
            if let Some(receipt) = result.receipt {
                println!("Receipt: {:?}", receipt);
            }
        }
    }

    #[cfg(not(feature = "x402"))]
    println!("x402 feature not enabled. Run with --features x402");

    Ok(())
}
