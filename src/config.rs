//! Configuration parameters for FutureSight dashboard
use std::time::Duration;

pub const STALE_AFTER: Duration = Duration::from_secs(20);

/// Number of blocks of history to keep in memory
pub const DEFAULT_MAX_BLOCK_HISTORY: usize = 20;

/// Maximum number of missing blocks to backfill per update cycle
pub const MAX_BACKFILL_PER_CYCLE: u64 = 6;

/// How long to wait before considering the chain halted
pub const BLOCK_DELAY_DEFAULT: u64 = 60;

// ========================= GAS CONFIG =========================
/// Number of blocks to request in eth_feeHistory per poll
pub const FEE_HISTORY_BLOCKS: u64 = 20;
/// Percentiles (0..=100) for eth_feeHistory rewards (tips)
pub const FEE_HISTORY_PERCENTILES: [f64; 5] = [10.0, 25.0, 50.0, 75.0, 90.0];
/// Max fee headroom factor applied to priority fee when computing suggested maxFee
pub const SUGGESTION_RAMP_FACTOR: f64 = 2.0;
/// High gas price/base fee warning threshold (in Gwei)
pub const GAS_ALERT_HIGH_GWEI: f64 = 100.0;
/// Spike multiplier threshold for base fee vs MA
pub const GAS_SPIKE_MULTIPLIER: f64 = 2.0;
// Utilization moving average window is aligned with FEE_HISTORY_BLOCKS in code paths
