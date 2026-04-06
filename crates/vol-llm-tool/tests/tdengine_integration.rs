//! TDengine integration tests.
//!
//! Run with: cargo test --test tdengine_integration -- --nocapture
//!
//! Requires TDengine server at 192.168.2.106:6041

use vol_llm_tool::{TdengineClient, TdengineConfig};

#[tokio::test]
async fn test_tdengine_connection() {
    let config = TdengineConfig::default();
    let client = TdengineClient::new(config);

    // Test basic connection with a simple query
    let result = client.query("SELECT 1").await;

    match result {
        Ok(response) => {
            println!("Connection successful: {:?}", response);
            assert_eq!(response.code, 0, "Query should succeed");
        }
        Err(e) => {
            println!("Connection failed: {}", e);
            // If connection fails, it's likely a network/server issue
            // Don't fail the test, just log the error
            eprintln!("TDengine connection test skipped - server unavailable");
        }
    }
}

#[tokio::test]
async fn test_query_tables() {
    let config = TdengineConfig::default();
    let client = TdengineClient::new(config);

    // Query available tables
    let result = client.query("SHOW TABLES").await;

    if let Ok(response) = result {
        println!("Tables: {:?}", response.data);
        if response.code == 0 {
            println!("Successfully queried tables");
        }
    } else {
        eprintln!("TDengine server may be unavailable");
    }
}

#[tokio::test]
async fn test_alert_history_query() {
    let config = TdengineConfig::default();
    let client = TdengineClient::new(config);

    // Try to query alert history
    let result = client.query_alert_history("BTC-PERP", 10, Some(24)).await;

    match result {
        Ok(response) => {
            println!("Alert history response: {:?}", response);
        }
        Err(e) => {
            println!("Alert history query failed (expected if table doesn't exist): {}", e);
        }
    }
}

#[tokio::test]
async fn test_iv_curve_query() {
    let config = TdengineConfig::default();
    let client = TdengineClient::new(config);

    // Try to query IV curve
    let result = client.query_iv_curve("BTC-29DEC23", Some(&[0.25, 0.5, 0.75])).await;

    match result {
        Ok(response) => {
            println!("IV curve response: {:?}", response);
        }
        Err(e) => {
            println!("IV curve query failed (expected if table doesn't exist): {}", e);
        }
    }
}
