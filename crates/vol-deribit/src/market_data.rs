//! Deribit market data structures.
//!
//! Contains ticker, mark price, order book, and trade data models.

use serde::{Deserialize, Serialize};

/// Ticker data for an instrument
///
/// Received from `ticker.<BASE>` or `ticker.<INSTRUMENT>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitTicker {
    /// Instrument name (e.g., "BTC-29MAR24-70000-C")
    pub instrument_name: String,
    /// Last traded price
    #[serde(default)]
    pub last: Option<f64>,
    /// Mark price
    #[serde(default)]
    pub mark: Option<f64>,
    /// Index price
    #[serde(default)]
    pub index_price: Option<f64>,
    /// Mark implied volatility
    #[serde(default, alias = "iv")]
    pub mark_iv: Option<f64>,
    /// Timestamp (milliseconds since Unix epoch)
    #[serde(default)]
    pub timestamp: Option<u64>,
    /// Best bid price
    #[serde(default)]
    pub best_bid_price: Option<f64>,
    /// Best ask price
    #[serde(default)]
    pub best_ask_price: Option<f64>,
    /// Best bid amount
    #[serde(default)]
    pub best_bid_amount: Option<f64>,
    /// Best ask amount
    #[serde(default)]
    pub best_ask_amount: Option<f64>,
    /// State (open, closed, etc.)
    #[serde(default)]
    pub state: Option<String>,
    /// 24h price change (as decimal, e.g., 0.05 for 5%)
    #[serde(default)]
    pub price_change: Option<f64>,
    /// 24h high price
    #[serde(default)]
    pub high: Option<f64>,
    /// 24h low price
    #[serde(default)]
    pub low: Option<f64>,
    /// 24h volume
    #[serde(default)]
    pub volume: Option<f64>,
    /// 24h volume in USD
    #[serde(default)]
    pub volume_usd: Option<f64>,
    /// Open interest
    #[serde(default)]
    pub open_interest: Option<f64>,
    /// Settlement price
    #[serde(default)]
    pub settlement_price: Option<f64>,
    /// Estimated delivery price (for options)
    #[serde(default)]
    pub estimated_delivery_price: Option<f64>,
    /// Current funding rate (for perpetuals)
    #[serde(default)]
    pub current_funding: Option<f64>,
    /// 8h funding rate (for perpetuals)
    #[serde(default)]
    pub funding_8h: Option<f64>,
    /// Interest value
    #[serde(default)]
    pub interest_value: Option<f64>,
    /// Minimum price allowed
    #[serde(default)]
    pub min_price: Option<f64>,
    /// Maximum price allowed
    #[serde(default)]
    pub max_price: Option<f64>,
}

/// Mark price update for options
///
/// Received from `markprice.options.<INDEX>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionMarkPrice {
    /// Instrument name (e.g., "BTC-29MAR24-70000-C")
    pub instrument_name: String,
    /// Mark price
    pub mark_price: f64,
    /// Implied volatility
    #[serde(default)]
    pub iv: Option<f64>,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: u64,
    /// Index price (included in markprice.options channel)
    #[serde(default)]
    pub index_price: Option<f64>,
}

impl OptionMarkPrice {
    /// Convert to vol-core VolatilityData
    pub fn to_volatility_data(&self) -> Option<vol_core::VolatilityData> {
        let iv = self.iv?;

        // Parse instrument name: "BTC-29MAR24-70000-C"
        let (underlying, _year, month, day, strike, option_type) =
            crate::instrument::parse_instrument_name(&self.instrument_name)?;

        // Calculate DTE from expiry
        let expiry_str = format!("{:02}{}{:02}", day,
            match month {
                1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
                5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
                9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
                _ => return None,
            },
            _year % 100
        );
        let dte = crate::instrument::calculate_dte(&expiry_str)?;

        // Use index_price if available, otherwise use a default based on underlying
        // Deribit's markprice.options channel may not include index_price
        let index_price = self.index_price.unwrap_or_else(|| {
            // Default approximate prices (will be refined when ticker data arrives)
            match underlying.to_lowercase().as_str() {
                "btc" => 100000.0,  // Default BTC price
                "eth" => 5000.0,    // Default ETH price
                _ => strike,         // Fallback to strike (ATM)
            }
        });

        let mut extra = std::collections::HashMap::new();
        extra.insert("underlying".to_string(), serde_json::json!(underlying));
        extra.insert("mark_price".to_string(), serde_json::json!(self.mark_price));
        extra.insert("index_price".to_string(), serde_json::json!(index_price));

        Some(vol_core::VolatilityData {
            symbol: self.instrument_name.clone(),
            dte: dte as u32,
            iv,
            timestamp: self.timestamp,
            source: "deribit".to_string(),
            strike,
            option_type: match option_type {
                crate::instrument::OptionType::Call => vol_core::OptionType::Call,
                crate::instrument::OptionType::Put => vol_core::OptionType::Put,
            },
            index_price,
            delta: None,
            extra,
        })
    }
}

/// Mark price update for index/perpetual
///
/// Received from `markprice.<INDEX>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMarkPrice {
    /// Index name (e.g., "btc_usd")
    pub index: String,
    /// Mark price
    pub price: f64,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: u64,
}

/// Order book snapshot
///
/// Received from `book.<INSTRUMENT>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    /// Instrument name
    pub instrument_name: String,
    /// Bid prices (highest first)
    pub bids: Vec<PriceLevel>,
    /// Ask prices (lowest first)
    pub asks: Vec<PriceLevel>,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: u64,
    /// Change ID for incremental updates
    #[serde(default)]
    pub change_id: Option<u64>,
    /// Previous change ID
    #[serde(default)]
    pub prev_change_id: Option<u64>,
}

/// Single price level in order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price
    pub price: f64,
    /// Amount at this price
    pub amount: f64,
}

/// Trade execution
///
/// Received from `trades.<INSTRUMENT>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Trade ID
    pub trade_id: String,
    /// Instrument name
    pub instrument_name: String,
    /// Trade price
    pub price: f64,
    /// Trade amount
    pub amount: f64,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: u64,
    /// Trade direction (buy/sell)
    pub direction: String,
    /// Whether this was a liquidation
    #[serde(default)]
    pub liquidation: Option<bool>,
    /// Whether this was a block trade
    #[serde(default)]
    pub block_trade: Option<bool>,
}

/// Ticker statistics (24h)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerStats {
    /// 24h high price
    #[serde(default)]
    pub high: Option<f64>,
    /// 24h low price
    #[serde(default)]
    pub low: Option<f64>,
    /// 24h price change (as decimal)
    #[serde(default)]
    pub price_change: Option<f64>,
    /// 24h volume
    #[serde(default)]
    pub volume: Option<f64>,
    /// 24h volume in USD
    #[serde(default)]
    pub volume_usd: Option<f64>,
    /// 24h notional volume
    #[serde(default)]
    pub volume_notional: Option<f64>,
}

impl DeribitTicker {
    /// Convert to vol-core VolatilityData if this is an option with IV
    pub fn to_volatility_data(&self) -> Option<vol_core::VolatilityData> {
        let iv = self.mark_iv?;
        // Use index_price if available, otherwise use mark price as fallback
        let index_price = self.index_price.or(self.mark).or(self.last)?;

        // Parse instrument name
        let (underlying, year, month, day, strike, option_type) =
            crate::instrument::parse_instrument_name(&self.instrument_name)?;

        // Calculate DTE
        let dte = crate::instrument::calculate_dte(
            &format!("{:02}{}{:02}", day,
                match month {
                    1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
                    5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
                    9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
                    _ => return None,
                },
                year % 100
            )
        )?;

        let mut extra = std::collections::HashMap::new();
        extra.insert("underlying".to_string(), serde_json::json!(underlying));
        if let Some(mark) = self.mark {
            extra.insert("mark_price".to_string(), serde_json::json!(mark));
        }
        if let Some(last) = self.last {
            extra.insert("last_price".to_string(), serde_json::json!(last));
        }

        Some(vol_core::VolatilityData {
            symbol: self.instrument_name.clone(),
            dte: dte as u32,
            iv,
            timestamp: self.timestamp.unwrap_or(0),
            source: "deribit".to_string(),
            strike,
            option_type: match option_type {
                crate::instrument::OptionType::Call => vol_core::OptionType::Call,
                crate::instrument::OptionType::Put => vol_core::OptionType::Put,
            },
            index_price,
            delta: None,
            extra,
        })
    }
}
