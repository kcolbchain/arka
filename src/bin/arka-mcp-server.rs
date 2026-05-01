use arka::agent::account::InMemoryAgentAccount;
use arka::chain::{Chain, ChainConnector};
use arka::dex::DexModule;
use arka::mcp::{run_stdio, AgentDepositTool, BalanceTool, McpServer, SwapQuoteTool};
use arka::{ArkaError, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let chain = chain_from_env()?;
    let connector = match std::env::var("ARKA_RPC_URL") {
        Ok(url) => ChainConnector::with_rpc(chain, &url).await?,
        Err(_) => ChainConnector::new(chain).await?,
    };

    let server = McpServer::new()
        .with_tool(BalanceTool::new(connector))
        .with_tool(SwapQuoteTool::new(DexModule::new(chain)))
        .with_tool(AgentDepositTool::new(
            InMemoryAgentAccount::with_default_address(),
        ));

    run_stdio(server).await
}

fn chain_from_env() -> Result<Chain> {
    let raw = std::env::var("ARKA_CHAIN").unwrap_or_else(|_| "arbitrum".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "ethereum" | "eth" => Ok(Chain::Ethereum),
        "arbitrum" | "arb" => Ok(Chain::Arbitrum),
        "optimism" | "op" => Ok(Chain::Optimism),
        "base" => Ok(Chain::Base),
        "avalanche" | "avax" => Ok(Chain::Avalanche),
        "polygon" | "matic" => Ok(Chain::Polygon),
        "bsc" | "bnb" => Ok(Chain::Bsc),
        "tempo" => Ok(Chain::Tempo),
        "tempo-testnet" | "tempo_testnet" => Ok(Chain::TempoTestnet),
        other => Err(ArkaError::Config(format!(
            "unsupported ARKA_CHAIN: {other}"
        ))),
    }
}
