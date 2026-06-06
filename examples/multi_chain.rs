//! Multi-chain example — same wallet across Base, Arbitrum, Optimism, Tempo, and Solana.
//!
//! NOTE: Solana support requires the `solana` feature flag:
//! ```bash
//! cargo run --example multi_chain --features solana
//! ```

use arka::prelude::*;

#[cfg(feature = "solana")]
use arka::chains::solana::{SolanaClient, SolanaCluster};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let wallet = Wallet::generate()?;
    println!("Wallet: {:?}\n", wallet.address());

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

    // -----------------------------------------------------------------
    // Solana demo (requires `--features solana`)
    // -----------------------------------------------------------------
    #[cfg(feature = "solana")]
    {
        println!("\n--- Solana ---");
        match SolanaClient::connect(SolanaCluster::Devnet).await {
            Ok(client) => {
                println!(
                    "Connected to Solana {} at {}",
                    client.cluster(),
                    client.cluster().rpc_url()
                );

                // Generate a throwaway wallet and check its (zero) balance.
                let ephemeral = solana_sdk::signature::Keypair::new();
                match client.get_balance_sol(&ephemeral.pubkey()).await {
                    Ok(bal) => println!("Ephemeral wallet balance: {bal} SOL"),
                    Err(e) => println!("Balance check (expected on fresh key): {e}"),
                }

                // Show a well-known devnet token mint (USDC devnet).
                let usdc_mint = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPXgGpA5K2Y"
                    .parse::<solana_sdk::pubkey::Pubkey>()
                    .expect("valid devnet USDC mint");
                println!("Devnet USDC mint: {usdc_mint}");
            }
            Err(e) => println!("Solana cluster unreachable (devnet may be down): {e}"),
        }
    }

    #[cfg(not(feature = "solana"))]
    {
        println!("\n--- Solana (skipped — enable with `--features solana`) ---");
    }

    Ok(())
}
