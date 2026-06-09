//! Multi-chain example — same wallet across Base, Arbitrum, Optimism, and Solana.
//!
//! Demonstrates arka's chain-agnostic design by querying block height and
//! balance across EVM chains AND Solana in a single run.

use arka::prelude::*;
use arka::chain::solana_connector::SolanaConnector;
use arka::chains::solana::{SolanaConstants, SplTokenClient};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = Wallet::generate()?;
    println!("EVM Wallet: {:?}\n", wallet.address());

    // ── EVM Chains ──────────────────────────────────────────────
    let evm_chains = [Chain::Base, Chain::Arbitrum, Chain::Optimism, Chain::Tempo];

    println!("=== EVM Chains ===");
    for chain in evm_chains {
        let agent = Agent::builder()
            .chain(chain)
            .wallet(wallet.clone())
            .build()
            .await?;

        let block = agent.block_number().await?;
        let balance = agent.balance().await?;

        println!(
            "{:12} | block: {:>10} | balance: {} wei | gas: {}",
            chain,
            block,
            balance,
            if chain.stablecoin_gas() {
                "stablecoin"
            } else {
                "native"
            }
        );
    }

    // ── Solana ──────────────────────────────────────────────────
    println!("\n=== Solana ===");

    // Use devnet for testing (switch to SolanaConnector::mainnet() for production)
    match SolanaConnector::devnet() {
        Ok(sol) => {
            let slot = sol.slot().unwrap_or(0);
            println!(
                "{:12} | slot: {:>10} | native: SOL",
                "solana-devnet", slot
            );

            // Example: check USDC token info
            let usdc = SplTokenClient::usdc(&sol);
            println!(
                "{:12} | SPL: USDC | mint: {} | decimals: {}",
                "solana-devnet",
                &SolanaConstants::USDC[..8],
                usdc.decimals()
            );
        }
        Err(e) => {
            println!("solana-devnet | connection failed: {e}");
        }
    }

    println!("\nDone! All chains queried.");
    Ok(())
}
