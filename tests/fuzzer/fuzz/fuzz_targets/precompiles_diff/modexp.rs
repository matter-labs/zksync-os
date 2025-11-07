#![no_main]
#![feature(allocator_api)]

use libfuzzer_sys::fuzz_target;
use revm::primitives::U256;
use revm_precompile::modexp::berlin_run;
use arbitrary::{Arbitrary, Unstructured};
use zk_ee::system::Resource;
use basic_system_proving::system_functions::modexp::delegation::delegated_modexp_with_naive_advisor;
use zk_ee::system::base_system_functions::ModExpErrors;
use zk_ee::system::errors::subsystem::SubsystemError;
use basic_system::system_functions::modexp::ModExpImpl;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::{SystemFunction,SystemFunctionExt};
use zk_ee::reference_implementations::DecreasingNative;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum LenMode {
    Exact,
    OffByOneShort,
    OffByOneLong,
    MuchShorter,
    MuchLonger,
    Random,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ExpKind {
    Random,
    Zero,
    One,
    Two,
    Rsa,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ModKind {
    Random,
    Zero,
    One,
    PowerOfTwo,
    Odd,
}

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    // Lengths for base/exp/mod payloads
    #[arbitrary(with = len_gen)]
    bl: u16,
    #[arbitrary(with = len_gen)]
    el: u16,
    #[arbitrary(with = len_gen)]
    ml: u16,

    // Component kinds
    ek: ExpKind,
    mk: ModKind,

    // Component length mutation mode
    bl_lm: LenMode,
    el_lm: LenMode,
    ml_lm: LenMode,

    // Raw entropy to fill buffers
    base_seed: Vec<u8>,
    exp_seed:  Vec<u8>,
    mod_seed:  Vec<u8>,

    // Random lengths used when LenMode::Random
    bl_rand: u32,
    el_rand: u32,
    ml_rand: u32,

    // Mutate the input bytes by trimming length
    max_len: Option<u32>,
}

const MAX_COMPONENT_LEN: u32 = 64;
const MAX_DECL_LEN: u32 = 4 * MAX_COMPONENT_LEN;

fn len_gen(u: &mut Unstructured<'_>) -> arbitrary::Result<u16> {
    let pick: u8 = u.arbitrary()?;
    let v = match pick % 8 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 32,
        _ => u.int_in_range::<u16>(0..=MAX_COMPONENT_LEN as u16)?,
    };
    Ok(v)
}

fn shape_exponent(mut exp: Vec<u8>, ek: ExpKind) -> Vec<u8> {
    match ek {
        ExpKind::Random => exp,
        ExpKind::Zero => vec![0u8; exp.len()],
        ExpKind::One => {
            if exp.is_empty() { return exp; }
            let mut v = vec![0u8; exp.len()];
            *v.last_mut().unwrap() = 1;
            v
        }
        ExpKind::Two => {
            if exp.is_empty() { return exp; }
            let mut v = vec![0u8; exp.len()];
            *v.last_mut().unwrap() = 2;
            v
        }
        ExpKind::Rsa => {
            if exp.len() < 3 {
                return vec![1];
            }
            let mut v = vec![0u8; exp.len()];
            let n = v.len();
            v[n-3] = 0x01;
            v[n-1] = 0x01;
            v
        }
    }
}

fn shape_modulus(mut m: Vec<u8>, mk: ModKind) -> Vec<u8> {
    match mk {
        ModKind::Random => m,
        ModKind::Zero => vec![0u8; m.len()],
        ModKind::One => {
            if m.is_empty() { return m; }
            let mut v = vec![0u8; m.len()];
            *v.last_mut().unwrap() = 1;
            v
        }
        ModKind::PowerOfTwo => {
            if m.is_empty() { return m; }
            let mut v = vec![0u8; m.len()];
            let bit = 1u8 << (v.len() as u8 % 8);
            *v.last_mut().unwrap() = bit.max(1);
            v
        }
        ModKind::Odd => {
            if m.is_empty() { return m; }
            let mut v = m;
            *v.last_mut().unwrap() |= 1;
            v
        }
    }
}

fn fill_len(u: &mut Unstructured<'_>, len: usize, seed: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; len];
    let take = seed.len().min(len);
    v[..take].copy_from_slice(&seed[..take]);
    if take < len {
        let _ = u.fill_buffer(&mut v[take..]);
    }
    v
}

fn mutate_len(chosen: u32, mode: LenMode, rnd: u32) -> u32 {
    match mode {
        LenMode::Exact         => chosen,
        LenMode::OffByOneShort => chosen.saturating_sub(1),
        LenMode::OffByOneLong  => chosen.saturating_add(1).min(MAX_COMPONENT_LEN),
        LenMode::MuchShorter   => chosen / 2,
        LenMode::MuchLonger    => chosen.saturating_mul(2).min(MAX_COMPONENT_LEN),
        LenMode::Random        => rnd.min(MAX_DECL_LEN),
    }
}

#[inline]
fn be_u256(n: usize) -> [u8; 32] {
    U256::from(n).to_be_bytes()
}

#[inline]
fn normalize_be(s: &[u8]) -> &[u8] {
    let i = s.iter().position(|&b| b != 0).unwrap_or(s.len());
    &s[i..]
}

fn build_input_bytes(
    mut u: &mut Unstructured<'_>,
    i: &Input
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let bl = i.bl as usize;
    let el = i.el as usize;
    let ml = i.ml as usize;

    let base = fill_len(&mut u, bl, &i.base_seed);
    let exp_raw  = fill_len(&mut u, el, &i.exp_seed);
    let mod_raw  = fill_len(&mut u, ml, &i.mod_seed);

    let mbl = mutate_len(bl as u32, i.bl_lm, i.bl_rand);
    let mel = mutate_len(el as u32, i.el_lm, i.el_rand);
    let mml = mutate_len(ml as u32, i.ml_lm, i.ml_rand);

    let exp  = shape_exponent(exp_raw.clone(), i.ek);
    let modu = shape_modulus(mod_raw.clone(), i.mk);

    let mut out = Vec::with_capacity(96 + bl + el + ml);
    out.extend_from_slice(&be_u256(mbl as usize));
    out.extend_from_slice(&be_u256(mel as usize));
    out.extend_from_slice(&be_u256(mml as usize));
    out.extend_from_slice(&base);
    out.extend_from_slice(&exp);
    out.extend_from_slice(&modu);

    if let Some(t) = i.max_len {
        let t = t as usize;
        if t < out.len() { out.truncate(t); }
    }
    (out, base, exp_raw, mod_raw)
}

fn fuzz(data: &[u8]) {
    let mut u = Unstructured::new(data);
    let input = match Input::arbitrary(&mut u) {
        Ok(x) => x,
        Err(_) => return,
    };

    let (in_bytes, base, exp, modu) = build_input_bytes(&mut u, &input);

    let mut dst1 = Vec::new();

    let r_reth = berlin_run(&in_bytes, u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);
    let reth_out = r_reth.unwrap().bytes.to_vec();

    let r1 = modexp_forward(&in_bytes, &mut dst1);
    let r1_ok = r1.is_ok();

    assert!(!(reth_ok ^ r1_ok), "forward <> reth status mismatch");
    assert_eq!(dst1, reth_out, "forward <> reth bytes mismatch");

    let res_delegated = delegated_modexp_with_naive_advisor(&base, &exp, &modu);
    let res_forward = modexp::modexp(&base, &exp, &modu, std::alloc::Global);
    assert_eq!(
        normalize_be(&res_delegated),
        normalize_be(&res_forward),
        "forward <> proving bytes mismatch"
    );
}

pub fn modexp_forward(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<ModExpErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    ModExpImpl::execute(
        &src,
        dst,
        &mut resource,
        &mut DummyOracle {},
        &mut zk_ee::system::NullLogger,
        allocator,
    )
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});

struct DummyOracle {}

impl zk_ee::oracle::IOOracle for DummyOracle {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<'a, I: zk_ee::oracle::usize_serialization::UsizeSerializable + zk_ee::oracle::usize_serialization::UsizeDeserializable>(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        unreachable!("oracle should not be consulted on native targets");
    }
}