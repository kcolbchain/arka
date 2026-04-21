//! Example: agent-registry deposit / execute-task flow, first against an
//! in-memory account (runs anywhere, no RPC needed), then building the
//! calldata an agent would submit to an Arbitrum AgentAccount-shaped
//! registry contract.
//!
//! Run with: `cargo run --example agent_account`

use arka::prelude::*;
use arka::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // ---- In-memory flow ----
    let registry = InMemoryAgentAccount::with_default_address();
    let agent: Address = "0x00000000000000000000000000000000000A9E41"
        .parse()
        .unwrap();

    // Seed the agent with 1.00 USDC (6 decimals).
    registry.deposit_for(agent, U256::from(1_000_000u64));
    let bal = AgentAccount::balance(&registry, agent).await?;
    println!("in-memory balance after deposit = {bal}");

    // Execute a paid task costing 0.10 USDC.
    let task_id = alloy::primitives::FixedBytes::from([0xABu8; 32]);
    let receipt = AgentAccount::execute_task(
        &registry,
        agent,
        task_id,
        U256::from(100_000u64),
        alloy::primitives::Bytes::from(b"hello".to_vec()),
    )
    .await?;
    println!(
        "task executed: success={} fee={} remaining={}",
        receipt.success,
        receipt.fee,
        AgentAccount::balance(&registry, agent).await?
    );

    // ---- On-chain calldata build (Arbitrum) ----
    // Replace with the real deployed AgentAccount contract address.
    let contract: Address = "0x0000000000000000000000000000000000000001"
        .parse()
        .unwrap();
    let client = AgentDepositClient::with_usdc(contract)?;
    let calldata = client.encode_deposit(U256::from(1_000_000u64));
    println!(
        "arbitrum deposit calldata ({} bytes, selector={:02x?}) ready to submit to {:?}",
        calldata.len(),
        &calldata[..4],
        client.contract()
    );
    println!(
        "settlement token (native Arbitrum USDC) = {:?}",
        client.settlement_token()
    );

    Ok(())
}
