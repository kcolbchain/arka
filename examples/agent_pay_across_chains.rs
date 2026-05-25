//! Cross-chain payment example — same agent paying USDC on Base and on Solana.
//!
//! Run with: `cargo run --example agent_pay_across_chains`
//!
//! Requires `solana` feature: `cargo run --features solana --example agent_pay_across_chains`

use arka::prelude::*;
use arka::wallet::{EvmWallet, Wallet};

#[cfg(feature = "solana")]
use arka::wallet::solana::SolanaWallet;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // EVM wallet for Base
    let evm_wallet = EvmWallet::generate()?;
    println!("EVM wallet: {} (family: {})", evm_wallet.pubkey(), evm_wallet.chain_family());

    // EVM agent on Base
    let evm_agent = Agent::builder()
        .chain(Chain::Base)
        .wallet(Box::new(evm_wallet))
        .build()
        .await?;
    println!("Base agent ready at {}", evm_agent.address());

    #[cfg(feature = "solana")]
    {
        // Solana wallet
        let sol_wallet = SolanaWallet::generate();
        println!(
            "Solana wallet: {} (family: {})",
            sol_wallet.pubkey(),
            sol_wallet.chain_family()
        );

        // Demonstrate signing on both chains
        let msg = b"cross-chain payment intent";
        let evm_sig = evm_agent.wallet().sign_message(msg).await?;
        let sol_sig = sol_wallet.sign_message(msg).await?;

        println!("EVM signature ({} bytes): 0x{}", evm_sig.len(), hex::encode(&evm_sig[..8]));
        println!("Solana signature ({} bytes): {}", sol_sig.len(), hex::encode(&sol_sig[..8]));
        println!("Cross-chain payment flow ready for USDC on Base and Solana.");
    }

    #[cfg(not(feature = "solana"))]
    {
        println!("Solana feature not enabled. Run with --features solana to see Solana wallet.");
    }

    Ok(())
}
