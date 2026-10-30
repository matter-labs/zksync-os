use super::*;
use crypto::hints::MemoryOracle;
use crypto::precompiles::{bls12_381_g1_msm, bls12_381_g2_msm};
use zk_ee::oracle::IOOracle;
use zk_ee::system::logger::Logger;
use zk_ee::system::{Resources, SystemFunctionExt};

pub const BLS12_381_G1_MSM_PER_POINT_GAS: u64 = 12000;
pub const BLS12_381_G2_MSM_PER_POINT_GAS: u64 = 22500;

pub const DISCOUNT_DENOMINATOR: u16 = 1000;

// k -> discount factor
pub const DISCOUNT_TABLE_G1_MSM: [u16; 128] = [
    1000, 949, 848, 797, 764, 750, 738, 728, 719, 712, 705, 698, 692, 687, 682, 677, 673, 669, 665,
    661, 658, 654, 651, 648, 645, 642, 640, 637, 635, 632, 630, 627, 625, 623, 621, 619, 617, 615,
    613, 611, 609, 608, 606, 604, 603, 601, 599, 598, 596, 595, 593, 592, 591, 589, 588, 586, 585,
    584, 582, 581, 580, 579, 577, 576, 575, 574, 573, 572, 570, 569, 568, 567, 566, 565, 564, 563,
    562, 561, 560, 559, 558, 557, 556, 555, 554, 553, 552, 551, 550, 549, 548, 547, 547, 546, 545,
    544, 543, 542, 541, 540, 540, 539, 538, 537, 536, 536, 535, 534, 533, 532, 532, 531, 530, 529,
    528, 528, 527, 526, 525, 525, 524, 523, 522, 522, 521, 520, 520, 519,
];
// k -> discount factor
pub const DISCOUNT_TABLE_G2_MSM: [u16; 128] = [
    1000, 1000, 923, 884, 855, 832, 812, 796, 782, 770, 759, 749, 740, 732, 724, 717, 711, 704,
    699, 693, 688, 683, 679, 674, 670, 666, 663, 659, 655, 652, 649, 646, 643, 640, 637, 634, 632,
    629, 627, 624, 622, 620, 618, 615, 613, 611, 609, 607, 606, 604, 602, 600, 598, 597, 595, 593,
    592, 590, 589, 587, 586, 584, 583, 582, 580, 579, 578, 576, 575, 574, 573, 571, 570, 569, 568,
    567, 566, 565, 563, 562, 561, 560, 559, 558, 557, 556, 555, 554, 553, 552, 552, 551, 550, 549,
    548, 547, 546, 545, 545, 544, 543, 542, 541, 541, 540, 539, 538, 537, 537, 536, 535, 535, 534,
    533, 532, 532, 531, 530, 530, 529, 528, 528, 527, 526, 526, 525, 524, 524,
];

fn compute_cost(
    input_len: usize,
    pair_len: usize,
    per_pair_cost: u64,
    discounts_table: &[u16; 128],
) -> u64 {
    let num_pairs = input_len / pair_len;
    if num_pairs == 0 {
        return 0;
    }
    let discount = if num_pairs > 128 {
        discounts_table[127]
    } else {
        discounts_table[num_pairs - 1]
    };

    (per_pair_cost * num_pairs as u64) * (discount as u64) / (DISCOUNT_DENOMINATOR as u64)
}

pub struct Bls12381G1MSMPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G1MSMPrecompile<USE_ADVICE>
{
    fn execute<
        O: IOOracle,
        L: Logger,
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_g1_msm", resources, {
            bls12_381_g1_msm_as_system_function_inner(
                input,
                output,
                resources,
                allocator,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_g1_msm_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: MemoryOracle,
    A: core::alloc::Allocator + Clone,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    allocator: A,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.is_empty() || !input.len().is_multiple_of(G1_MSM_PAIR_LEN) {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost = compute_cost(
        input.len(),
        G1_MSM_PAIR_LEN,
        BLS12_381_G1_MSM_PER_POINT_GAS,
        &DISCOUNT_TABLE_G1_MSM,
    );
    let cost_gas = cost;
    let cost_native = compute_cost(
        input.len(),
        G1_MSM_PAIR_LEN,
        crate::cost_constants::BLS12_381_G1MSM_PER_POINT_NATIVE_COST,
        &DISCOUNT_TABLE_G1_MSM,
    );
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let result = bls12_381_g1_msm(input.as_chunks::<G1_MSM_PAIR_LEN>().0, allocator, oracle)
        .map_err(interface_error_of)?;

    write_g1(result, output)?;

    Ok(())
}

pub struct Bls12381G2MSMPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G2MSMPrecompile<USE_ADVICE>
{
    fn execute<
        O: IOOracle,
        L: Logger,
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_g2_msm", resources, {
            bls12_381_g2_msm_as_system_function_inner(
                input,
                output,
                resources,
                allocator,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_g2_msm_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: MemoryOracle,
    A: core::alloc::Allocator + Clone,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    allocator: A,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.is_empty() || !input.len().is_multiple_of(G2_MSM_PAIR_LEN) {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost = compute_cost(
        input.len(),
        G2_MSM_PAIR_LEN,
        BLS12_381_G2_MSM_PER_POINT_GAS,
        &DISCOUNT_TABLE_G2_MSM,
    );
    let cost_gas = cost;
    let cost_native = compute_cost(
        input.len(),
        G2_MSM_PAIR_LEN,
        crate::cost_constants::BLS12_381_G2MSM_PER_POINT_NATIVE_COST,
        &DISCOUNT_TABLE_G2_MSM,
    );
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let result = bls12_381_g2_msm(input.as_chunks::<G2_MSM_PAIR_LEN>().0, allocator, oracle)
        .map_err(interface_error_of)?;

    write_g2(result, output)?;

    Ok(())
}

#[cfg(test)]
mod tests;
