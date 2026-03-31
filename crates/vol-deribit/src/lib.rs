//! vol-deribit: Deribit-specific data structures and message types.
//!
//! This crate contains all Deribit WebSocket API related data models,
//! extracted from the official Deribit documentation.
//!
//! ## Key Structures
//!
//! ### Instrument Types
//! - [`DeribitInstrument`] - Full instrument metadata
//! - [`InstrumentType`] - option, future, perpetual, spot
//! - [`OptionType`] - call or put
//!
//! ### Market Data
//! - [`DeribitTicker`] - Ticker snapshot for an instrument
//! - [`MarkPrice`] - Mark price and IV updates
//! - [`OrderBook`] - Order book snapshot
//! - [`Trade`] - Trade execution data
//!
//! ### WebSocket Messages
//! - [`JsonRpcRequest`] - Outgoing JSON-RPC request
//! - [`JsonRpcResponse`] - Incoming JSON-RPC response
//! - [`SubscriptionNotification`] - Real-time subscription data
//!
//! ### Channels
//!
//! Deribit supports the following notification channels:
//!
//! | Channel Pattern | Description |
//! |-----------------|-------------|
//! | `ticker.<BASE>` | Ticker updates for all instruments of a base currency |
//! | `ticker.<BASE>.<KIND>` | Ticker for specific instrument type |
//! | `ticker.<INSTRUMENT>` | Ticker for specific instrument |
//! | `markprice.options.<INDEX>` | Options mark prices with IV |
//! | `markprice.<INDEX>` | Index mark prices |
//! | `book.<INSTRUMENT>` | Order book updates |
//! | `trades.<INSTRUMENT>` | Trade executions |

pub mod instrument;
pub mod market_data;
pub mod message;
pub mod subscription;
pub mod subscription_manager;
pub mod client;
pub mod subscription_manager;

// Re-export commonly used types
pub use instrument::*;
pub use market_data::*;
pub use message::*;
pub use subscription::*;
pub use client::{DeribitClient, ClientState};
