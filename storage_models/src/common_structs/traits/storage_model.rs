use super::snapshottable_io::SnapshottableIo;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::oracle::usize_serialization::{WordDeserializable, WordSerializable};
use zk_ee::oracle::IOOracle;
use zk_ee::system::{BalanceSubsystemError, DeconstructionSubsystemError, NonceSubsystemError};
use zk_ee::utils::Bytes32;
use zk_ee::{
    system::{
        errors::system::SystemError, logger::Logger, AccountData, AccountDataRequest,
        IOResultKeeper, Maybe, Resources,
    },
    types_config::SystemIOTypesConfig,
};

///
/// Storage model trait needed to allow using different storage models in the system.
///
/// It defines methods to read/write contracts storage slots and account data,
/// but all the details about underlying structure, commitment, and pubdata compression are hidden behind this trait.
///
pub trait StorageModel: Sized + SnapshottableIo {
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;
    type StorageCommitment: Clone + WordDeserializable + WordSerializable + core::fmt::Debug; // easier to have it here than propagate

    /// Reads a value from contract storage at the given address and key.
    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    /// Touches a storage slot without reading its value, used for warming up storage.
    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError>;

    /// Writes a value to contract storage. Returns the old value.
    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    /// Reads requested account properties for the given address.
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
        CodeVersion: Maybe<u8>,
        IsDelegated: Maybe<bool>,
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
                CodeVersion,
                IsDelegated,
            >,
        >,
        oracle: &mut impl IOOracle,
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
            CodeVersion,
            IsDelegated,
        >,
        SystemError,
    >;

    /// Touches an account without reading its data, used for warming up accounts.
    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError>;

    /// Increments the nonce for the given address. Returns the old nonce value.
    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
        oracle: &mut impl zk_ee::oracle::IOOracle,
    ) -> Result<u64, NonceSubsystemError>;

    /// Updates the nominal token balance for an address using the provided update function.
    fn update_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        update_fn: impl FnOnce(
            &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        ) -> Result<
            <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
            BalanceSubsystemError,
        >,
        oracle: &mut impl IOOracle,
        fee_payment_in_simulation: bool,
    ) -> Result<
        <Self::IOTypes as zk_ee::types_config::SystemIOTypesConfig>::NominalTokenValue,
        BalanceSubsystemError,
    >;

    /// Returns the nominal token balance for the given address.
    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<
        <Self::IOTypes as zk_ee::types_config::SystemIOTypesConfig>::NominalTokenValue,
        SystemError,
    >;

    /// Transfers nominal token value from one address to another.
    fn transfer_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError>;

    /// Deploys bytecode at the given address. Returns the bytecode slice, its hash, and length.
    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        oracle: &mut impl IOOracle,
    ) -> Result<
        (
            &'static [u8],
            <Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
            u32,
        ),
        SystemError,
    >;

    /// Sets bytecode metadata for an account (hash, length, artifacts length, etc.).
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
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError>;

    /// Sets a delegation from one address to another (EIP-7702 style delegation).
    fn set_delegation(
        &mut self,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        delegate: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError>;

    /// Marks an account for deconstruction (self-destruct). Returns the transferred balance.
    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<
        <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        DeconstructionSubsystemError,
    >;

    type Allocator: core::alloc::Allocator + Clone;
    type InitData;

    /// Constructs a new storage model instance from initialization data and an allocator.
    fn construct(init_data: Self::InitData, allocator: Self::Allocator) -> Self;

    /// Get amount of pubdata needed to encode current tx diff in bytes.
    fn pubdata_used_by_tx(&self) -> u32;

    /// Get current counter of refunds
    fn get_refund_counter(&'_ self) -> &'_ Self::Resources;

    /// Add resources to refund at the end of transaction
    fn add_to_refund_counter(&mut self, refund: Self::Resources) -> Result<(), SystemError>;

    /// Persists internal caches to the oracle and result keeper.
    fn persist_caches(
        &mut self,
        oracle: &mut impl IOOracle,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
    );

    /// Reports any new preimages (e.g., bytecode) to the result keeper.
    fn report_new_preimages(&mut self, result_keeper: &mut impl IOResultKeeper<Self::IOTypes>);

    type StorageKey<'a>: 'a + Clone + Copy + PartialEq + Eq + core::fmt::Debug
    where
        Self: 'a;

    type StorageDiff<'a>: 'a + Clone + Copy + PartialEq + Eq + core::fmt::Debug
    where
        Self: 'a;

    /// Returns the diff for a specific storage key, if any changes were made.
    fn get_storage_diff<'a>(&'a self, key: Self::StorageKey<'a>) -> Option<Self::StorageDiff<'a>>;

    /// Returns an iterator over all storage diffs (key, diff pairs).
    fn storage_diffs_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<Item = (Self::StorageKey<'a>, Self::StorageDiff<'a>)> + Clone;

    /// Updates the storage commitment based on current diffs and reports results.
    fn update_commitment(
        &mut self,
        state_commitment: Option<&mut Self::StorageCommitment>,
        oracle: &mut impl IOOracle,
        logger: &mut impl Logger,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
    );
}
