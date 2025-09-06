//! Configuration parameters for FutureSight dashboard
use std::time::Duration;

pub const STALE_AFTER: Duration = Duration::from_secs(20);

/// Number of blocks of history to keep in memory
pub const DEFAULT_MAX_BLOCK_HISTORY: usize = 20;

/// Maximum number of missing blocks to backfill per update cycle
pub const MAX_BACKFILL_PER_CYCLE: u64 = 6;

/// How long to wait before considering the chain halted
pub const BLOCK_DELAY_DEFAULT: u64 = 60;