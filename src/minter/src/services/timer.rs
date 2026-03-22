use core::time::Duration;

use ic_cdk_timers::set_timer_interval_serial;

use super::rebalance;

/// How often the automatic rebalance cycle runs (1 hour).
const REBALANCE_INTERVAL: Duration = Duration::from_secs(3600);

/// Starts the recurring rebalance timer.
///
/// Must be called exactly once during `init` and once during `post_upgrade`.
/// IC clears all timers on upgrade, so `post_upgrade` must re-arm.
///
/// Uses `set_timer_interval_serial`: if a rebalance cycle is still running
/// when the next tick fires, the new invocation is skipped — no concurrent
/// executions, no manual reentrancy guard needed.
pub(crate) fn start_rebalance_timer() {
    set_timer_interval_serial(REBALANCE_INTERVAL, async || {
        rebalance::rebalance_all().await;
    });
}
