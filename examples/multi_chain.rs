//! Multi-chain example — same wallet across Base, Arbitrum, and Optimism.

use arka::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = Wallet::generate()?;
    println!("Wallet: {:?}\n", wallet.address());

    let chains = [Chain::Base, Chain::Arbitrum, Chain::Optimism, Chain::Tempo];

    for chain in chains {
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

    Ok(())
}
