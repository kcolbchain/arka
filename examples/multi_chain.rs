//! Multi-chain example — same wallet across Base, Arbitrum, Optimism, and Solana.

use arka::prelude::*;
use arka::chain::solana;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = Wallet::generate()?;
    println!("Wallet: {:?}\n", wallet.address());

    // EVM chains
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

    // Solana chain
    println!("\n--- Solana ---");
    let solana_conn = solana::constants::mainnet()?;
    println!("RPC: {}", solana_conn.rpc_url());

    let sol_address = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTspzyhU6MrLuAFbaf";
    match solana_conn.get_balance(sol_address).await {
        Ok(lamports) => {
            let sol = lamports as f64 / 1_000_000_000.0;
            println!("SOL balance: {:.9} SOL ({} lamports)", sol, lamports);
        }
        Err(e) => {
            println!("Failed to get SOL balance: {}", e);
        }
    }

    let slot = solana_conn.get_slot().await?;
    println!("Slot: {}", slot);

    Ok(())
}
