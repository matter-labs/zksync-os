use super::db::BlockStatus;
use rig::log::info;

/// Statistics tracking for live run execution.
pub struct RunStatistics {
    pub total_block_time: std::time::Duration,
    pub total_overhead_time: std::time::Duration,
    pub total_prefetch_time: std::time::Duration,
    pub blocks_actually_processed: u64,
    pub blocks_skipped_trace_fetch: u64,
    pub blocks_skipped_already_succeeded: u64,
    pub prefetch_hits: u64,
    pub prefetch_misses: u64,
    pub total_blocks_prefetched: u64,
    pub failures: usize,
    pub critical_failures: usize, // Failures that count towards MAX_FAILURES
}

impl RunStatistics {
    pub fn new() -> Self {
        Self {
            total_block_time: std::time::Duration::ZERO,
            total_overhead_time: std::time::Duration::ZERO,
            total_prefetch_time: std::time::Duration::ZERO,
            blocks_actually_processed: 0,
            blocks_skipped_trace_fetch: 0,
            blocks_skipped_already_succeeded: 0,
            prefetch_hits: 0,
            prefetch_misses: 0,
            total_blocks_prefetched: 0,
            failures: 0,
            critical_failures: 0,
        }
    }
}

pub fn log_run_statistics(
    start_block: u64,
    end_block: u64,
    chain_id: u64,
    init_time: std::time::Duration,
    total_time: std::time::Duration,
    stats: &RunStatistics,
) {
    let blocks_in_range = (end_block - start_block + 1) as f64;
    let total_overhead_other = total_time
        .saturating_sub(init_time)
        .saturating_sub(stats.total_block_time)
        .saturating_sub(stats.total_overhead_time)
        .saturating_sub(stats.total_prefetch_time);
    
    info!("=== Live Run Completed ===");
    info!("Blocks in range: {} ({} to {})", blocks_in_range as u64, start_block, end_block);
    info!("Blocks actually processed: {}", stats.blocks_actually_processed);
    info!("Blocks skipped (already succeeded): {}", stats.blocks_skipped_already_succeeded);
    info!("Blocks skipped (trace fetch failed): {}", stats.blocks_skipped_trace_fetch);
    info!("Blocks skipped (other): {}", blocks_in_range as u64 - stats.blocks_actually_processed - stats.blocks_skipped_already_succeeded - stats.blocks_skipped_trace_fetch);
    info!("Failures: {} ({} critical)", stats.failures, stats.critical_failures);
    info!("");
    info!("=== Timing Breakdown ===");
    info!("  Initialization:      {:6.2}ms ({:5.1}%)", init_time.as_secs_f64() * 1000.0, init_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    if stats.blocks_actually_processed > 0 {
        info!("  Block execution:      {:6.2}ms ({:5.1}%)", stats.total_block_time.as_secs_f64() * 1000.0, stats.total_block_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
        info!("  Per-block overhead:  {:6.2}ms ({:5.1}%)", stats.total_overhead_time.as_secs_f64() * 1000.0, stats.total_overhead_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
        info!("    (webhooks, error handling)");
        if stats.total_prefetch_time.as_secs_f64() > 0.0 {
            info!("  Prefetching:          {:6.2}ms ({:5.1}%)", stats.total_prefetch_time.as_secs_f64() * 1000.0, stats.total_prefetch_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
            info!("    Blocks prefetched: {} (avg {:.2}ms per block)", 
                stats.total_blocks_prefetched,
                stats.total_prefetch_time.as_secs_f64() * 1000.0 / stats.total_blocks_prefetched.max(1) as f64
            );
        }
        info!("  Other overhead:      {:6.2}ms ({:5.1}%)", total_overhead_other.as_secs_f64() * 1000.0, total_overhead_other.as_secs_f64() / total_time.as_secs_f64() * 100.0);
        info!("  Total:               {:6.2}ms ({:5.1}%)", total_time.as_secs_f64() * 1000.0, 100.0);
        info!("");
        let avg_time_per_block = stats.total_block_time.as_secs_f64() / stats.blocks_actually_processed as f64;
        let avg_total_per_block = total_time.as_secs_f64() / stats.blocks_actually_processed as f64;
        info!("=== Per Block Averages ===");
        info!("  Execution time:      {:.2}ms", avg_time_per_block * 1000.0);
        info!("  Total time (w/ overhead): {:.2}ms", avg_total_per_block * 1000.0);
        info!("  Blocks per second:  {:.2}", stats.blocks_actually_processed as f64 / total_time.as_secs_f64());
        if stats.prefetch_hits + stats.prefetch_misses > 0 {
            let prefetch_hit_rate = stats.prefetch_hits as f64 / (stats.prefetch_hits + stats.prefetch_misses) as f64 * 100.0;
            info!("=== Prefetch Statistics ===");
            info!("  Prefetch hits:       {} ({:.1}%)", stats.prefetch_hits, prefetch_hit_rate);
            info!("  Prefetch misses:     {} ({:.1}%)", stats.prefetch_misses, 100.0 - prefetch_hit_rate);
            info!("  Total blocks prefetched: {}", stats.total_blocks_prefetched);
        }
    }
    info!("==========================");
}
