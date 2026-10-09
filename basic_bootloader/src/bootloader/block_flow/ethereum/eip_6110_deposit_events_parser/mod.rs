use crate::bootloader::supported_ees::EthereumLikeSystem;
use ruint::aliases::B160;
use zk_ee::common_structs::GenericEventContentWithTxRef;
use zk_ee::logger_log;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::IOTeardown;
use zk_ee::system::System;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

pub const DEPOSIT_REQUEST_EIP_7685_TYPE: u8 = 0x00;

/// The deposit contract of mainnet (and of Hoodi): `0x00000000219ab540356cBB839Cbe05303d7705Fa`
pub const MAINNET_DEPOSIT_CONTRACT_ADDRESS: B160 =
    B160::from_limbs([0x9cbe05303d7705fa, 0x219ab540356cbb83, 0x00000000]);
/// The deposit contract of Sepolia: `0x7f02C3E3c98b133055B8B348B2Ac625669Ed295D`
pub const SEPOLIA_DEPOSIT_CONTRACT_ADDRESS: B160 =
    B160::from_limbs([0xb2ac625669ed295d, 0xc98b133055b8b348, 0x7f02c3e3]);
/// The deposit contract of Holesky: `0x4242424242424242424242424242424242424242`
pub const HOLESKY_DEPOSIT_CONTRACT_ADDRESS: B160 =
    B160::from_limbs([0x4242424242424242, 0x4242424242424242, 0x42424242]);

/// The deposit contract whose events are the deposit requests: a chain
/// parameter (EIP-6110's `DEPOSIT_CONTRACT_ADDRESS`), by chain id; the
/// mainnet one for a chain not listed
pub fn deposit_contract_address(chain_id: u64) -> B160 {
    match chain_id {
        // Sepolia
        11155111 => SEPOLIA_DEPOSIT_CONTRACT_ADDRESS,
        // Holesky
        17000 => HOLESKY_DEPOSIT_CONTRACT_ADDRESS,
        // mainnet, Hoodi (560048) and the rest
        _ => MAINNET_DEPOSIT_CONTRACT_ADDRESS,
    }
}

const DEPOSIT_EVENT_SIGNATURE_HASH: Bytes32 =
    Bytes32::from_hex("649bbc62d0e31342afea4e5cd82d4049e7e1ee912fc0889aa790803be39038c5");

// it's fully fixed. It's SSZ internally, even though it's not SSZ for the outer vec/list
const DEPOSIT_REQUEST_SERIALIZATION_LEN: usize = 48 + 32 + 8 + 96 + 8;

/// The serialization of one deposit request (EIP-6110): pubkey, withdrawal
/// credentials, amount, signature, index
pub const DEPOSIT_REQUEST_LEN: usize = DEPOSIT_REQUEST_SERIALIZATION_LEN;

/// Where the deposit request bytes go: a hasher, or a buffer collecting them
pub trait DepositRequestsSink {
    fn write(&mut self, bytes: &[u8]);
}

impl<D: crypto::sha256::Digest> DepositRequestsSink for D {
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}

/// Whether the event is a deposit event of the chain's deposit contract
pub fn is_deposit_event(
    event: &GenericEventContentWithTxRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    deposit_contract: &B160,
) -> bool {
    event.address == deposit_contract
        && event.topics.len() > 0
        && event.topics[0] == DEPOSIT_EVENT_SIGNATURE_HASH
}

/// Feeds the hasher with the EIP-7685 type byte and the deposit requests of
/// the block's logs; returns whether there was any. Panics on a malformed
/// deposit event (see [`eip6110_deposit_requests`] for an error instead).
pub fn eip6110_events_parser<S: EthereumLikeSystem>(
    system: &System<S>,
    requests_hasher: &mut impl crypto::sha256::Digest,
) -> Result<bool, SystemError>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    // we can not easily get the number from one scan, so we will accumulate into hasher directly
    match eip6110_deposit_requests(system, requests_hasher, true) {
        Ok(found) => Ok(found),
        Err(()) => panic!("invalid deposit event structure"),
    }
}

/// Writes the deposit requests of the deposit events among the block's (or
/// the range's) logs, in order, to the sink, after the EIP-7685 type byte when
/// `with_type_byte` and there is any; returns whether there was any, or an
/// error for a malformed deposit event
pub fn eip6110_deposit_requests<S: EthereumLikeSystem>(
    system: &System<S>,
    sink: &mut impl DepositRequestsSink,
    with_type_byte: bool,
) -> Result<bool, ()>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    let deposit_contract = deposit_contract_address(system.get_chain_id());
    let mut event_encountered = false;
    let mut logger = system.get_logger();
    for event in system.io.events_iterator() {
        if !is_deposit_event(&event, &deposit_contract) {
            continue;
        }
        if !event_encountered {
            event_encountered = true;
            if with_type_byte {
                sink.write(&[DEPOSIT_REQUEST_EIP_7685_TYPE]);
            }
        }
        validate_and_write_event_data(event, sink, &mut logger)?;
    }
    Ok(event_encountered)
}

fn validate_u16_at_most(input: &[u8; 32], value: u16) -> Result<(), ()> {
    if input[..30].iter().all(|el| *el == 0) == false {
        Err(())
    } else {
        let u16_bytes = [input[30], input[31]];
        if u16::from_be_bytes(u16_bytes) != value {
            Err(())
        } else {
            Ok(())
        }
    }
}

fn validate_and_write_event_data(
    event: GenericEventContentWithTxRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    requests_hasher: &mut impl DepositRequestsSink,
    logger: &mut impl Logger,
) -> Result<(), ()> {
    let data = event.data;
    if data.len() != 576 {
        return Err(());
    }
    let mut chunks = data.as_chunks::<32>().0.iter();
    validate_u16_at_most(chunks.next().unwrap(), 160)?;
    validate_u16_at_most(chunks.next().unwrap(), 256)?;
    validate_u16_at_most(chunks.next().unwrap(), 320)?;
    validate_u16_at_most(chunks.next().unwrap(), 384)?;
    validate_u16_at_most(chunks.next().unwrap(), 512)?;
    drop(chunks);

    validate_u16_at_most(data[160..192].try_into().unwrap(), 48)?;
    validate_u16_at_most(data[256..288].try_into().unwrap(), 32)?;
    validate_u16_at_most(data[320..352].try_into().unwrap(), 8)?;
    validate_u16_at_most(data[384..416].try_into().unwrap(), 96)?;
    validate_u16_at_most(data[512..544].try_into().unwrap(), 8)?;

    logger_log!(logger, "Processing EIP-6110 deposit event with:");

    logger_log!(logger, "\nPubkey = ");
    let pubkey = &data[192..][..48];
    let _ = logger.log_data(pubkey.iter().copied());
    requests_hasher.write(pubkey);

    logger_log!(logger, "\nWithdrawal credentials = ");
    let withdrawal_credentials = &data[288..][..32];
    let _ = logger.log_data(withdrawal_credentials.iter().copied());
    requests_hasher.write(withdrawal_credentials);

    let amount = &data[352..][..8];
    logger_log!(
        logger,
        "\nAmount = {}",
        u64::from_le_bytes(amount.try_into().unwrap())
    );
    requests_hasher.write(amount);

    logger_log!(logger, "\nSignature = ");
    let signature = &data[416..][..96];
    let _ = logger.log_data(signature.iter().copied());
    requests_hasher.write(signature);

    let index = &data[544..][..8];
    logger_log!(
        logger,
        "\nIndex = {}\n",
        u64::from_le_bytes(index.try_into().unwrap())
    );
    requests_hasher.write(index);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_contract_addresses() {
        let hex = |address: B160| format!("{:040x}", address.as_uint());
        assert_eq!(
            hex(deposit_contract_address(1)),
            "00000000219ab540356cbb839cbe05303d7705fa"
        );
        assert_eq!(
            hex(deposit_contract_address(560048)),
            "00000000219ab540356cbb839cbe05303d7705fa"
        );
        assert_eq!(
            hex(deposit_contract_address(11155111)),
            "7f02c3e3c98b133055b8b348b2ac625669ed295d"
        );
        assert_eq!(
            hex(deposit_contract_address(17000)),
            "4242424242424242424242424242424242424242"
        );
    }
}
