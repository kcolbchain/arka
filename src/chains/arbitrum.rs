//! Arbitrum One primitives — well-known contract addresses, agent-registry
//! client helpers, and USDC settlement wiring.
//!
//! Arbitrum is the home chain for the agent-economy deployments arka targets:
//! low-cost settlement, mature DeFi liquidity (Uniswap V3, Camelot), native
//! USDC, and fast finality — the right substrate for registered autonomous
//! agents that deposit funds, earn fees, and run tasks.
//!
//! ## What this module provides
//! - `ArbitrumContracts` — static addresses: USDC, USDT, WETH, Uniswap V3 router.
//! - `AgentDepositClient` — typed client for an ERC-20-denominated agent
//!   deposit / balance / withdrawal contract. The ABI matches a minimal
//!   `AgentAccount`-shaped registry: `deposit(uint256)`, `balanceOf(address)`,
//!   `withdraw(uint256)`, `executeTask(bytes32,bytes)`.
//!
//! The contract address is **configurable** (not hard-coded) because the
//! registry address will change between testnet deployments and production
//! Arbitrum One. Agents supply the address explicitly.

use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::sol;
use alloy::sol_types::SolCall;

use crate::chain::Chain;
use crate::error::{ArkaError, Result};

/// Arbitrum One chain ID.
pub const ARBITRUM_ONE_CHAIN_ID: u64 = 42161;

/// Arbitrum Sepolia (testnet) chain ID.
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;

/// Well-known contract addresses on Arbitrum One.
///
/// These are stable across the lifetime of the network. Protocol-specific
/// deployment addresses (like an agent registry) are NOT listed here —
/// they belong to a specific deployment and should be passed by the caller.
pub struct ArbitrumContracts;

impl ArbitrumContracts {
    /// Native USDC on Arbitrum One (Circle, not bridged USDC.e).
    pub const USDC: &'static str = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831";

    /// Bridged USDC.e on Arbitrum One (legacy, kept for interop).
    pub const USDC_E: &'static str = "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8";

    /// Tether USD on Arbitrum One.
    pub const USDT: &'static str = "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9";

    /// Wrapped Ether on Arbitrum One.
    pub const WETH: &'static str = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1";

    /// Uniswap V3 SwapRouter02 on Arbitrum One.
    pub const UNISWAP_V3_ROUTER: &'static str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";

    /// Uniswap V3 QuoterV2 on Arbitrum One.
    pub const UNISWAP_V3_QUOTER: &'static str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
}

// Solidity bindings for a minimal AgentAccount-shaped registry. The shape
// intentionally matches common agent-deposit contracts (CR8 / Create Protocol
// AgentDeposit, ERC-4337 accounts, and x402-style escrows): an ERC-20-backed
// account that exposes deposit, balance, withdraw, and executeTask.
sol! {
    #[derive(Debug)]
    interface IAgentAccount {
        function deposit(uint256 amount) external;
        function balanceOf(address agent) external view returns (uint256);
        function withdraw(uint256 amount) external;
        function executeTask(bytes32 taskId, bytes calldata payload) external returns (bool);
    }
}

/// Typed client for an Arbitrum-deployed AgentAccount-shaped contract.
///
/// This does NOT submit transactions on its own — it only builds the
/// calldata and holds the target address. Callers feed the calldata into
/// their signing / broadcast pipeline (e.g. `crate::tx::TxRequest`). This
/// separation keeps the client trivially testable without an RPC.
#[derive(Debug, Clone)]
pub struct AgentDepositClient {
    /// Address of the deployed AgentAccount-shaped registry contract.
    contract: Address,
    /// Settlement token address (e.g. Arbitrum USDC).
    settlement_token: Address,
}

impl AgentDepositClient {
    /// Create a client bound to a specific deployed contract and
    /// settlement token (typically USDC on Arbitrum One).
    pub fn new(contract: Address, settlement_token: Address) -> Self {
        Self {
            contract,
            settlement_token,
        }
    }

    /// Shortcut: bind to a contract using native Arbitrum USDC.
    pub fn with_usdc(contract: Address) -> Result<Self> {
        let usdc: Address = ArbitrumContracts::USDC.parse().map_err(|e| {
            ArkaError::Config(format!("Failed to parse Arbitrum USDC address: {e}"))
        })?;
        Ok(Self::new(contract, usdc))
    }

    /// Contract address.
    pub fn contract(&self) -> Address {
        self.contract
    }

    /// Settlement token (ERC-20) address.
    pub fn settlement_token(&self) -> Address {
        self.settlement_token
    }

    /// Chain this client targets.
    pub fn chain(&self) -> Chain {
        Chain::Arbitrum
    }

    /// Encode calldata for `deposit(amount)`.
    pub fn encode_deposit(&self, amount: U256) -> Bytes {
        let call = IAgentAccount::depositCall { amount };
        Bytes::from(call.abi_encode())
    }

    /// Encode calldata for `balanceOf(agent)`.
    pub fn encode_balance_of(&self, agent: Address) -> Bytes {
        let call = IAgentAccount::balanceOfCall { agent };
        Bytes::from(call.abi_encode())
    }

    /// Encode calldata for `withdraw(amount)`.
    pub fn encode_withdraw(&self, amount: U256) -> Bytes {
        let call = IAgentAccount::withdrawCall { amount };
        Bytes::from(call.abi_encode())
    }

    /// Encode calldata for `executeTask(taskId, payload)`.
    pub fn encode_execute_task(&self, task_id: FixedBytes<32>, payload: Bytes) -> Bytes {
        let call = IAgentAccount::executeTaskCall {
            taskId: task_id,
            payload,
        };
        Bytes::from(call.abi_encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> Address {
        "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap()
    }

    #[test]
    fn usdc_address_parses() {
        let usdc: Address = ArbitrumContracts::USDC.parse().expect("valid address");
        // Native Arbitrum USDC starts with 0xaf88
        assert_eq!(
            format!("{usdc:?}").to_lowercase(),
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831"
        );
    }

    #[test]
    fn chain_id_matches() {
        assert_eq!(Chain::Arbitrum.chain_id(), ARBITRUM_ONE_CHAIN_ID);
    }

    #[test]
    fn client_binds_usdc_by_default() {
        let client = AgentDepositClient::with_usdc(sample_contract()).unwrap();
        let usdc: Address = ArbitrumContracts::USDC.parse().unwrap();
        assert_eq!(client.settlement_token(), usdc);
        assert_eq!(client.contract(), sample_contract());
        assert_eq!(client.chain(), Chain::Arbitrum);
    }

    #[test]
    fn encode_deposit_selector_is_correct() {
        let client = AgentDepositClient::with_usdc(sample_contract()).unwrap();
        let calldata = client.encode_deposit(U256::from(1_000_000u64));
        // deposit(uint256) selector = first 4 bytes of keccak256("deposit(uint256)") = 0xb6b55f25
        assert_eq!(&calldata[..4], &[0xb6, 0xb5, 0x5f, 0x25]);
        // Full calldata = 4 (selector) + 32 (amount) = 36 bytes
        assert_eq!(calldata.len(), 36);
    }

    #[test]
    fn encode_balance_of_selector_is_correct() {
        let client = AgentDepositClient::with_usdc(sample_contract()).unwrap();
        let agent: Address = "0x2222222222222222222222222222222222222222"
            .parse()
            .unwrap();
        let calldata = client.encode_balance_of(agent);
        // balanceOf(address) selector = 0x70a08231
        assert_eq!(&calldata[..4], &[0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(calldata.len(), 36);
    }

    #[test]
    fn encode_withdraw_roundtrips_amount() {
        let client = AgentDepositClient::with_usdc(sample_contract()).unwrap();
        let amount = U256::from(42u64);
        let calldata = client.encode_withdraw(amount);
        // The amount must appear in the last 32 bytes as big-endian uint256.
        let last_byte = calldata[calldata.len() - 1];
        assert_eq!(last_byte, 42);
    }

    #[test]
    fn encode_execute_task_contains_task_id() {
        let client = AgentDepositClient::with_usdc(sample_contract()).unwrap();
        let mut task_id = [0u8; 32];
        task_id[31] = 0x7f;
        let calldata =
            client.encode_execute_task(FixedBytes::from(task_id), Bytes::from(vec![0xde, 0xad]));
        // Selector (4) + taskId (32) + offset (32) + length (32) + payload (32 padded) = 132
        assert_eq!(calldata.len(), 132);
        // taskId sits right after the 4-byte selector
        assert_eq!(calldata[4 + 31], 0x7f);
    }

    #[test]
    fn custom_settlement_token_is_honored() {
        let usdt: Address = ArbitrumContracts::USDT.parse().unwrap();
        let client = AgentDepositClient::new(sample_contract(), usdt);
        assert_eq!(client.settlement_token(), usdt);
    }
}
