//! Built-in message compressor implementations.

pub mod position_sample;
pub mod role_filter;

pub use position_sample::PositionSampleCompressor;
pub use role_filter::RoleFilterCompressor;
