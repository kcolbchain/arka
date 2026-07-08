use super::*;
use crate::chain::Chain;
use crate::wallet::switchboard::SwitchboardWallet;

#[tokio::test]
async fn test_builder_missing_chain() {
    let result = CR8ClientBuilder::new()
        .with_wallet(SwitchboardWallet::generate().unwrap())
        .with_contract_address("0x123")
        .build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_success() {
    let wallet = SwitchboardWallet::generate().unwrap();
    let client = CR8ClientBuilder::new()
        .with_chain(Chain::ArbitrumSepolia)
        .with_wallet(wallet)
        .with_contract_address("0x123")
        .build()
        .unwrap();
    assert_eq!(client.contract_address, "0x123");
}
