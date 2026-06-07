use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};

use crate::error::{ArkaError, Result};
use super::Chain;

/// Manages connection and transactions for Solana network.
pub struct SolanaChain {
    chain: Chain,
    rpc_url: String,
    client: RpcClient,
}

impl SolanaChain {
    /// Create a new connector with the default RPC.
    pub fn new() -> Result<Self> {
        Self::with_rpc(Chain::Solana.default_rpc())
    }

    /// Create a new connector with a custom RPC URL.
    pub fn with_rpc(rpc_url: &str) -> Result<Self> {
        let client = RpcClient::new(rpc_url.to_string());

        Ok(Self {
            chain: Chain::Solana,
            rpc_url: rpc_url.to_string(),
            client,
        })
    }

    /// Get native SOL balance for a pubkey (in lamports).
    pub fn balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.client
            .get_balance(pubkey)
            .map_err(|e| ArkaError::Rpc(format!("Failed to get balance: {e}")))
    }

    /// Transfer SOL from one account to another.
    pub fn transfer_sol(
        &self,
        sender: &Keypair,
        receiver: &Pubkey,
        lamports: u64,
    ) -> Result<Signature> {
        let ix = system_instruction::transfer(&sender.pubkey(), receiver, lamports);
        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::Rpc(format!("Failed to get blockhash: {e}")))?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&sender.pubkey()),
            &[sender],
            recent_blockhash,
        );

        self.client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| ArkaError::Rpc(format!("Failed to transfer SOL: {e}")))
    }

    /// Transfer SPL token.
    pub fn transfer_spl(
        &self,
        sender: &Keypair,
        source: &Pubkey,
        destination: &Pubkey,
        authority: &Keypair,
        amount: u64,
    ) -> Result<Signature> {
        let ix = spl_token::instruction::transfer(
            &spl_token::id(),
            source,
            destination,
            &authority.pubkey(),
            &[&authority.pubkey()],
            amount,
        )
        .map_err(|e| ArkaError::Rpc(format!("Failed to create SPL transfer instruction: {e}")))?;

        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| ArkaError::Rpc(format!("Failed to get blockhash: {e}")))?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&sender.pubkey()),
            &[sender, authority],
            recent_blockhash,
        );

        self.client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| ArkaError::Rpc(format!("Failed to transfer SPL token: {e}")))
    }

    /// Get the chain this connector is for.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Get the RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;
    use solana_test_validator::TestValidatorGenesis;

    #[test]
    fn test_solana_chain_sol_transfer() {
        // Start a local test validator
        let (test_validator, payer) = TestValidatorGenesis::default().start();
        let rpc_url = test_validator.rpc_url();
        
        let solana = SolanaChain::with_rpc(&rpc_url).unwrap();
        
        // Check initial balance
        let balance = solana.balance(&payer.pubkey()).unwrap();
        assert!(balance > 0);

        // Generate a new receiver
        let receiver = Keypair::new();
        
        // Transfer 1 SOL (1_000_000_000 lamports)
        let amount = 1_000_000_000;
        let _sig = solana.transfer_sol(&payer, &receiver.pubkey(), amount).unwrap();
        
        // Check receiver balance
        let receiver_balance = solana.balance(&receiver.pubkey()).unwrap();
        assert_eq!(receiver_balance, amount);
    }
}
