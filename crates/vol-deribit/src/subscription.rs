//! Deribit WebSocket subscription channels.
//!
//! This module provides helpers for building Deribit subscription channel names.
//!
//! ## Channel Types
//!
//! ### Public Channels (no auth required)
//!
//! | Channel Pattern | Description |
//! |-----------------|-------------|
//! | `ticker.<BASE>` | All instruments for base currency |
//! | `ticker.<BASE>.<KIND>` | Specific instrument type |
//! | `ticker.<INSTRUMENT>` | Single instrument |
//! | `markprice.options.<INDEX>` | Options mark prices with IV |
//! | `markprice.<INDEX>` | Index mark prices |
//! | `book.<INSTRUMENT>` | Order book (10 levels) |
//! | `book.<INSTRUMENT>.<DEPTH>` | Order book with depth (1/10/20) |
//! | `trades.<INSTRUMENT>` | Trade executions |
//! | `charts.<BASE>.<INTERVAL>` | Candlestick charts |
//!
//! ### Private Channels (auth required)
//!
//! | Channel Pattern | Description |
//! |-----------------|-------------|
//! | `user.changes.<BASE>` | Position/trade updates |
//! | `user.portfolio.<BASE>` | Portfolio snapshots |
//! | `user.orders.<INSTRUMENT>` | Order updates |

/// Base currency constants
pub mod base {
    pub const BTC: &str = "BTC";
    pub const ETH: &str = "ETH";
    pub const SOL: &str = "SOL";
    pub const USDC: &str = "USDC";
    pub const USDT: &str = "USDT";
}

/// Instrument kind constants
pub mod kind {
    pub const OPTION: &str = "option";
    pub const FUTURE: &str = "future";
    pub const PERPETUAL: &str = "perpetual";
    pub const SPOT: &str = "spot";
}

/// Index name constants
pub mod index {
    pub const BTC_USD: &str = "btc_usd";
    pub const ETH_USD: &str = "eth_usd";
    pub const BTC_USDC: &str = "btc_usdc";
    pub const ETH_USDC: &str = "eth_usdc";
    pub const BTC_USDT: &str = "btc_usdt";
    pub const ETH_USDT: &str = "eth_usdt";
}

/// Build ticker channel for base currency
/// e.g., "ticker.BTC"
pub fn ticker_base(base: &str) -> String {
    format!("ticker.{}", base)
}

/// Build ticker channel for specific instrument kind
/// e.g., "ticker.BTC.perpetual"
pub fn ticker_kind(base: &str, kind: &str) -> String {
    format!("ticker.{}.{}", base, kind)
}

/// Build ticker channel for specific instrument
/// e.g., "ticker.BTC-PERPETUAL"
pub fn ticker_instrument(instrument: &str) -> String {
    format!("ticker.{}", instrument)
}

/// Build markprice options channel
/// e.g., "markprice.options.btc_usd"
pub fn markprice_options(index: &str) -> String {
    format!("markprice.options.{}", index)
}

/// Build markprice index channel
/// e.g., "markprice.btc_usd"
pub fn markprice_index(index: &str) -> String {
    format!("markprice.{}", index)
}

/// Build order book channel with default depth
/// e.g., "book.BTC-PERPETUAL"
pub fn book(instrument: &str) -> String {
    format!("book.{}", instrument)
}

/// Build order book channel with specific depth
/// e.g., "book.BTC-PERPETUAL.10"
pub fn book_depth(instrument: &str, depth: u8) -> String {
    format!("book.{}.{}", instrument, depth)
}

/// Build deribit price index channel
/// e.g., "deribit_price_index.btc_usd"
pub fn deribit_price_index(index: &str) -> String {
    format!("deribit_price_index.{}", index)
}

/// Build trades channel
/// e.g., "trades.BTC-PERPETUAL"
pub fn trades(instrument: &str) -> String {
    format!("trades.{}", instrument)
}

/// Build charts channel
/// e.g., "charts.BTC.1m"
pub fn charts(base: &str, interval: &str) -> String {
    format!("charts.{}.{}", base, interval)
}

/// Build user changes channel (requires auth)
/// e.g., "user.changes.BTC"
pub fn user_changes(base: &str) -> String {
    format!("user.changes.{}", base)
}

/// Build user portfolio channel (requires auth)
/// e.g., "user.portfolio.BTC"
pub fn user_portfolio(base: &str) -> String {
    format!("user.portfolio.{}", base)
}

/// Build user orders channel (requires auth)
/// e.g., "user.orders.BTC-PERPETUAL"
pub fn user_orders(instrument: &str) -> String {
    format!("user.orders.{}", instrument)
}

/// Common channel presets for volatility monitoring
pub mod presets {
    use super::*;

    /// Subscribe to options mark prices for BTC and ETH
    /// Returns: ["markprice.options.btc_usd", "markprice.options.eth_usd"]
    pub fn options_markprices() -> Vec<String> {
        vec![
            markprice_options(index::BTC_USD),
            markprice_options(index::ETH_USD),
        ]
    }

    /// Subscribe to all ticker data for BTC and ETH
    /// Returns: ["ticker.BTC", "ticker.ETH"]
    pub fn tickers(bases: Vec<&str>) -> Vec<String> {
        bases.iter().map(|&b| ticker_base(b)).collect()
    }

    /// Subscribe to perpetual tickers for BTC and ETH
    /// Returns: ["ticker.BTC.perpetual", "ticker.ETH.perpetual"]
    pub fn perpetual_tickers(bases: Vec<&str>) -> Vec<String> {
        bases
            .iter()
            .map(|&b| ticker_kind(b, kind::PERPETUAL))
            .collect()
    }

    /// Subscribe to both options mark prices and perpetual tickers
    pub fn vol_monitoring(bases: Vec<&str>) -> Vec<String> {
        let mut channels = options_markprices();
        channels.extend(perpetual_tickers(bases));
        channels
    }

    /// Subscribe to deribit price index for BTC and ETH
    /// Returns: ["deribit_price_index.btc_usd", "deribit_price_index.eth_usd"]
    pub fn price_indices(bases: Vec<&str>) -> Vec<String> {
        bases
            .iter()
            .map(|&b| deribit_price_index(&format!("{}_usd", b.to_lowercase())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_builders() {
        assert_eq!(ticker_base("BTC"), "ticker.BTC");
        assert_eq!(ticker_kind("BTC", "perpetual"), "ticker.BTC.perpetual");
        assert_eq!(ticker_instrument("BTC-PERPETUAL"), "ticker.BTC-PERPETUAL");
        assert_eq!(markprice_options("btc_usd"), "markprice.options.btc_usd");
        assert_eq!(book("BTC-PERPETUAL"), "book.BTC-PERPETUAL");
        assert_eq!(book_depth("BTC-PERPETUAL", 20), "book.BTC-PERPETUAL.20");
    }

    #[test]
    fn test_presets() {
        let mp = presets::options_markprices();
        assert_eq!(mp.len(), 2);
        assert!(mp.contains(&"markprice.options.btc_usd".to_string()));
        assert!(mp.contains(&"markprice.options.eth_usd".to_string()));
    }

    #[test]
    fn test_deribit_price_index_builder() {
        assert_eq!(
            deribit_price_index("btc_usd"),
            "deribit_price_index.btc_usd"
        );
        assert_eq!(
            deribit_price_index("eth_usd"),
            "deribit_price_index.eth_usd"
        );
    }

    #[test]
    fn test_price_indices_preset() {
        let indices = presets::price_indices(vec!["BTC", "ETH"]);
        assert_eq!(indices.len(), 2);
        assert!(indices.contains(&"deribit_price_index.btc_usd".to_string()));
        assert!(indices.contains(&"deribit_price_index.eth_usd".to_string()));
    }
}
