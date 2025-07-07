//! IO subsystem interface.
//! Interface is split into a user-facing minimal interface (exposed to EEs)
//! and an extended one with more functionality for use within the
//! rest of the system and bootloader.

use core::marker::PhantomData;

use super::errors::{InternalError, SystemError, UpdateQueryError};
use super::logger::Logger;
use super::{IOResultKeeper, Resources};
use crate::execution_environment_type::ExecutionEnvironmentType;
use crate::kv_markers::MAX_EVENT_TOPICS;
use crate::system::metadata::BlockMetadataFromOracle;
use crate::system_io_oracle::IOOracle;
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};
use crate::utils::Bytes32;
use arrayvec::ArrayVec;
use ruint::aliases::U256;

///
/// User facing IO trait.
/// Completely hides both IO internals (how storage is modeled),
/// as well as any cost model.
///
pub trait IOSubsystem: Sized {
    type Resources: Resources;
    type IOTypes: SystemIOTypesConfig;
    type StateSnapshot;

    /// Read value from storage at a given slot (address, key).
    fn storage_read<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, SystemError>;

    /// Write value in the storage at a given slot (address, key).
    fn storage_write<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        value_to_write: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) -> Result<(), SystemError>;

    /// Read the token balance for a given address.
    fn get_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError>;

    /// Read observable bytecode size for a given address.
    fn get_observable_bytecode_size(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<u32, SystemError>;

    /// Read observable bytecode hash for a given address.
    fn get_observable_bytecode_hash(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue, SystemError>;

    /// Read observable bytecode  for a given address.
    fn get_observable_bytecode(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<&'static [u8], SystemError>;

    /// Get balance of the currently executing address.
    /// WARNING: this function assumes the address's properties to be warm,
    /// it raises an internal error otherwise.
    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError>;

    /// Emit an event.
    fn emit_event(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        topics: &ArrayVec<<Self::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
        data: &[u8],
    ) -> Result<(), SystemError>;

    /// Emit a l1 -> l2 message.
    ///
    /// Returns message data hash.
    fn emit_l1_message(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        data: &[u8],
    ) -> Result<Bytes32, SystemError>;

    /// Mark an account to be destructed at the end of the transaction.
    /// Perform token transfer to beneficiary.
    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        in_constructor: bool,
    ) -> Result<(), SystemError>;

    fn net_pubdata_used(&self) -> Result<u64, InternalError>;

    /// Starts a new "local" frame that does not that memory (like `near_call` in the EraVM).
    /// Returns a snapshot to which the system can rollback to on frame finish.
    fn start_io_frame(&mut self) -> Result<<Self as IOSubsystem>::StateSnapshot, InternalError>;

    /// Finishes "local" frame, reverts I/O writes in case of revert.
    /// If `rollback_handle` is provided, will rollback to requested snapshot.
    fn finish_io_frame(
        &mut self,
        rollback_handle: Option<&<Self as IOSubsystem>::StateSnapshot>,
    ) -> Result<(), InternalError>;
}

pub trait Maybe<T> {
    fn construct(f: impl FnOnce() -> T) -> Self;
    fn try_construct<E>(f: impl FnOnce() -> Result<T, E>) -> Result<Self, E>
    where
        Self: Sized,
    {
        f().map(|x| Self::construct(|| x))
    }
}

pub struct Just<T>(pub T);
impl<T> Maybe<T> for Just<T> {
    fn construct(f: impl FnOnce() -> T) -> Self {
        Self(f())
    }
}
pub struct Nothing;
impl<T> Maybe<T> for Nothing {
    fn construct(_: impl FnOnce() -> T) -> Self {
        Self
    }
}

/// Struct holding the values returned by an account properties
/// read request.
///
/// Each field that was requested in the corresponding [AccountDataRequest] is [Just] and the others are [Nothing].
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountData<
    EEVersion,
    ObservableBytecodeHash,
    ObservableBytecodeLen,
    Nonce,
    BytecodeHash,
    BytecodeLen,
    ArtifactsLen,
    NominalTokenBalance,
    Bytecode,
> {
    pub ee_version: EEVersion,
    pub observable_bytecode_hash: ObservableBytecodeHash,
    pub observable_bytecode_len: ObservableBytecodeLen,
    pub nonce: Nonce,
    pub bytecode_hash: BytecodeHash,
    pub bytecode_len: BytecodeLen,
    pub artifacts_len: ArtifactsLen,
    pub nominal_token_balance: NominalTokenBalance,
    pub bytecode: Bytecode,
}

impl<A, B, C, D, E, F, G> AccountData<A, B, C, D, E, Just<u32>, Just<u32>, F, G> {
    pub fn is_contract(&self) -> bool {
        self.bytecode_len.0 > 0 || self.artifacts_len.0 > 0
    }
}

impl<A, B, C, D, E, F> AccountData<A, B, C, Just<u64>, D, Just<u32>, Just<u32>, E, F> {
    pub fn can_deploy_into(&self) -> bool {
        self.bytecode_len.0 == 0 && self.artifacts_len.0 == 0 && self.nonce.0 == 0
    }
}

/// A ZST for specifying which account fields to get.
pub struct AccountDataRequest<T>(PhantomData<T>);

impl
    AccountDataRequest<
        AccountData<
            Nothing,
            Nothing,
            Nothing,
            Nothing,
            Nothing,
            Nothing,
            Nothing,
            Nothing,
            Nothing,
        >,
    >
{
    pub fn empty() -> Self {
        Self(PhantomData)
    }
}

impl<A, B, C, D, E, F, G, H, I> AccountDataRequest<AccountData<A, B, C, D, E, F, G, H, I>> {
    pub fn with_ee_version(
        self,
    ) -> AccountDataRequest<AccountData<Just<u8>, B, C, D, E, F, G, H, I>> {
        AccountDataRequest(PhantomData)
    }
    pub fn with_observable_bytecode_hash<T>(
        self,
    ) -> AccountDataRequest<AccountData<A, Just<T>, C, D, E, F, G, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_observable_bytecode_len(
        self,
    ) -> AccountDataRequest<AccountData<A, B, Just<u32>, D, E, F, G, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_nonce(self) -> AccountDataRequest<AccountData<A, B, C, Just<u64>, E, F, G, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_bytecode_hash<T>(
        self,
    ) -> AccountDataRequest<AccountData<A, B, C, D, Just<T>, F, G, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_bytecode_len(
        self,
    ) -> AccountDataRequest<AccountData<A, B, C, D, E, Just<u32>, G, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_artifacts_len(
        self,
    ) -> AccountDataRequest<AccountData<A, B, C, D, E, F, Just<u32>, H, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_nominal_token_balance<T>(
        self,
    ) -> AccountDataRequest<AccountData<A, B, C, D, E, F, G, Just<T>, I>> {
        AccountDataRequest(PhantomData)
    }

    pub fn with_bytecode(
        self,
    ) -> AccountDataRequest<AccountData<A, B, C, D, E, F, G, H, Just<&'static [u8]>>> {
        AccountDataRequest(PhantomData)
    }
}

///
/// Extended IO trait for use in the system and bootloader.
///
pub trait IOSubsystemExt: IOSubsystem {
    type IOOracle: IOOracle;
    type FinalData;

    fn init_from_oracle(oracle: Self::IOOracle) -> Result<Self, InternalError>;

    fn oracle(&mut self) -> &mut Self::IOOracle;

    /// Indicate the a new transaction is being processed.
    fn begin_next_tx(&mut self);

    /// Finish current transaction, destructing accounts marked during
    /// selfdestruct.
    fn finish_tx(&mut self) -> Result<(), InternalError>;

    /// Touch a slot (address, key) to make it warm.
    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    /// Read an account's nonce.
    fn read_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<u64, SystemError>;

    /// Increments an account's nonce and
    /// returns the old nonce.
    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
    ) -> Result<u64, UpdateQueryError>;

    /// Perform a transfer of token balance.
    fn transfer_nominal_token_value(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    ) -> Result<(), UpdateQueryError>;

    /// Touch an account to make it warm.
    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    /// Generic function to read some of an account's properties
    fn read_account_properties<
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue>,
        Bytecode: Maybe<&'static [u8]>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        request: AccountDataRequest<
            AccountData<
                EEVersion,
                ObservableBytecodeHash,
                ObservableBytecodeLen,
                Nonce,
                BytecodeHash,
                BytecodeLen,
                ArtifactsLen,
                NominalTokenBalance,
                Bytecode,
            >,
        >,
    ) -> Result<
        AccountData<
            EEVersion,
            ObservableBytecodeHash,
            ObservableBytecodeLen,
            Nonce,
            BytecodeHash,
            BytecodeLen,
            ArtifactsLen,
            NominalTokenBalance,
            Bytecode,
        >,
        SystemError,
    >;

    /// Store bytecode from deployment.
    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        bytecode_len: u32,
        artifacts_len: u32,
    ) -> Result<&'static [u8], SystemError>;

    /// Special method that allows to set bytecode under address by hash.
    /// Also, pubdata for such bytecode will not be published.
    /// This method can be only triggered during special protocol upgrade txs.
    fn set_bytecode_details(
        &mut self,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
    ) -> Result<(), SystemError>;

    fn finish(
        self,
        block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
        logger: impl Logger,
    ) -> Self::FinalData;

    /// Emit a log for a l1 -> l2 tx.
    fn emit_l1_l2_tx_log(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        tx_hash: Bytes32,
        success: bool,
    ) -> Result<(), SystemError>;

    /// Returns old balance
    fn update_account_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        diff: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        should_subtract: bool,
    ) -> Result<U256, UpdateQueryError>;
}

pub trait EthereumLikeIOSubsystem: IOSubsystem<IOTypes = EthereumIOTypesConfig> {}
