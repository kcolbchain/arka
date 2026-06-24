use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;

pub struct SolanaChain {
    client: RpcClient,
}

impl SolanaChain {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            client: RpcClient::new(rpc_url.to_string()),
        }
    }

    pub fn connect(&self) -> Result<()> {
        self.client.get_version()?;
        Ok(())
    }

    pub fn balance(&self, address: &str) -> Result<u64> {
        let pubkey = Pubkey::from_str(address)?;
        let balance = self.client.get_balance(&pubkey)?;
        Ok(balance)
    }

    pub fn balance_sol(&self, address: &str) -> Result<f64> {
        let lamports = self.balance(address)?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    pub fn transfer_sol(
        &self,
        from: &Keypair,
        to: &str,
        amount_sol: f64,
    ) -> Result<String> {
        let to_pubkey = Pubkey::from_str(to)?;
        let lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
        
        let instruction = system_instruction::transfer(
            &from.pubkey(),
            &to_pubkey,
            lamports,
        );
        
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );
        
        let signature = self.client.send_and_confirm_transaction(&tx)?;
        Ok(signature.to_string())
    }

    pub fn spl_token_balance(
        &self,
        token_account: &str,
    ) -> Result<u64> {
        let pubkey = Pubkey::from_str(token_account)?;
        let balance = self.client.get_token_account_balance(&pubkey)?;
        Ok(balance.amount.parse::<u64>().unwrap_or(0))
    }

    pub fn transfer_spl_token(
        &self,
        from: &Keypair,
        token_mint: &str,
        to_token_account: &str,
        amount: u64,
    ) -> Result<String> {
        let mint_pubkey = Pubkey::from_str(token_mint)?;
        let to_pubkey = Pubkey::from_str(to_token_account)?;

        let instruction = spl_token::instruction::transfer(
            &spl_token::id(),
            &from.pubkey(),
            &to_pubkey,
            &from.pubkey(),
            &[],
            amount,
        )?;

        let recent_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&from.pubkey()),
            &[from],
            recent_blockhash,
        );

        let signature = self.client.send_and_confirm_transaction(&tx)?;
        Ok(signature.to_string())
    }
}
