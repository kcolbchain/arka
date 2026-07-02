use super::*;
use crate::chain::Chain;
use crate::wallet::Wallet;

#[tokio::test]
async fn test_builder_missing_chain() {
    let result = CR8ClientBuilder::new()
        .with_wallet(Wallet::generate().unwrap())
        .with_contract_address("0x123")
        .build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_success() {
    let wallet = Wallet::generate().unwrap();
    let client = CR8ClientBuilder::new()
        .with_chain(Chain::Arbitrum)
        .with_wallet(wallet)
        .with_contract_address("0x123")
        .build()
        .unwrap();
    assert_eq!(client.contract_address, "0x123");
}
