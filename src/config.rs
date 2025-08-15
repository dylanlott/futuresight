//! Configuration parameters for FutureSight dashboard
use std::time::Duration;

pub const STALE_AFTER: Duration = Duration::from_secs(20);

/// Number of blocks of history to keep in memory
pub const MAX_BLOCK_HISTORY: usize = 20;

/// 
pub const MAX_BACKFILL_PER_CYCLE: u64 = 6;

/// How long to wait before considering the chain halted
pub const BLOCK_DELAY_DEFAULT: u64 = 60;