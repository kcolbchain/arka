//! MCP server support for exposing arka primitives as tools.
//!
//! The server intentionally implements the small JSON-RPC surface arka needs:
//! `initialize`, `tools/list`, and `tools/call` over stdio. Tool wrappers are
//! provider-backed so unit tests can exercise them without live RPC calls.

use std::collections::BTreeMap;
use std::sync::Arc;

use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::agent::account::AgentAccount;
use crate::chain::{Chain, ChainConnector};
use crate::dex::{DexModule, FeeTier};
use crate::error::{ArkaError, Result};

/// A single arka primitive exposed as an MCP tool.
#[async_trait]
pub trait ArkaTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;
    async fn invoke(&self, args: Value) -> Result<Value>;
}

/// Minimal MCP JSON-RPC server.
#[derive(Default)]
pub struct McpServer {
    tools: BTreeMap<String, Arc<dyn ArkaTool>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tool<T>(mut self, tool: T) -> Self
    where
        T: ArkaTool + 'static,
    {
        self.add_tool(tool);
        self
    }

    pub fn add_tool<T>(&mut self, tool: T)
    where
        T: ArkaTool + 'static,
    {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    pub async fn handle_request(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return id.map(|id| jsonrpc_error(id, -32600, "missing JSON-RPC method"));
        };

        let Some(id) = id else {
            return None;
        };

        let result = match method {
            "initialize" => self.initialize_result(&request),
            "tools/list" => Ok(self.tools_list_result()),
            "tools/call" => self.tools_call_result(&request).await,
            _ => Err(jsonrpc_error(id.clone(), -32601, "method not found")),
        };

        Some(match result {
            Ok(result) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            }),
            Err(error) => error,
        })
    }

    fn initialize_result(&self, request: &Value) -> std::result::Result<Value, Value> {
        let protocol_version = request
            .get("params")
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str)
            .unwrap_or("2024-11-05");

        Ok(json!({
            "protocolVersion": protocol_version,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "arka-mcp-server",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn tools_list_result(&self) -> Value {
        let tools = self
            .tools
            .values()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "inputSchema": tool.schema(),
                })
            })
            .collect::<Vec<_>>();

        json!({ "tools": tools })
    }

    async fn tools_call_result(&self, request: &Value) -> std::result::Result<Value, Value> {
        let params = request.get("params").unwrap_or(&Value::Null);
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| jsonrpc_error(request_id(request), -32602, "missing tool name"))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| jsonrpc_error(request_id(request), -32602, "unknown tool"))?;

        match tool.invoke(args).await {
            Ok(result) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
                }],
                "structuredContent": result,
                "isError": false
            })),
            Err(err) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": err.to_string()
                }],
                "isError": true
            })),
        }
    }
}

/// Serve MCP requests from stdin and write newline-delimited JSON-RPC responses
/// to stdout.
pub async fn run_stdio(server: McpServer) -> Result<()> {
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| ArkaError::Config(format!("failed to read MCP request: {err}")))?
    {
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => server.handle_request(request).await,
            Err(err) => Some(jsonrpc_error(
                Value::Null,
                -32700,
                &format!("parse error: {err}"),
            )),
        };

        if let Some(response) = response {
            let encoded = serde_json::to_vec(&response)?;
            stdout
                .write_all(&encoded)
                .await
                .map_err(|err| ArkaError::Config(format!("failed to write MCP response: {err}")))?;
            stdout
                .write_all(b"\n")
                .await
                .map_err(|err| ArkaError::Config(format!("failed to write MCP response: {err}")))?;
            stdout
                .flush()
                .await
                .map_err(|err| ArkaError::Config(format!("failed to flush MCP response: {err}")))?;
        }
    }

    Ok(())
}

#[async_trait]
pub trait BalanceProvider: Send + Sync {
    fn chain(&self) -> Chain;
    async fn balance(&self, address: Address) -> Result<U256>;
}

#[async_trait]
impl BalanceProvider for ChainConnector {
    fn chain(&self) -> Chain {
        self.chain()
    }

    async fn balance(&self, address: Address) -> Result<U256> {
        ChainConnector::balance(self, address).await
    }
}

pub struct BalanceTool<P> {
    provider: Arc<P>,
    default_address: Option<Address>,
}

impl<P> BalanceTool<P>
where
    P: BalanceProvider,
{
    pub fn new(provider: P) -> Self {
        Self {
            provider: Arc::new(provider),
            default_address: None,
        }
    }

    pub fn from_shared(provider: Arc<P>) -> Self {
        Self {
            provider,
            default_address: None,
        }
    }

    pub fn with_default_address(mut self, address: Address) -> Self {
        self.default_address = Some(address);
        self
    }
}

#[async_trait]
impl<P> ArkaTool for BalanceTool<P>
where
    P: BalanceProvider + 'static,
{
    fn name(&self) -> &'static str {
        "balance"
    }

    fn description(&self) -> &'static str {
        "Read the native-token balance for an address on the configured chain"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "EVM address to check. Optional only when the server was configured with a default address."
                }
            }
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value> {
        let address = match optional_str(&args, "address") {
            Some(raw) => parse_address(raw, "address")?,
            None => self.default_address.ok_or_else(|| {
                ArkaError::Config("balance.address is required without a default address".into())
            })?,
        };
        let balance = self.provider.balance(address).await?;

        Ok(json!({
            "chain": self.provider.chain().to_string(),
            "address": address.to_string(),
            "balance_wei": balance.to_string()
        }))
    }
}

#[async_trait]
pub trait SwapQuoteProvider: Send + Sync {
    async fn quote(&self, request: SwapQuoteRequest) -> Result<Value>;
}

pub struct SwapQuoteRequest {
    token_in: String,
    token_out: String,
    amount_in: U256,
    expected_amount_out: Option<U256>,
    slippage_bps: u16,
    fee_tier: FeeTier,
}

#[async_trait]
impl SwapQuoteProvider for DexModule {
    async fn quote(&self, request: SwapQuoteRequest) -> Result<Value> {
        let params = self
            .swap(&request.token_in, &request.token_out, request.amount_in)
            .slippage_bps(request.slippage_bps)
            .fee_tier(request.fee_tier)
            .build();
        let router = self.router().address()?;
        let amount_out_minimum = request
            .expected_amount_out
            .map(|amount| crate::dex::UniswapV3Router::min_output(amount, request.slippage_bps));

        Ok(json!({
            "chain": params.chain.to_string(),
            "router": router.to_string(),
            "token_in": params.token_in,
            "token_out": params.token_out,
            "amount_in": params.amount_in.to_string(),
            "expected_amount_out": request.expected_amount_out.map(|amount| amount.to_string()),
            "amount_out_minimum": amount_out_minimum.map(|amount| amount.to_string()),
            "slippage_bps": params.slippage_bps,
            "fee_tier_bps": params.fee_tier.as_u24(),
            "deadline_secs": params.deadline_secs,
        }))
    }
}

pub struct SwapQuoteTool<P> {
    provider: Arc<P>,
}

impl<P> SwapQuoteTool<P>
where
    P: SwapQuoteProvider,
{
    pub fn new(provider: P) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    pub fn from_shared(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<P> ArkaTool for SwapQuoteTool<P>
where
    P: SwapQuoteProvider + 'static,
{
    fn name(&self) -> &'static str {
        "swap_quote"
    }

    fn description(&self) -> &'static str {
        "Build swap parameters and router metadata for an arka DEX swap"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "token_in": { "type": "string" },
                "token_out": { "type": "string" },
                "amount_in": { "type": "string", "description": "Base-unit integer amount" },
                "expected_amount_out": { "type": "string", "description": "Optional base-unit quote used to compute min output" },
                "slippage_bps": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 50 },
                "fee_tier_bps": { "type": "integer", "enum": [100, 500, 3000, 10000], "default": 3000 }
            },
            "required": ["token_in", "token_out", "amount_in"]
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value> {
        let slippage_bps = optional_u16(&args, "slippage_bps")?.unwrap_or(50);
        if slippage_bps > 10_000 {
            return Err(ArkaError::Config(
                "swap_quote.slippage_bps must be <= 10000".into(),
            ));
        }

        let fee_tier_bps = optional_u32(&args, "fee_tier_bps")?.unwrap_or(3000);
        let request = SwapQuoteRequest {
            token_in: required_str(&args, "token_in")?.to_string(),
            token_out: required_str(&args, "token_out")?.to_string(),
            amount_in: required_u256(&args, "amount_in")?,
            expected_amount_out: optional_u256(&args, "expected_amount_out")?,
            slippage_bps,
            fee_tier: parse_fee_tier(fee_tier_bps)?,
        };

        self.provider.quote(request).await
    }
}

pub struct AgentDepositTool<A> {
    account: Arc<A>,
}

impl<A> AgentDepositTool<A>
where
    A: AgentAccount,
{
    pub fn new(account: A) -> Self {
        Self {
            account: Arc::new(account),
        }
    }

    pub fn from_shared(account: Arc<A>) -> Self {
        Self { account }
    }
}

#[async_trait]
impl<A> ArkaTool for AgentDepositTool<A>
where
    A: AgentAccount + 'static,
{
    fn name(&self) -> &'static str {
        "agent_account"
    }

    fn description(&self) -> &'static str {
        "Deposit into an AgentAccount or read an AgentAccount balance"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["deposit", "balance"] },
                "amount": { "type": "string", "description": "Required for deposit; settlement-token base units" },
                "agent": { "type": "string", "description": "Address to check for balance; defaults to the account address" }
            },
            "required": ["action"]
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value> {
        match required_str(&args, "action")? {
            "deposit" => {
                let amount = required_u256(&args, "amount")?;
                let balance = self.account.deposit(amount).await?;
                Ok(json!({
                    "account": self.account.address().to_string(),
                    "balance": balance.to_string()
                }))
            }
            "balance" => {
                let agent = match optional_str(&args, "agent") {
                    Some(raw) => parse_address(raw, "agent")?,
                    None => self.account.address(),
                };
                let balance = self.account.balance(agent).await?;
                Ok(json!({
                    "account": self.account.address().to_string(),
                    "agent": agent.to_string(),
                    "balance": balance.to_string()
                }))
            }
            action => Err(ArkaError::Config(format!(
                "unsupported agent_account.action: {action}"
            ))),
        }
    }
}

fn request_id(request: &Value) -> Value {
    request.get("id").cloned().unwrap_or(Value::Null)
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn required_str<'a>(args: &'a Value, field: &str) -> Result<&'a str> {
    optional_str(args, field)
        .ok_or_else(|| ArkaError::Config(format!("missing required field: {field}")))
}

fn optional_str<'a>(args: &'a Value, field: &str) -> Option<&'a str> {
    args.get(field).and_then(Value::as_str)
}

fn required_u256(args: &Value, field: &str) -> Result<U256> {
    optional_u256(args, field)?
        .ok_or_else(|| ArkaError::Config(format!("missing required field: {field}")))
}

fn optional_u256(args: &Value, field: &str) -> Result<Option<U256>> {
    args.get(field)
        .map(|value| parse_u256(value, field))
        .transpose()
}

fn parse_u256(value: &Value, field: &str) -> Result<U256> {
    let raw = match value {
        Value::String(raw) => raw.clone(),
        Value::Number(raw) => raw.to_string(),
        _ => {
            return Err(ArkaError::Config(format!(
                "{field} must be a base-unit integer string"
            )))
        }
    };
    raw.parse::<U256>()
        .map_err(|err| ArkaError::Config(format!("invalid {field}: {err}")))
}

fn parse_address(raw: &str, field: &str) -> Result<Address> {
    raw.parse::<Address>()
        .map_err(|err| ArkaError::Config(format!("invalid {field}: {err}")))
}

fn optional_u16(args: &Value, field: &str) -> Result<Option<u16>> {
    optional_u64(args, field)?
        .map(|value| {
            u16::try_from(value)
                .map_err(|_| ArkaError::Config(format!("{field} is too large for u16")))
        })
        .transpose()
}

fn optional_u32(args: &Value, field: &str) -> Result<Option<u32>> {
    optional_u64(args, field)?
        .map(|value| {
            u32::try_from(value)
                .map_err(|_| ArkaError::Config(format!("{field} is too large for u32")))
        })
        .transpose()
}

fn optional_u64(args: &Value, field: &str) -> Result<Option<u64>> {
    match args.get(field) {
        Some(Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| ArkaError::Config(format!("{field} must be an unsigned integer"))),
        Some(Value::String(raw)) => raw
            .parse::<u64>()
            .map(Some)
            .map_err(|err| ArkaError::Config(format!("invalid {field}: {err}"))),
        Some(_) => Err(ArkaError::Config(format!(
            "{field} must be an unsigned integer"
        ))),
        None => Ok(None),
    }
}

fn parse_fee_tier(fee_tier_bps: u32) -> Result<FeeTier> {
    match fee_tier_bps {
        100 => Ok(FeeTier::Lowest),
        500 => Ok(FeeTier::Low),
        3000 => Ok(FeeTier::Medium),
        10000 => Ok(FeeTier::High),
        other => Err(ArkaError::Config(format!(
            "unsupported fee_tier_bps: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::account::InMemoryAgentAccount;

    struct EchoTool;

    #[async_trait]
    impl ArkaTool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn description(&self) -> &'static str {
            "Echo test tool"
        }

        fn schema(&self) -> Value {
            json!({ "type": "object" })
        }

        async fn invoke(&self, args: Value) -> Result<Value> {
            Ok(json!({ "echo": args }))
        }
    }

    struct StaticBalanceProvider {
        chain: Chain,
        balance: U256,
    }

    #[async_trait]
    impl BalanceProvider for StaticBalanceProvider {
        fn chain(&self) -> Chain {
            self.chain
        }

        async fn balance(&self, _address: Address) -> Result<U256> {
            Ok(self.balance)
        }
    }

    fn request(id: u64, method: &str, params: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })
    }

    #[tokio::test]
    async fn initializes_and_lists_tools() {
        let server = McpServer::new().with_tool(EchoTool);

        let initialize = server
            .handle_request(request(
                1,
                "initialize",
                json!({ "protocolVersion": "test-version" }),
            ))
            .await
            .expect("response");
        assert_eq!(initialize["result"]["protocolVersion"], "test-version");
        assert_eq!(
            initialize["result"]["serverInfo"]["name"],
            "arka-mcp-server"
        );

        let tools = server
            .handle_request(request(2, "tools/list", json!({})))
            .await
            .expect("response");
        assert_eq!(tools["result"]["tools"][0]["name"], "echo");
    }

    #[tokio::test]
    async fn calls_tool_and_returns_structured_content() {
        let server = McpServer::new().with_tool(EchoTool);

        let response = server
            .handle_request(request(
                1,
                "tools/call",
                json!({
                    "name": "echo",
                    "arguments": { "message": "hello" }
                }),
            ))
            .await
            .expect("response");

        assert_eq!(
            response["result"]["structuredContent"]["echo"]["message"],
            "hello"
        );
        assert_eq!(response["result"]["isError"], false);
    }

    #[tokio::test]
    async fn balance_tool_uses_provider_without_network() {
        let address: Address = "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        let tool = BalanceTool::new(StaticBalanceProvider {
            chain: Chain::Base,
            balance: U256::from(123u64),
        });

        let result = tool
            .invoke(json!({ "address": address.to_string() }))
            .await
            .unwrap();

        assert_eq!(result["chain"], "base");
        assert_eq!(result["balance_wei"], "123");
    }

    #[tokio::test]
    async fn swap_quote_tool_builds_router_metadata() {
        let tool = SwapQuoteTool::new(DexModule::new(Chain::Base));

        let result = tool
            .invoke(json!({
                "token_in": "ETH",
                "token_out": "USDC",
                "amount_in": "1000000000000000000",
                "expected_amount_out": "3000000000",
                "slippage_bps": 100,
                "fee_tier_bps": 500
            }))
            .await
            .unwrap();

        assert_eq!(result["chain"], "base");
        assert_eq!(result["fee_tier_bps"], 500);
        assert_eq!(result["amount_out_minimum"], "2970000000");
    }

    #[tokio::test]
    async fn agent_account_tool_deposits_and_reads_balance() {
        let account = InMemoryAgentAccount::with_default_address();
        let address = account.address();
        let tool = AgentDepositTool::new(account);

        let deposit = tool
            .invoke(json!({ "action": "deposit", "amount": "42" }))
            .await
            .unwrap();
        assert_eq!(deposit["balance"], "42");

        let balance = tool
            .invoke(json!({ "action": "balance", "agent": address.to_string() }))
            .await
            .unwrap();
        assert_eq!(balance["balance"], "42");
    }
}
