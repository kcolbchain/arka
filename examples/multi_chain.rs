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

    // Solana Example
    let solana = arka::chain::solana::SolanaChain::new()?;
    let solana_wallet = solana_sdk::signature::Keypair::new();
    let balance = solana.balance(&solana_wallet.pubkey()).await.unwrap_or(0);
    println!("solana       | balance: {} lamports | gas: native", balance);

    Ok(())
}
