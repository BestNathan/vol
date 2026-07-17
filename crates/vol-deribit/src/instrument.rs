//! Deribit instrument types and metadata.

use serde::{Deserialize, Serialize};

/// Deribit instrument type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentType {
    Option,
    Future,
    Perpetual,
    Spot,
}

/// Option type (call or put)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OptionType {
    #[serde(rename = "call")]
    Call,
    #[serde(rename = "put")]
    Put,
}

/// Deribit instrument metadata
///
/// Returned from `public/get_instruments` API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitInstrument {
    /// Unique instrument identifier (e.g., "BTC-29MAR24-70000-C")
    pub instrument_name: String,
    /// Instrument kind
    pub kind: InstrumentType,
    /// Trading state
    pub state: String,
    /// Whether instrument is actively trading
    pub is_active: bool,
    /// Option type (for options only)
    #[serde(default)]
    pub option_type: Option<OptionType>,
    /// Strike price (for options)
    #[serde(default)]
    pub strike: Option<f64>,
    /// Expiration timestamp (milliseconds since Unix epoch)
    #[serde(default)]
    pub expiration_timestamp: Option<u64>,
    /// Creation timestamp (milliseconds since Unix epoch)
    #[serde(default)]
    pub creation_timestamp: Option<u64>,
    /// Base currency (e.g., "BTC")
    pub base_currency: String,
    /// Quote currency (e.g., "USD", "BTC")
    pub quote_currency: String,
    /// Settlement currency
    #[serde(default)]
    pub settlement_currency: Option<String>,
    /// Settlement period (e.g., "day", "week")
    #[serde(default)]
    pub settlement_period: Option<String>,
    /// Contract size
    #[serde(default)]
    pub contract_size: Option<f64>,
    /// Minimum trade amount
    #[serde(default)]
    pub min_trade_amount: Option<f64>,
    /// Tick size for price
    #[serde(default)]
    pub tick_size: Option<f64>,
    /// Maker commission rate
    #[serde(default)]
    pub maker_commission: Option<f64>,
    /// Taker commission rate
    #[serde(default)]
    pub taker_commission: Option<f64>,
    /// Price index name
    #[serde(default)]
    pub price_index: Option<String>,
    /// Instrument type (reversed, linear, etc.)
    #[serde(default)]
    pub instrument_type: Option<String>,
    /// Unique instrument ID
    #[serde(default)]
    pub instrument_id: Option<u64>,
    /// Block trade minimum amount
    #[serde(default)]
    pub block_trade_min_trade_amount: Option<f64>,
    /// Block trade commission
    #[serde(default)]
    pub block_trade_commission: Option<f64>,
    /// Block trade tick size
    #[serde(default)]
    pub block_trade_tick_size: Option<f64>,
}

/// Parse Deribit instrument name into components
/// e.g., "BTC-29MAR24-70000-C" -> (BTC, 2024-03-29, 70000, Call)
pub fn parse_instrument_name(name: &str) -> Option<(String, u32, u32, u32, f64, OptionType)> {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() != 4 {
        return None;
    }

    let underlying = parts.first().copied()?.to_string();
    let expiry_str = parts.get(1).copied()?;
    let strike_str = parts.get(2).copied()?;
    let option_type_str = parts.get(3).copied()?;

    let (day, month, year) = parse_expiry(expiry_str)?;
    let strike = strike_str.parse::<f64>().ok()?;

    let option_type = match option_type_str {
        "C" => OptionType::Call,
        "P" => OptionType::Put,
        _ => return None,
    };

    Some((underlying, year, month, day, strike, option_type))
}

/// Parse Deribit expiry format (e.g., "29MAR24" or "1APR26") into components
/// Supports both 2-digit day (29MAR24) and 1-digit day (1APR26) formats
fn parse_expiry(expiry: &str) -> Option<(u32, u32, u32)> {
    if expiry.len() < 5 {
        return None;
    }

    // Find where the month starts (first letter)
    let day_end = expiry.find(|c: char| c.is_ascii_alphabetic())?;
    let day: u32 = expiry[0..day_end].parse().ok()?;

    // Find where the year starts (first digit after month)
    let month_start = day_end;
    let month_end = expiry[month_start..].find(|c: char| c.is_ascii_digit())?;
    let month_str = &expiry[month_start..month_start + month_end];
    let month = month_from_str(month_str)?;

    let year_str = &expiry[month_start + month_end..];
    let year_short: u32 = year_str.parse().ok()?;
    let year = if year_short < 50 {
        2000 + year_short
    } else {
        1900 + year_short
    };

    Some((day, month, year))
}

/// Convert month abbreviation to number
fn month_from_str(s: &str) -> Option<u32> {
    match s.to_uppercase().as_str() {
        "JAN" => Some(1),
        "FEB" => Some(2),
        "MAR" => Some(3),
        "APR" => Some(4),
        "MAY" => Some(5),
        "JUN" => Some(6),
        "JUL" => Some(7),
        "AUG" => Some(8),
        "SEP" => Some(9),
        "OCT" => Some(10),
        "NOV" => Some(11),
        "DEC" => Some(12),
        _ => None,
    }
}

/// Calculate days to expiry from Deribit expiry string
pub fn calculate_dte(expiry: &str) -> Option<u32> {
    let (day, month, year) = parse_expiry(expiry)?;
    let expiry_date = chrono::NaiveDate::from_ymd_opt(year as i32, month, day)?;
    let today = chrono::Utc::now().date_naive();
    let dte = expiry_date.signed_duration_since(today).num_days();

    if dte < 0 {
        Some(0)
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(dte as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_instrument_name() {
        let result = parse_instrument_name("BTC-29MAR24-70000-C").unwrap();
        assert_eq!(result.0, "BTC");
        assert_eq!(result.1, 2024);
        assert_eq!(result.2, 3);
        assert_eq!(result.3, 29);
        assert_eq!(result.4, 70000.0);
        assert_eq!(result.5, OptionType::Call);
    }

    #[test]
    fn test_parse_instrument_name_put() {
        let result = parse_instrument_name("ETH-15JUN25-3000-P").unwrap();
        assert_eq!(result.5, OptionType::Put);
    }

    #[test]
    fn test_month_parsing() {
        assert_eq!(month_from_str("JAN"), Some(1));
        assert_eq!(month_from_str("dec"), Some(12));
        assert_eq!(month_from_str("INVALID"), None);
    }
}
