use std::sync::atomic::AtomicU64;

pub static UPGRADE_FAIL_LOG_COUNT: AtomicU64 = AtomicU64::new(0);
pub const UPGRADE_FAIL_LOG_MAX_COUNT: u64 = 10;
