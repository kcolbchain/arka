//! `AgentAccount` — the agent-registry abstraction.
//!
//! An `AgentAccount` is an on-chain account owned by an agent into which
//! the agent deposits settlement funds (typically USDC), from which the
//! agent earns fees, and through which the agent executes paid tasks.
//!
//! This trait is shaped to match common agent-registry contracts:
//! - AgentDeposit-style registries (CR8 / Create Protocol).
//! - ERC-4337 smart accounts that hold a settlement balance.
//! - x402-style escrows where the agent's running balance funds per-task fees.
//!
//! Two implementations are provided:
//! - [`InMemoryAgentAccount`] — a deterministic local mock for tests and
//!   local simulations. Real.
//! - On-chain implementations live under `crate::chains::*` (e.g.
//!   `AgentDepositClient` for Arbitrum) and use this trait as a common
//!   interface so higher-level agent logic stays chain-agnostic.

use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{ArkaError, Result};

/// Receipt emitted when an agent executes a paid task through its account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReceipt {
    pub task_id: FixedBytes<32>,
    pub agent: Address,
    /// Fee charged against the agent's account balance, in settlement-token
    /// smallest units (e.g. USDC = 6 decimals).
    pub fee: U256,
    pub success: bool,
}

/// A registered agent account that holds settlement-token balance and
/// executes paid tasks.
///
/// Callers interact with this trait when they don't care whether the
/// account is on-chain or a local mock. The on-chain flavor (`AgentDepositClient`
/// on Arbitrum) wires this up to a real contract; the in-memory flavor
/// gives you a deterministic sandbox.
#[async_trait]
pub trait AgentAccount: Send + Sync {
    /// Chain-scoped address of the agent account.
    fn address(&self) -> Address;

    /// Deposit `amount` of settlement token into the agent's account.
    async fn deposit(&self, amount: U256) -> Result<U256>;

    /// Current balance for the given agent address, in settlement-token smallest units.
    async fn balance(&self, agent: Address) -> Result<U256>;

    /// Withdraw `amount` from the agent's account. Returns the new balance.
    async fn withdraw(&self, agent: Address, amount: U256) -> Result<U256>;

    /// Execute a paid task. Deducts `fee` from `agent`'s account and emits a receipt.
    async fn execute_task(
        &self,
        agent: Address,
        task_id: FixedBytes<32>,
        fee: U256,
        payload: Bytes,
    ) -> Result<TaskReceipt>;
}

/// A deterministic, in-memory implementation of [`AgentAccount`] for tests
/// and local simulations. This is not a mock in the "unreachable" sense —
/// it's a working registry that honors every invariant the trait specifies
/// and can drive real end-to-end agent flows without an RPC node.
pub struct InMemoryAgentAccount {
    address: Address,
    state: Mutex<InMemoryState>,
}

struct InMemoryState {
    balances: HashMap<Address, U256>,
    executed_tasks: Vec<TaskReceipt>,
}

impl InMemoryAgentAccount {
    /// Create a fresh in-memory account registry bound to a pseudo-address.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            state: Mutex::new(InMemoryState {
                balances: HashMap::new(),
                executed_tasks: Vec::new(),
            }),
        }
    }

    /// Create with a default address useful for tests.
    pub fn with_default_address() -> Self {
        let addr: Address = "0x000000000000000000000000000000000000a1ca"
            .parse()
            .expect("static address literal");
        Self::new(addr)
    }

    /// Deposit on behalf of a specific agent address (the trait's `deposit`
    /// doesn't take an agent — this helper lets tests seed multiple agents).
    pub fn deposit_for(&self, agent: Address, amount: U256) -> U256 {
        let mut st = self.state.lock().expect("poisoned");
        let entry = st.balances.entry(agent).or_insert(U256::ZERO);
        *entry = entry.saturating_add(amount);
        *entry
    }

    /// View all executed task receipts (for test assertions).
    pub fn executed_tasks(&self) -> Vec<TaskReceipt> {
        self.state.lock().expect("poisoned").executed_tasks.clone()
    }

    /// Number of agents with a non-zero balance.
    pub fn registered_agents(&self) -> usize {
        self.state
            .lock()
            .expect("poisoned")
            .balances
            .iter()
            .filter(|(_, v)| **v > U256::ZERO)
            .count()
    }
}

#[async_trait]
impl AgentAccount for InMemoryAgentAccount {
    fn address(&self) -> Address {
        self.address
    }

    async fn deposit(&self, amount: U256) -> Result<U256> {
        // Deposits without an explicit agent target credit the account's own
        // "self" address — matches an ERC-4337 / smart-account deposit flow
        // where msg.sender IS the agent.
        Ok(self.deposit_for(self.address, amount))
    }

    async fn balance(&self, agent: Address) -> Result<U256> {
        let st = self.state.lock().expect("poisoned");
        Ok(st.balances.get(&agent).copied().unwrap_or(U256::ZERO))
    }

    async fn withdraw(&self, agent: Address, amount: U256) -> Result<U256> {
        let mut st = self.state.lock().expect("poisoned");
        let entry = st.balances.entry(agent).or_insert(U256::ZERO);
        if *entry < amount {
            return Err(ArkaError::InsufficientBalance {
                have: entry.to_string(),
                need: amount.to_string(),
            });
        }
        *entry -= amount;
        Ok(*entry)
    }

    async fn execute_task(
        &self,
        agent: Address,
        task_id: FixedBytes<32>,
        fee: U256,
        _payload: Bytes,
    ) -> Result<TaskReceipt> {
        let mut st = self.state.lock().expect("poisoned");
        let entry = st.balances.entry(agent).or_insert(U256::ZERO);
        if *entry < fee {
            return Err(ArkaError::InsufficientBalance {
                have: entry.to_string(),
                need: fee.to_string(),
            });
        }
        *entry -= fee;
        let receipt = TaskReceipt {
            task_id,
            agent,
            fee,
            success: true,
        };
        st.executed_tasks.push(receipt.clone());
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_addr(byte: u8) -> Address {
        let mut raw = [0u8; 20];
        raw[19] = byte;
        Address::from(raw)
    }

    #[tokio::test]
    async fn deposit_and_balance_roundtrip() {
        let acct = InMemoryAgentAccount::with_default_address();
        let agent = agent_addr(1);
        acct.deposit_for(agent, U256::from(1_000_000u64));
        assert_eq!(acct.balance(agent).await.unwrap(), U256::from(1_000_000u64));
    }

    #[tokio::test]
    async fn withdraw_reduces_balance() {
        let acct = InMemoryAgentAccount::with_default_address();
        let agent = agent_addr(2);
        acct.deposit_for(agent, U256::from(500u64));
        let remaining = acct.withdraw(agent, U256::from(200u64)).await.unwrap();
        assert_eq!(remaining, U256::from(300u64));
    }

    #[tokio::test]
    async fn withdraw_rejects_overdraw() {
        let acct = InMemoryAgentAccount::with_default_address();
        let agent = agent_addr(3);
        acct.deposit_for(agent, U256::from(100u64));
        let err = acct.withdraw(agent, U256::from(200u64)).await.unwrap_err();
        matches!(err, ArkaError::InsufficientBalance { .. })
            .then_some(())
            .expect("expected InsufficientBalance");
    }

    #[tokio::test]
    async fn execute_task_charges_fee_and_emits_receipt() {
        let acct = InMemoryAgentAccount::with_default_address();
        let agent = agent_addr(4);
        acct.deposit_for(agent, U256::from(10_000u64));

        let task_id = FixedBytes::from([7u8; 32]);
        let receipt = acct
            .execute_task(agent, task_id, U256::from(250u64), Bytes::from(vec![0x01]))
            .await
            .unwrap();

        assert!(receipt.success);
        assert_eq!(receipt.fee, U256::from(250u64));
        assert_eq!(receipt.agent, agent);

        let bal = acct.balance(agent).await.unwrap();
        assert_eq!(bal, U256::from(9_750u64));

        let tasks = acct.executed_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, task_id);
    }

    #[tokio::test]
    async fn execute_task_refuses_without_balance() {
        let acct = InMemoryAgentAccount::with_default_address();
        let agent = agent_addr(5);
        // No deposit.
        let task_id = FixedBytes::from([9u8; 32]);
        let err = acct
            .execute_task(agent, task_id, U256::from(1u64), Bytes::from(vec![]))
            .await
            .unwrap_err();
        matches!(err, ArkaError::InsufficientBalance { .. })
            .then_some(())
            .expect("expected InsufficientBalance");

        // Nothing recorded.
        assert_eq!(acct.executed_tasks().len(), 0);
    }

    #[tokio::test]
    async fn registered_agents_counts_nonzero_balances() {
        let acct = InMemoryAgentAccount::with_default_address();
        acct.deposit_for(agent_addr(10), U256::from(1u64));
        acct.deposit_for(agent_addr(11), U256::from(2u64));
        acct.deposit_for(agent_addr(12), U256::ZERO);
        assert_eq!(acct.registered_agents(), 2);
    }
}
