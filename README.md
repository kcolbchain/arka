# arka

Rust AI agent SDK for blockchain. By [kcolbchain](https://kcolbchain.com) (est. 2015).

## The Problem

AI agents need to transact on blockchains — pay for services, trade on DEXes, manage positions, settle payments. Current options:

- **Python (web3.py, LangChain)** — too slow for competitive execution, fragile in production
- **JavaScript (ethers, viem)** — not suitable for high-performance agent workloads
- **Chain-specific SDKs** — every chain has its own SDK, nothing is unified

There is no Rust SDK that lets an AI agent interact with multiple blockchains from one interface.

## What arka Does

```rust
use arka::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create an agent with a wallet
    let agent = Agent::builder()
        .chain(Chain::Base)
        .wallet(Wallet::generate()?)
        .build()
        .await?;

    // Read on-chain state
    let balance = agent.balance().await?;
    let price = agent.oracle().price("ETH/USDC").await?;

    // Execute a swap
    let tx = agent.dex()
        .swap("ETH", "USDC", parse_ether("0.1")?)
        .slippage_bps(50)
        .execute()
        .await?;

    // Pay for an API via MPP (Machine Payments Protocol)
    let response = agent.mpp()
        .pay("https://api.example.com/inference", 0.001)
        .await?;

    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────┐
│                 Agent                        │
│  (wallet, identity, state, configuration)   │
├──────────┬──────────┬───────────┬───────────┤
│  Chains  │   DEX    │   MPP     │  Oracle   │
│ (EVM,    │ (swap,   │ (HTTP 402,│ (price    │
│  Solana, │  LP,     │  sessions,│  feeds,   │
│  Cosmos) │  route)  │  receipts)│  TWAP)    │
├──────────┴──────────┴───────────┴───────────┤
│              Transport Layer                 │
│  (RPC, WebSocket, HTTP, signing)            │
└─────────────────────────────────────────────┘
```

## Features

- **Multi-chain** — EVM (Ethereum, Arbitrum, Optimism, Base, Avalanche, Tempo) + Solana from one agent. Cosmos planned.
- **Wallet management** — Generate, import, derive. Sign transactions. Manage multiple wallets.
- **DEX interaction** — Swap, add/remove liquidity, read pool state. Uniswap V3, Aerodrome, Trader Joe.
- **MPP payments** — Native support for Machine Payments Protocol. Agent pays for APIs, services, compute.
- **Oracle feeds** — Chainlink, TWAP, custom feeds. Real-world price data for agent decisions.
- **Type-safe** — Rust type system prevents common mistakes (wrong chain, wrong token, overflow).
- **Fast** — Sub-millisecond execution for competitive agent workloads (MEV, market making, solving).

## Modules

| Module | Status | Description |
|--------|--------|-------------|
| `arka::agent` | ✅ MVP | Agent builder, lifecycle, configuration |
| `arka::wallet` | ✅ MVP | Key generation, signing, multi-wallet |
| `arka::chain` | ✅ MVP | EVM + Solana chain connectors, RPC management |
| `arka::tx` | ✅ MVP | Transaction building, gas estimation, simulation |
| `arka::dex` | 🚧 WIP | DEX swap execution, routing |
| `arka::mpp` | 🚧 WIP | Machine Payments Protocol client |
| `arka::oracle` | 🚧 WIP | Price feeds, TWAP |
| `arka::solana` | ✅ MVP | Solana chain connector — SOL balance, SOL/SPL transfers |
| `arka::cosmos` | 📋 Planned | Cosmos chain connector |

## Quick Start

```bash
cargo add arka
```

Or clone and run examples:

```bash
git clone https://github.com/kcolbchain/arka.git
cd arka
cargo run --example basic_agent
```

## Examples

| Example | What it does |
|---------|-------------|
| `basic_agent` | Create agent, check balance, send transaction |
| `dex_swap` | Swap tokens on Uniswap V3 |
| `mpp_payment` | Pay for an API using MPP on Tempo |
| `multi_chain` | Same agent operating across Base + Arbitrum + Optimism |
| `switchboard_x402_client` | Pay a [switchboard](https://github.com/kcolbchain/switchboard)-served HTTP-402 endpoint. Cross-language interop demo (Rust ↔ Python). |

## MCP

arka can expose core primitives as [MCP](https://modelcontextprotocol.io/) tools over stdio:

```bash
ARKA_CHAIN=base cargo run --bin arka-mcp-server
```

Claude Desktop config example:

```json
{
  "mcpServers": {
    "arka": {
      "command": "cargo",
      "args": ["run", "--bin", "arka-mcp-server"],
      "cwd": "/path/to/arka",
      "env": {
        "ARKA_CHAIN": "base",
        "ARKA_RPC_URL": "https://mainnet.base.org"
      }
    }
  }
}
```

The server currently exposes `balance`, `swap_quote`, and `agent_account`.

## Contributing

We welcome contributions. See [CONTRIBUTING.md](CONTRIBUTING.md) and issues tagged `good-first-issue`.

## License

MIT — see [LICENSE](LICENSE)
