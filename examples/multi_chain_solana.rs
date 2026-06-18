//! Multi-chain example with Solana support.
//!
//! This example demonstrates how to use Arka with both EVM chains (Arbitrum)
//! and Solana in a single application.
//!
//! ## Usage
//! ```bash
//! cargo run --example multi_chain_solana
//! ```

use arka::chains::arbitrum::{ArbitrumChain, ArbitrumContracts};
use arka::chains::solana::{SolanaChain, SolanaClient, SolanaPrograms};
use solana_sdk::signature::Keypair;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Arka Multi-Chain Example ===\n");

    // Initialize Arbitrum connection
    println!("1. Connecting to Arbitrum...");
    let arbitrum = ArbitrumChain::new("https://arb1.arbitrum.io/rpc")?;
    println!("   ✓ Connected to Arbitrum One");

    // Initialize Solana connection
    println!("\n2. Connecting to Solana...");
    let solana = SolanaChain::new("https://api.mainnet-beta.solana.com")?;
    println!("   ✓ Connected to Solana Mainnet");

    // Display program IDs
    println!("\n3. Solana Program IDs:");
    println!("   System Program: {}", SolanaPrograms::SYSTEM_PROGRAM);
    println!("   Token Program:  {}", SolanaPrograms::TOKEN_PROGRAM);

    // Display contract addresses
    println!("\n4. Arbitrum Contract Addresses:");
    println!("   USDC: {}", ArbitrumContracts::USDC);
    println!("   WETH: {}", ArbitrumContracts::WETH);

    // Create a Solana client
    println!("\n5. Creating Solana Client...");
    let keypair = Keypair::new();
    let solana_client = SolanaClient::new("https://api.devnet.solana.com", keypair)?;
    println!("   ✓ Client created");
    println!("   Wallet: {}", solana_client.pubkey());

    // Check balance
    println!("\n6. Checking Solana Balance...");
    match solana_client.balance() {
        Ok(balance) => println!("   Balance: {} lamports", balance),
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
