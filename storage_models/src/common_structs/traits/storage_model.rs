use super::snapshottable_io::SnapshottableIo;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system_io_oracle::IOOracle;
use zk_ee::utils::Bytes32;
use zk_ee::{
    system::{
        errors::{InternalError, SystemError, UpdateQueryError},
        logger::Logger,
        AccountData, AccountDataRequest, IOResultKeeper, Maybe, Resources,
    },
    types_config::SystemIOTypesConfig,
};

///
/// Storage model trait needed to allow using different storage models in the system.
///
/// It defines methods to read/write contracts storage slots and account data,
/// but all the details about underlying structure, commitment, pubdata compression are hidden behind this trait.
///
pub trait StorageModel: Sized + SnapshottableIo {
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;
    type StorageCommitment;

    fn finish_tx(&mut self) -> Result<(), InternalError>;

    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    // returns old value
    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

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
        >,
        SystemError,
    >;

    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
        oracle: &mut impl zk_ee::system_io_oracle::IOOracle,
    ) -> Result<u64, UpdateQueryError>;

    fn update_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        update_fn: impl FnOnce(
            &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        ) -> Result<
            <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
            UpdateQueryError,
        >,
        oracle: &mut impl IOOracle,
    ) -> Result<
        <Self::IOTypes as zk_ee::types_config::SystemIOTypesConfig>::NominalTokenValue,
        UpdateQueryError,
    >;

    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<
        <Self::IOTypes as zk_ee::types_config::SystemIOTypesConfig>::NominalTokenValue,
        SystemError,
    >;

    fn transfer_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        oracle: &mut impl IOOracle,
    ) -> Result<(), UpdateQueryError>;

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        bytecode_len: u32,
        artifacts_len: u32,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError>;

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

    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<(), SystemError>;

    type Allocator: core::alloc::Allocator + Clone;
    type InitData;

    fn construct(init_data: Self::InitData, allocator: Self::Allocator) -> Self;

    /// Get amount of pubdata needed to encode current tx diff in bytes.
    fn pubdata_used_by_tx(&self) -> u32;

    ///
    /// Finish work, there are 3 outputs:
    /// - state changes: uncompressed state diffs(including new preimages), writes to `results_keeper`
    /// - pubdata - compressed state diffs(including preimages) that should be posted on the DA layer, writes to `results_keeper` and `pubdata_hasher`.
    /// - new state commitment: if `state_commitment` is `Some` - verifies all the reads, applies writes and updates state commitment
    ///
    // Currently, result_keeper accepts storage diffs and preimages.
    // However, future storage models may require different format, so we'll need to generalize it.
    fn finish(
        self,
        oracle: &mut impl IOOracle, // oracle is needed here to prove tree
        state_commitment: Option<&mut Self::StorageCommitment>,
        pubdata_hasher: &mut impl crypto::MiniDigest,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError>;
}
