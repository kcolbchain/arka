//! Multi-chain example — same wallet across EVM chains + Solana.
//!
//! Run with:
//!   cargo run --example multi_chain
//!   cargo run --example multi_chain --features solana

use arka::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = Wallet::generate()?;
    println!("EVM Wallet: {:?}\n", wallet.address());

    let evm_chains = [Chain::Base, Chain::Arbitrum, Chain::Optimism, Chain::Tempo];

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

    #[cfg(feature = "solana")]
    {
        use arka::chains::solana::{SolanaConnector, SolanaWallet};
        println!("\n--- Solana ---");
        let sol_wallet = SolanaWallet::generate();
        println!("Solana Wallet: {}", sol_wallet.address());

        let connector = SolanaConnector::new("https://api.devnet.solana.com")
            .expect("Solana connector");
        let block = connector.block_height().unwrap_or(0);
        println!(
            "{:12} | block: {:>10} | balance: N/A (RPC required) | gas: native",
            Chain::SolanaDevnet,
            block,
        );
    }

    Ok(())
}
