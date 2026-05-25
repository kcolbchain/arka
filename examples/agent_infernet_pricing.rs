//! Infernet pricing example — agent requests a Meridian pricing signal via
//! an Infernet container and uses it to quote.
//!
//! Run with: `cargo run --features ritual --example agent_infernet_pricing`

use arka::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "ritual")]
    {
        use arka::compute::infernet::InfernetClient;

        let endpoint = std::env::var("INFERNET_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:8000".into());

        let client = InfernetClient::new(&endpoint);

        // List available containers
        match client.list_containers().await {
            Ok(containers) => {
                println!("Available Infernet containers:");
                for (name, info) in &containers {
                    println!("  - {}: {:?}", name, info);
                }
            }
            Err(e) => println!("Could not list containers (infernet may not be running): {e}"),
        }

        // Request pricing signal
        let payload = serde_json::json!({
            "input": "Get the current USDC/ETH Meridian pricing signal for Arbitrum",
            "model": "meridian-pricing-v1",
        });

        println!("\nRequesting Infernet inference...");
        match client.request_inference("meridian-pricing", payload).await {
            Ok(result) => {
                println!("Inference result:");
                println!("  Container: {}", result.container_id);
                println!("  Output: {}", result.output);
                println!("  Node: {}", result.node_id);
                println!("  Timestamp: {}", result.timestamp);
                if result.proof.is_some() {
                    println!("  Proof: available ({} bytes)", result.proof.as_ref().unwrap().len());
                }
            }
            Err(e) => println!("Inference failed (expected without running Infernet): {e}"),
        }
    }

    #[cfg(not(feature = "ritual"))]
    println!("ritual feature not enabled. Run with --features ritual");

    Ok(())
}
