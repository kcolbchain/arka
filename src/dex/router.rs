//! Uniswap V3 SwapRouter02 integration.
//!
//! Builds calldata for exactInputSingle swaps. Can be used directly
//! or through the MeridianVault for agent-managed positions.

use alloy::primitives::{Address, Bytes, Uint, U256};
use alloy::sol;
use alloy::sol_types::SolCall;

type U160 = Uint<160, 3>;

use crate::chain::Chain;
use crate::error::{ArkaError, Result};

use super::types::FeeTier;

/// Uniswap V3 SwapRouter02 addresses per chain.
fn router_address(chain: Chain) -> Result<Address> {
    let addr = match chain {
        Chain::Ethereum => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
        Chain::Arbitrum => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
        Chain::Optimism => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
        Chain::Base => "0x2626664c2603336E57B271c5C0b26F421741e481",
        Chain::Polygon => "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
        _ => return Err(ArkaError::Dex(format!("No Uniswap V3 router on {chain}"))),
    };
    addr.parse()
        .map_err(|e| ArkaError::Dex(format!("Invalid router address: {e}")))
}

// Generate Rust bindings for the SwapRouter02 exactInputSingle function.
sol! {
    #[derive(Debug)]
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    #[derive(Debug)]
    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        returns (uint256 amountOut);
}

/// Uniswap V3 router interface for building swap transactions.
pub struct UniswapV3Router {
    chain: Chain,
    router: Result<Address>,
}

impl UniswapV3Router {
    pub fn new(chain: Chain) -> Self {
        Self {
            chain,
            router: router_address(chain),
        }
    }

    /// Get the chain this router targets.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Get the router address for this chain.
    pub fn address(&self) -> Result<Address> {
        match &self.router {
            Ok(addr) => Ok(*addr),
            Err(e) => Err(ArkaError::Dex(format!("{e}"))),
        }
    }

    /// Check if this chain has a Uniswap V3 deployment.
    pub fn is_supported(&self) -> bool {
        self.router.is_ok()
    }

    /// Encode calldata for an exactInputSingle swap.
    pub fn encode_swap(
        &self,
        token_in: Address,
        token_out: Address,
        fee_tier: FeeTier,
        recipient: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<Bytes> {
        let params = ExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: Uint::<24, 1>::from(fee_tier.as_u24()),
            recipient,
            amountIn: amount_in,
            amountOutMinimum: min_amount_out,
            sqrtPriceLimitX96: U160::ZERO,
        };

        let call = exactInputSingleCall { params };
        Ok(Bytes::from(call.abi_encode()))
    }

    /// Compute minimum output amount given slippage tolerance.
    pub fn min_output(amount_out_expected: U256, slippage_bps: u16) -> U256 {
        let slippage = amount_out_expected * U256::from(slippage_bps) / U256::from(10000u64);
        amount_out_expected - slippage
    }
}
