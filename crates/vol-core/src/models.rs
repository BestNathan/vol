use serde::{Deserialize, Serialize};

/// Option type: Call or Put
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionType::Call => write!(f, "C"),
            OptionType::Put => write!(f, "P"),
        }
    }
}

/// Tenor classification based on DTE (days to expiry)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tenor {
    Short,  // DTE <= 7
    Medium, // 20 < DTE < 40
    Long,   // DTE > 80
}

impl std::fmt::Display for Tenor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tenor::Short => write!(f, "short"),
            Tenor::Medium => write!(f, "medium"),
            Tenor::Long => write!(f, "long"),
        }
    }
}

/// Unified data model - all data sources emit this structure.
///
/// This ensures the rest of the system doesn't need to know about
/// specific API details from Deribit, Binance, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityData {
    /// Option symbol, e.g., "BTC-29MAR24-70000-C"
    pub symbol: String,

    /// Days to expiry
    pub dte: u32,

    /// Implied volatility (0.0 - 1.0, where 0.5 = 50%)
    pub iv: f64,

    /// Unix timestamp in milliseconds
    pub timestamp: u64,

    /// Data source name, e.g., "deribit", "binance"
    pub source: String,

    /// Option strike price
    pub strike: f64,

    /// Option type (Call/Put)
    pub option_type: OptionType,

    /// Underlying index price (e.g., BTC/USD index price)
    pub index_price: f64,

    /// Delta (optional greek data)
    pub delta: Option<f64>,

    /// Source-specific extra fields
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl VolatilityData {
    /// Classify the tenor based on DTE.
    /// Returns None for gap regions (8-20 days, 40-79 days) where no tenor-based alerts apply.
    pub fn tenor(&self) -> Option<Tenor> {
        classify_tenor(self.dte)
    }

    /// Check if the option is In-The-Money (ITM)
    ///
    /// Call options are ITM when index_price > strike
    /// Put options are ITM when index_price < strike
    pub fn is_itm(&self) -> bool {
        match self.option_type {
            OptionType::Call => self.index_price > self.strike,
            OptionType::Put => self.index_price < self.strike,
        }
    }

    /// Calculate moneyness (how far ITM/OTM)
    ///
    /// For calls: (index_price - strike) / strike
    /// For puts: (strike - index_price) / strike
    ///
    /// Positive = ITM, Negative = OTM, Zero = ATM
    pub fn moneyness(&self) -> f64 {
        match self.option_type {
            OptionType::Call => (self.index_price - self.strike) / self.strike,
            OptionType::Put => (self.strike - self.index_price) / self.strike,
        }
    }

    /// Check if the option is At-The-Money (ATM)
    ///
    /// ATM is defined as |moneyness| <= max_moneyness
    /// Default threshold is 5% from the index price
    pub fn is_atm(&self, max_moneyness: f64) -> bool {
        self.moneyness().abs() <= max_moneyness
    }
}

/// Tenor classification based on DTE (days to expiry).
///
/// Business rule classifications - gaps between ranges are intentional.
/// Options in gap regions don't trigger tenor-based alerts.
///
/// Default ranges:
/// - Short:  DTE <= 7
/// - Medium: 20 < DTE < 40
/// - Long:   DTE >= 80
pub fn classify_tenor(dte: u32) -> Option<Tenor> {
    if dte <= 7 {
        Some(Tenor::Short)
    } else if dte > 20 && dte < 40 {
        Some(Tenor::Medium)
    } else if dte >= 80 {
        Some(Tenor::Long)
    } else {
        // Gap regions: 8-20 days or 40-79 days
        // These don't trigger tenor-based alerts
        None
    }
}
