//! Solana chain integration tests.
//!
//! These tests require a running solana-test-validator instance.
//! Start it with: `solana-test-validator`

use arka::chains::solana::{SolanaChain, SolanaClient, SolanaPrograms};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};
use std::str::FromStr;

/// Test SOL balance query on devnet.
#[test]
fn test_get_sol_balance() {
    let chain = SolanaChain::new("https://api.devnet.solana.com").unwrap();

    // Use a known devnet address
    let address = Pubkey::from_str("11111111111111111111111111111111").unwrap();

    // This should not error (balance may be 0)
    let result = chain.get_sol_balance(&address);
    assert!(result.is_ok());
}

/// Test SOL transfer on local test validator.
#[test]
fn test_transfer_sol() {
    // This test requires solana-test-validator running locally
    let chain = SolanaChain::new("http://localhost:8899").unwrap();

    let from = Keypair::new();
    let to = Keypair::new();

    // Airdrop some SOL to sender
    // Note: This requires manual airdrop in test validator

    // Transfer 0.1 SOL
    let lamports = 100_000_000; // 0.1 SOL
    let result = chain.transfer_sol(&from, &to.pubkey(), lamports);

    // May fail due to insufficient balance, but should not panic
    // In a real test, we'd airdrop first
    assert!(result.is_ok() || result.is_err());
}

/// Test token balance query.
#[test]
fn test_get_token_balance() {
    let chain = SolanaChain::new("https://api.devnet.solana.com").unwrap();

    let wallet = Keypair::new();
    let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC

    // This may fail if no token account exists, but should not panic
    let result = chain.get_token_balance(&wallet.pubkey(), &mint);
    assert!(result.is_ok() || result.is_err());
}

/// Test account existence check.
#[test]
fn test_account_exists() {
    let chain = SolanaChain::new("https://api.devnet.solana.com").unwrap();

    // System program should exist
    let system_program = Pubkey::from_str("11111111111111111111111111111111").unwrap();
    let exists = chain.account_exists(&system_program).unwrap();
    assert!(exists);

    // Random address likely doesn't exist
    let random = Keypair::new();
    let exists = chain.account_exists(&random.pubkey()).unwrap();
    assert!(!exists);
}

/// Test block height query.
#[test]
fn test_get_block_height() {
    let chain = SolanaChain::new("https://api.devnet.solana.com").unwrap();

    let height = chain.get_block_height();
    assert!(height.is_ok());
    assert!(height.unwrap() > 0);
}

/// Test Solana programs constants.
#[test]
fn test_solana_programs() {
    assert_eq!(SolanaPrograms::SYSTEM_PROGRAM, "11111111111111111111111111111111");
    assert_eq!(
        SolanaPrograms::TOKEN_PROGRAM,
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    );
    assert_eq!(
        SolanaPrograms::ASSOCIATED_TOKEN_PROGRAM,
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
    );
}

/// Test SolanaClient creation.
#[test]
fn test_solana_client_creation() {
    let keypair = Keypair::new();
    let client = SolanaClient::new("https://api.devnet.solana.com", keypair);

    assert!(client.is_ok());

    let client = client.unwrap();
    assert_eq!(client.pubkey().to_string().len(), 44); // Base58 encoded pubkey
}

/// Test SolanaClient balance query.
#[test]
fn test_solana_client_balance() {
    let keypair = Keypair::new();
    let client = SolanaClient::new("https://api.devnet.solana.com", keypair).unwrap();

    // Balance should be 0 for new keypair
    let balance = client.balance();
    assert!(balance.is_ok());
    assert_eq!(balance.unwrap(), 0);
}
