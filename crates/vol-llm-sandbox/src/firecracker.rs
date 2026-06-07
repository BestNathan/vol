//! Firecracker microVM sandbox — lightweight KVM-based isolation.
//!
//! Linux only. Requires the `firecracker` binary on PATH and KVM access.
//! Spawns microVMs via REST API over Unix socket, runs commands via SSH,
//! and manages a pool of pre-warmed VMs for low-latency execution.
