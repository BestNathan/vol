-- TDengine Database Schema for vol-monitor
-- Run this script to initialize the database schema
-- Usage: taos -f init_tdengine.sql

-- Create database
CREATE DATABASE IF NOT EXISTS vol_monitor;

-- Use the database
USE vol_monitor;

-- Create alerts table (super table)
CREATE STABLE IF NOT EXISTS alerts (
    ts TIMESTAMP,
    symbol NCHAR(32),
    alert_type NCHAR(64),
    severity NCHAR(16),
    value DOUBLE,
    threshold DOUBLE,
    message NCHAR(255)
) TAGS (
    rule_name NCHAR(64)
);

-- Create IV curves table (super table)
CREATE STABLE IF NOT EXISTS iv_curves (
    ts TIMESTAMP,
    instrument NCHAR(32),
    delta DOUBLE,
    iv DOUBLE,
    expiry TIMESTAMP,
    underlying_price DOUBLE,
    bid DOUBLE,
    ask DOUBLE
) TAGS (
    expiry_date NCHAR(16)
);

-- Create market data table (super table)
CREATE STABLE IF NOT EXISTS market_data (
    ts TIMESTAMP,
    instrument NCHAR(32),
    price DOUBLE,
    bid DOUBLE,
    ask DOUBLE,
    volume DOUBLE,
    open_interest DOUBLE,
    funding_rate DOUBLE,
    mark_price DOUBLE
) TAGS (
    instrument_type NCHAR(16)
);

-- Create rules table
CREATE TABLE IF NOT EXISTS rules (
    ts TIMESTAMP,
    rule_name NCHAR(64),
    rule_type NCHAR(32),
    threshold DOUBLE,
    enabled BOOL,
    config NCHAR(1024),
    created_at TIMESTAMP,
    updated_at TIMESTAMP
);

-- Create sample subtables (optional - actual tables will be created automatically by the application)
-- CREATE TABLE IF NOT EXISTS alerts_btc_perp USING alerts TAGS ('vol_spike_monitor');
-- CREATE TABLE IF NOT EXISTS iv_curves_btc USING iv_curves TAGS ('20231229');
-- CREATE TABLE IF NOT EXISTS market_data_btc_perp USING market_data TAGS ('perpetual');
