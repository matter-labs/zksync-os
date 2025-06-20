use rig::chain::BlockExtraStats;
use rig::log::{error, info};

pub fn compute_ratio(stats: BlockExtraStats) -> f64 {
    // Check for native model
    let native_used = match stats.native_used {
        Some(x) => x,
        None => {
            error!("Native usage not reported, remember to enable the report_native feature!");
            panic!()
        }
    };
    info!("Native used: {native_used}");
    let effective_used = match stats.effective_used {
        Some(x) => x,
        None => {
            error!(
                "Effective cycles usage not reported, remember to enable the cycle_marker feature!"
            );
            panic!()
        }
    };
    info!("Effective cycles: {effective_used}");
    let ratio = native_used as f64 / effective_used as f64;
    info!("Native/effective ratio: {ratio}");
    ratio
}
