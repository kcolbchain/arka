//! Chain-specific primitives (addresses, known contracts, helpers).
//!
//! The top-level `crate::chain` module handles the generic `Chain` enum
//! and RPC connector. This `chains` module contains per-network modules
//! with constants and typed clients for well-known contracts on that
//! network — starting with Arbitrum, the home chain for the agent-economy
//! deployments arka is designed to support.

pub mod arbitrum;
