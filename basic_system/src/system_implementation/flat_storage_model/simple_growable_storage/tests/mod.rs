use super::*;
use crypto::blake2_ext::Digest;
use proptest::{prelude::*, sample::Index};
use ruint::aliases::{B160, U256};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops;
use std::panic::{catch_unwind, AssertUnwindSafe};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::oracle::memory_io::host::{NativeQuerierMemory, ReadQueryInput, ResponseBuffer};
use zk_ee::oracle::memory_io::MemoryOracle;
use zk_ee::system::NullLogger;

#[allow(clippy::extra_unused_lifetimes)]
mod reference;

mod harness;
use harness::*;
mod fixtures;
use fixtures::*;
mod adversarial;
mod batch;
mod bounds;
mod components;
mod hashers;
mod mutations;
mod tree;
pub(super) use harness::{count, Counter, HARDENING};
