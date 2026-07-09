//! IO caches shared by the storage models.
//!
//! # Charging invariant: independence from cache state
//!
//! Resource charging must be a pure function of the *included* transactions.
//! The sequencer run also executes transactions that are later dropped from the
//! block (late validation failure or block-limit eviction), while the proving
//! run re-executes only the included ones. `HistoryMap` initial records survive
//! the dropped transaction's rollback, so cache *presence* carries no
//! information about who paid for materialization and must never select a
//! charge amount.
//!
//! Derive every charging decision only from:
//! - block-start facts fixed at materialization
//!   ([`cache_element_properties::CacheElementProperties::is_new_element`]),
//! - rollback-aware metadata updated through `HistoryMap` records
//!   (`last_touched_in_tx`, `write_extra_charged_in_tx`,
//!   `new_read_extra_charged`, `persist_charged_in_tx`),
//! - cache values (`initial` / `committed` / `current`), which roll back too.
//!
//! See `docs/system/io/caches.md` ("Charging invariant") for the full
//! discussion and the regression tests in
//! `tests/instances/transactions/src/storage_charging.rs`.

pub mod basic_account_properties;
pub mod cache_element_properties;
pub mod generic_pubdata_aware_plain_storage;
pub mod storage_access_policy;
