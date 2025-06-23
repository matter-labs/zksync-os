use rig::chain::BlockExtraStats;
use rig::log::{info, warn};

pub fn compute_ratio(stats: BlockExtraStats) -> Option<f64> {
    // Check for native model
    let native_used = match stats.native_used {
        Some(x) => x,
        None => {
            warn!("Native usage not reported, remember to enable the report_native feature!");
            return None;
        }
    };
    info!("Native used: {native_used}");
    let effective_used = match stats.effective_used {
        Some(x) => x,
        None => {
            warn!(
                "Effective cycles usage not reported, remember to enable the cycle_marker feature!"
            );
            return None;
        }
    };
    info!("Effective cycles: {effective_used}");
    let ratio = native_used as f64 / effective_used as f64;
    info!("Native/effective ratio: {ratio}");
    Some(ratio)
}
