//! Per-precompile gas / native resource stats tracer.
//!
//! Mirrors `EvmOpcodeStatsTracer` but keyed on EVM precompile addresses
//! (e.g. 0x01 = ECRECOVER, 0x05 = MODEXP, 0x0100 = P256 verify, …).
//! Source of truth for the address list:
//! `evm_interpreter::precompile_addresses::PRECOMPILE_ADDRESSES_LOWS`.

use std::collections::BTreeMap;
use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;

use evm_interpreter::precompile_addresses::PRECOMPILE_ADDRESSES_LOWS;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        evm::{EvmError, EvmFrameInterface},
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, Computational, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources,
        SystemTypes,
    },
    types_config::SystemIOTypesConfig,
};

#[derive(Clone, Default)]
pub struct PrecompileStats {
    pub count: u64,
    pub total_gas: u64,
    pub total_native: u64,
    pub gas_samples: Vec<u64>,
    pub native_samples: Vec<u64>,
}

impl PrecompileStats {
    fn median(samples: &[u64]) -> u64 {
        if samples.is_empty() {
            return 0;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            ((sorted[mid - 1] as u128 + sorted[mid] as u128) / 2) as u64
        } else {
            sorted[mid]
        }
    }

    pub fn gas_median(&self) -> u64 {
        Self::median(&self.gas_samples)
    }

    pub fn native_median(&self) -> u64 {
        Self::median(&self.native_samples)
    }

    pub fn gas_min(&self) -> u64 {
        self.gas_samples.iter().copied().min().unwrap_or(0)
    }

    pub fn gas_max(&self) -> u64 {
        self.gas_samples.iter().copied().max().unwrap_or(0)
    }

    pub fn native_min(&self) -> u64 {
        self.native_samples.iter().copied().min().unwrap_or(0)
    }

    pub fn native_max(&self) -> u64 {
        self.native_samples.iter().copied().max().unwrap_or(0)
    }

    pub fn record(&mut self, gas: u64, native: u64) {
        self.count += 1;
        self.total_gas += gas;
        self.total_native += native;
        self.gas_samples.push(gas);
        self.native_samples.push(native);
    }

    /// Dump per-execution samples to a writer: one line per execution with "gas,native".
    /// Samples are in execution order — the Kth line is the Kth execution.
    pub fn dump_samples(&self, writer: &mut impl Write) -> std::io::Result<()> {
        for (g, n) in self.gas_samples.iter().zip(self.native_samples.iter()) {
            writeln!(writer, "{},{}", g, n)?;
        }
        Ok(())
    }
}

/// Map u16 precompile-low-address → human-readable name.
/// Covers every variant in `PRECOMPILE_ADDRESSES_LOWS` regardless of feature
/// gates (rows for inactive precompiles are simply unused).
pub fn precompile_name(low: u16) -> &'static str {
    match low {
        0x0001 => "ecrecover",
        0x0002 => "sha256",
        0x0003 => "ripemd160",
        0x0004 => "identity",
        0x0005 => "modexp",
        0x0006 => "ecadd",
        0x0007 => "ecmul",
        0x0008 => "ecpairing",
        0x0009 => "blake2f",
        0x000a => "point_eval",
        0x000b => "bls12_g1add",
        0x000c => "bls12_g1msm",
        0x000d => "bls12_g2add",
        0x000e => "bls12_g2msm",
        0x000f => "bls12_pairing_check",
        0x0010 => "bls12_map_fp_to_g1",
        0x0011 => "bls12_map_fp2_to_g2",
        0x0100 => "p256_verify",
        #[cfg(feature = "fri_precompile")]
        0x7003 => "fri_proof",
        _ => {
            debug_assert!(false, "precompile_name: unexpected low-address {low:#06x}");
            "unknown"
        }
    }
}

/// Write a stats map to a CSV file.
pub fn write_stats_csv(stats: &BTreeMap<u16, PrecompileStats>, path: &Path) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "name,address,count,avg_gas,median_gas,min_gas,max_gas,\
         avg_native,median_native,min_native,max_native,native_per_gas"
    )?;
    for (&addr, s) in stats {
        if s.count == 0 {
            continue;
        }
        let avg_gas = s.total_gas as f64 / s.count as f64;
        let avg_native = s.total_native as f64 / s.count as f64;
        let native_per_gas = if s.total_gas > 0 {
            s.total_native as f64 / s.total_gas as f64
        } else {
            0.0
        };
        writeln!(
            f,
            "{name},0x{addr:04x},{count},{avg_gas:.2},{med_gas},{min_gas},{max_gas},\
             {avg_native:.2},{med_native},{min_native},{max_native},{ratio:.4}",
            name = precompile_name(addr),
            addr = addr,
            count = s.count,
            avg_gas = avg_gas,
            med_gas = s.gas_median(),
            min_gas = s.gas_min(),
            max_gas = s.gas_max(),
            avg_native = avg_native,
            med_native = s.native_median(),
            min_native = s.native_min(),
            max_native = s.native_max(),
            ratio = native_per_gas,
        )?;
    }
    Ok(())
}

/// Check if a callee address is an EVM precompile. Returns the 16-bit low
/// halfword if yes.
fn precompile_id_from_address(addr_bytes: &[u8]) -> Option<u16> {
    if addr_bytes.len() != 20 {
        return None;
    }
    if !addr_bytes[..18].iter().all(|&b| b == 0) {
        return None;
    }
    let low = u16::from_be_bytes([addr_bytes[18], addr_bytes[19]]);
    if PRECOMPILE_ADDRESSES_LOWS.contains(&low)
        || (cfg!(feature = "fri_precompile")
            && low == system_hooks::addresses_constants::FRI_PRECOMPILE_ADDRESS_LOW)
    {
        Some(low)
    } else {
        None
    }
}

struct PendingFrame {
    precompile_id: u16,
    ergs_in: u64,
    native_in: u64,
}

pub struct PrecompileStatsTracer<S: SystemTypes> {
    pub stats: BTreeMap<u16, PrecompileStats>,
    pending: Option<PendingFrame>,
    /// `fn() -> S` is always `Send + Sync` regardless of `S`, so the tracer
    /// can be held in a `static OnceLock<Mutex<...>>` even when `S` itself
    /// contains non-`Sync` types (e.g. `Box<dyn OracleQueryProcessor>` in
    /// `ForwardRunningSystem`).
    _marker: PhantomData<fn() -> S>,
}

impl<S: SystemTypes> Default for PrecompileStatsTracer<S> {
    fn default() -> Self {
        Self {
            stats: BTreeMap::new(),
            pending: None,
            _marker: PhantomData,
        }
    }
}

impl<S: SystemTypes> PrecompileStatsTracer<S> {
    pub fn write_csv(&self, path: &Path) -> std::io::Result<()> {
        write_stats_csv(&self.stats, path)
    }

    /// Dump per-execution samples to a directory.
    /// Creates one file per precompile: `<dir>/<name>.samples` with "gas,native" per line.
    /// File names use the user-facing names from `precompile_name`.
    /// Files are in execution order so line K = Kth execution.
    pub fn dump_samples(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for (&addr, s) in &self.stats {
            if s.gas_samples.is_empty() {
                continue;
            }
            let name = precompile_name(addr);
            let path = dir.join(format!("{name}.samples"));
            let mut f = std::fs::File::create(path)?;
            s.dump_samples(&mut f)?;
        }
        Ok(())
    }

    /// Print a human-readable stats table to stdout. Omits the
    /// `native_per_gas` ratio column emitted by `write_csv` — that's
    /// CSV-only.
    pub fn print_stats(&self) {
        println!("=== EVM Precompile Stats:");
        println!(
            "{:<22} {:>8} {:>12} {:>12} {:>12} {:>12} {:>14} {:>14} {:>14} {:>14}",
            "precompile",
            "count",
            "avg_gas",
            "med_gas",
            "min_gas",
            "max_gas",
            "avg_native",
            "med_native",
            "min_native",
            "max_native",
        );
        for (&addr, s) in &self.stats {
            if s.count == 0 {
                continue;
            }
            let avg_gas = s.total_gas as f64 / s.count as f64;
            let avg_native = s.total_native as f64 / s.count as f64;
            println!(
                "{:<22} {:>8} {:>12.1} {:>12} {:>12} {:>12} {:>14.1} {:>14} {:>14} {:>14}",
                precompile_name(addr),
                s.count,
                avg_gas,
                s.gas_median(),
                s.gas_min(),
                s.gas_max(),
                avg_native,
                s.native_median(),
                s.native_min(),
                s.native_max(),
            );
        }
        println!("==================");
    }
}

impl<S: EthereumLikeTypes> Tracer<S> for PrecompileStatsTracer<S> {
    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>) {
        // Single-slot pending state: precompile frames are leaves in the
        // current EVM dispatch (a precompile body cannot itself enter
        // another EVM frame through this hook). If that invariant ever
        // changes — e.g. a precompile bounces back through the EVM call
        // frame mechanism — the assertion below catches the silent
        // overwrite that would otherwise misattribute stats. Promote to
        // a stack of `Option<PendingFrame>` if that becomes a real path.
        // Hard-asserted (rather than debug-asserted) because this tracer
        // only runs host-side as part of benchmarking — silently
        // misattributing stats in release builds is worse than panicking
        // in a non-production codepath.
        assert!(
            self.pending.is_none(),
            "PrecompileStatsTracer: a new execution frame opened while a precompile pending frame was still recorded — \
             nested precompile dispatch would clobber the outer frame's stats."
        );
        let addr = &request.external_call.callee;
        let bytes: [u8; 20] = addr.to_be_bytes::<{ ruint::aliases::B160::BYTES }>();
        if let Some(id) = precompile_id_from_address(&bytes) {
            let ergs_in = request.external_call.available_resources.ergs().0 / ERGS_PER_GAS;
            let native_in = request.external_call.available_resources.native().as_u64();
            self.pending = Some(PendingFrame {
                precompile_id: id,
                ergs_in,
                native_in,
            });
        } else {
            self.pending = None;
        }
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let Some((post, _)) = result else {
            return;
        };
        // `post.ergs().0` unwraps the inner u64 of the Ergs newtype (not a
        // tuple index). Equivalent: `post.ergs().to_u64()` if available.
        let ergs_out = post.ergs().0 / ERGS_PER_GAS;
        let native_out = post.native().as_u64();
        let gas_used = pending.ergs_in.saturating_sub(ergs_out);
        let native_used = pending.native_in.saturating_sub(native_out);
        self.stats
            .entry(pending.precompile_id)
            .or_default()
            .record(gas_used, native_used);
    }

    #[inline(always)]
    fn begin_tx(&mut self, _calldata: &[u8]) {}

    #[inline(always)]
    fn finish_tx(&mut self) {}

    #[inline(always)]
    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: <S::IOTypes as SystemIOTypesConfig>::Address,
        _new_bytecode: Option<&[u8]>,
        _new_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        _new_observable_bytecode_length: u32,
    ) {
    }

    #[inline(always)]
    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        _data: &[u8],
    ) {
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }
}

// EvmTracer no-op forwarder so `evm_tracer()` can return `self`.
impl<S: EthereumLikeTypes> EvmTracer<S> for PrecompileStatsTracer<S> {
    #[inline(always)]
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_opcode_error(&mut self, _error: &EvmError, _frame_state: &impl EvmFrameInterface<S>) {}

    #[inline(always)]
    fn on_call_error(&mut self, _error: &EvmError) {}

    #[inline(always)]
    fn on_selfdestruct(
        &mut self,
        _beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_create_request(&mut self, _is_create2: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_empty_is_zero() {
        let s = PrecompileStats::default();
        assert_eq!(s.gas_median(), 0);
        assert_eq!(s.native_median(), 0);
    }

    #[test]
    fn median_odd_count() {
        let mut s = PrecompileStats::default();
        for g in [10u64, 30, 20] {
            s.record(g, g * 2);
        }
        assert_eq!(s.gas_median(), 20);
        assert_eq!(s.native_median(), 40);
    }

    #[test]
    fn median_even_count_averages_middle_two() {
        let mut s = PrecompileStats::default();
        for g in [10u64, 20, 30, 40] {
            s.record(g, g);
        }
        assert_eq!(s.gas_median(), 25);
        assert_eq!(s.native_median(), 25);
    }

    #[test]
    fn min_max_track_extremes() {
        let mut s = PrecompileStats::default();
        for g in [10u64, 50, 30, 70, 20] {
            s.record(g, g);
        }
        assert_eq!(s.gas_min(), 10);
        assert_eq!(s.gas_max(), 70);
        assert_eq!(s.native_min(), 10);
        assert_eq!(s.native_max(), 70);
    }

    #[test]
    fn record_accumulates_totals_and_samples() {
        let mut s = PrecompileStats::default();
        s.record(10, 100);
        s.record(20, 200);
        assert_eq!(s.count, 2);
        assert_eq!(s.total_gas, 30);
        assert_eq!(s.total_native, 300);
        assert_eq!(s.gas_samples, vec![10, 20]);
        assert_eq!(s.native_samples, vec![100, 200]);
    }

    #[test]
    fn write_csv_round_trip() {
        use std::io::Read;

        let mut map: BTreeMap<u16, PrecompileStats> = BTreeMap::new();
        let mut s = PrecompileStats::default();
        s.record(3000, 41912);
        s.record(3000, 41912);
        map.insert(0x0001, s);

        let mut s = PrecompileStats::default();
        s.record(1250, 6_100_000);
        s.record(84210, 9_800_000);
        map.insert(0x0005, s);

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("p.csv");
        write_stats_csv(&map, &path).expect("write");

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines[0],
            "name,address,count,avg_gas,median_gas,min_gas,max_gas,\
             avg_native,median_native,min_native,max_native,native_per_gas"
        );
        assert!(lines.iter().any(|l| l.starts_with("ecrecover,0x0001,2,")));
        assert!(lines.iter().any(|l| l.starts_with("modexp,0x0005,2,")));

        // Full-line check on the deterministic ecrecover row: two identical
        // (3000, 41912) samples → avg=3000.00, med/min/max=3000, avg_native
        // formatted to .2 = 41912.00, med/min/max=41912, ratio=41912/3000=13.9707.
        let expected_ecrecover =
            "ecrecover,0x0001,2,3000.00,3000,3000,3000,41912.00,41912,41912,41912,13.9707";
        assert!(
            lines.contains(&expected_ecrecover),
            "missing exact ecrecover row; got lines: {:?}",
            lines
        );
    }

    #[test]
    fn precompile_id_from_address_filters_non_precompile() {
        // Random non-precompile address.
        let mut addr = [0u8; 20];
        addr[0] = 0xab;
        addr[19] = 0xcd;
        assert_eq!(precompile_id_from_address(&addr), None);
    }

    #[test]
    fn precompile_id_from_address_accepts_ecrecover() {
        // 0x...0001 — ECRECOVER.
        let mut addr = [0u8; 20];
        addr[19] = 0x01;
        assert_eq!(precompile_id_from_address(&addr), Some(0x0001));
    }

    #[test]
    fn precompile_id_from_address_rejects_wrong_length() {
        let short = [0u8; 10];
        assert_eq!(precompile_id_from_address(&short), None);
        let long = [0u8; 32];
        assert_eq!(precompile_id_from_address(&long), None);
    }

    #[test]
    fn precompile_id_from_address_rejects_unknown_low() {
        // Last byte 0x42 is not in PRECOMPILE_ADDRESSES_LOWS.
        let mut addr = [0u8; 20];
        addr[19] = 0x42;
        assert_eq!(precompile_id_from_address(&addr), None);
    }
}
