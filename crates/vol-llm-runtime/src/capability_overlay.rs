// Re-export from vol-llm-core to avoid circular dependency (vol-llm-runtime
// depends on vol-llm-agent, and the agent crate uses CapabilityOverlay).
pub use vol_llm_core::capability_overlay::CapabilityOverlay;
