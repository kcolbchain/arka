//! Basic agent example — create an agent, check balance, read block number.

use arka::prelude::*;
use arka::wallet::EvmWallet;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Generate a fresh EVM wallet
    let wallet = EvmWallet::generate()?;
    println!("Agent wallet: {:?}", wallet.address());

    // Create agent on Base
    let agent = Agent::builder()
        .chain(Chain::Base)
        .wallet(Box::new(wallet))
        .build()
        .await?;

    // Read chain state
    let block = agent.block_number().await?;
    println!("Current block on {}: {}", agent.chain(), block);

    let balance = agent.balance().await?;
    println!("Balance: {} wei", balance);

    println!("\nAgent ready on {} at {}", agent.chain(), agent.address());

    Ok(())
}
