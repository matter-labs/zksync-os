use crate::cost_constants::{POINT_EVALUATION_COST_ERGS, POINT_EVALUATION_NATIVE_COST};
use crypto::ark_ec::pairing::Pairing;
use crypto::ark_ec::{AdditiveGroup, AffineRepr, CurveGroup};
use crypto::ark_ff::{Field, PrimeField};
use zk_ee::common_traits::TryExtend;
use zk_ee::interface_error;
use zk_ee::out_of_return_memory;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::*;

pub type KzgScalar = <crypto::bls12_381::Fr as PrimeField>::BigInt;

///
/// Point evaluation system function implementation.
///
pub struct PointEvaluationImpl;

impl<R: Resources> SystemFunction<R, PointEvaluationErrors> for PointEvaluationImpl {
    /// Returns `OutOfGas` if not enough resources provided, resources may be not touched.
    ///
    /// Returns `InvalidInputSize` error if `input_len` != 192,
    /// `InvalidPoint` if commitment or proof point encoded incorrectly,
    /// `InvalidScalar` if `z` or `y` scalars encoded incorrectly,
    /// `InvalidVersionedHash` if versioned hash doesn't correspond to the commitment,
    /// `PairingMismatch` if kzg proof pairing check failed.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<PointEvaluationErrors>> {
        cycle_marker::wrap_with_resources!("point_evaluation", resources, {
            point_evaluation_as_system_function_inner(input, output, resources)
        })
    }
}

pub const POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE: [u8; 64] = const {
    // u256_be(4096) || u256_be(BLS12-381 Fr characteristic)
    let Ok(res) = const_hex::const_decode_to_array(
        b"000000000000000000000000000000000000000000000000000000000000100073eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001"
    ) else {
        panic!()
    };

    res
};

pub fn versioned_hash_for_kzg(data: &[u8]) -> [u8; 32] {
    use crypto::sha256::Digest;
    let mut hash: [u8; 32] = crypto::sha256::Sha256::digest(data).into();
    hash[0] = VERSIONED_HASH_VERSION_KZG;

    hash
}

// We do not need internal representation, just canonical scalar
fn parse_scalar(input: &[u8; 32]) -> Result<KzgScalar, ()> {
    // Arkworks has strange format for integer serialization, so we do manually
    let result = crypto::parse_u256_be(input);
    if result >= crypto::bls12_381::Fr::MODULUS {
        Err(())
    } else {
        Ok(result)
    }
}

pub fn parse_g1_compressed(input: &[u8]) -> Result<crypto::bls12_381::G1Affine, ()> {
    // format coincides with one defined in ZCash/Arkworks
    use crypto::ark_serialize::CanonicalDeserialize;
    crypto::bls12_381::G1Affine::deserialize_compressed(input).map_err(|_| ())
}

// Precomputed Miller-loop ell-coeffs for tau*G2 (the BLS12-381 KZG trusted-setup
// point). The runtime conversion G2_BY_TAU_POINT.into() is ~5% of the
// point_evaluation cycle budget; this const moves that work to compile time.
// Two variants because the underlying Fq representation differs between the
// host build (6 u64 limbs, ark_bls12_381::Fq) and the proving build (8 u64
// limbs, ark_ff_delegation::Fp512 padded for delegation alignment).
#[cfg(not(feature = "proving"))]
const PREPARED_G2_BY_TAU:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared =
    crypto::bls12_381::curves::G2PreparedNoAlloc {
        ell_coeffs: [
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11925556972146567016,
                            16334858588728237790,
                            13071673221206363140,
                            4783444548443525780,
                            9297645921556835221,
                            798161992579604048,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7376160959796979546,
                            11460070759966881008,
                            10492686378742475312,
                            18013447854818146093,
                            15456241908358726572,
                            635138503118167767,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6114330492017684964,
                            8790753945787631157,
                            9109704048932222599,
                            16063852289741162872,
                            8377119718655554252,
                            1832772230590679437,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2983431651187967594,
                            9420143361936114962,
                            18125422127679645388,
                            3852314742599414363,
                            11643442536871777939,
                            1101294227671161536,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2294565932855004968,
                            8712853091106804624,
                            314325015324907846,
                            2897586001882532972,
                            15351753497310318789,
                            240094519746241833,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14609329544893013314,
                            15552738521130673666,
                            365644675938667335,
                            2391623741463743042,
                            9436071031795180168,
                            1036865072251795898,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7723343621718374829,
                            10690066398185861339,
                            2494964356889654226,
                            5255070301620754644,
                            1712667504987536399,
                            1745829113203828461,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4632718803837617420,
                            2504526965494693108,
                            15967090077267071861,
                            16278734691332148616,
                            12688970467601551992,
                            1013120441263378432,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5851202926675066597,
                            8625259415380560030,
                            5775474712845252843,
                            2990323860552012375,
                            9396062353727999757,
                            331892216918616288,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14586558243183611968,
                            17524543797977087142,
                            147083720772250940,
                            2038665296479797963,
                            14487328477006156279,
                            1448680451683251830,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3706365046372996849,
                            15243608661071384470,
                            14635776042563515120,
                            1851701167321031407,
                            8820890296379460833,
                            522151934952124336,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13019291832814359394,
                            12555641349657668780,
                            12415858299046873171,
                            15175952227944801875,
                            3992626979404397404,
                            844381687493518199,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11126498769479087478,
                            8805449472259600475,
                            1510354522944665755,
                            17808941009963723098,
                            11328606327463895348,
                            1596419105081771476,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5867919863234581894,
                            3476735551422891727,
                            10424079148189177212,
                            14369416176525130752,
                            868294977286103665,
                            122208983685177601,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7572668876069719148,
                            2330973559498381271,
                            13545139541393905918,
                            5725567809532437965,
                            16621707573129851746,
                            711162580003806297,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3843478665137236538,
                            3675084615728424722,
                            8564678526025159121,
                            16999961491484719438,
                            3273021535376930499,
                            974015536801244301,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9123377991879708471,
                            9616151698894714241,
                            12313169436103337212,
                            947127648629502427,
                            8690483684094594957,
                            1542679077341447563,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2130326851293644007,
                            1864996701564738455,
                            8914823939469460989,
                            15925830120212279554,
                            14989069730935700779,
                            827449668066551955,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16458892502348306983,
                            7137808065319900475,
                            13439136086791834271,
                            724299830792069365,
                            10224640249104169120,
                            546184773436092812,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4518717432653601243,
                            14492929032961939219,
                            18284070141478398033,
                            17252523880739695892,
                            2205770993354539260,
                            231153303040177112,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4955655540166178043,
                            14675133973170698999,
                            17596569191572679049,
                            10600808513070253868,
                            8186661412975786009,
                            948332965792059826,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4852844119935032131,
                            9935075551820148163,
                            5867351718971333704,
                            3936086317971208791,
                            18275735868563066676,
                            545789397746215247,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2447373873837925436,
                            1234312995491994788,
                            14292493294204475088,
                            3902654513106541673,
                            16388187768823863606,
                            1592570474472269559,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7744705746090013493,
                            8722998669371208313,
                            2955259259686435374,
                            18404147149789984156,
                            7510161283664275675,
                            1776633548580548608,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10644212501011758444,
                            18174418352105425130,
                            9026101209493494077,
                            9381211259926198700,
                            2441433427485537878,
                            26062380204325124,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17085134777386197337,
                            15881017453795797895,
                            2240648804981151702,
                            18260351677438622324,
                            15947898179225436361,
                            1315838886102656678,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11257329424145065763,
                            13690459336946258764,
                            6695064597969666423,
                            12778832381962286288,
                            10515171397961685936,
                            381672177004480860,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4611385146047256996,
                            4052754049362437872,
                            16989457981807047355,
                            3936797166479366182,
                            5192545408015349402,
                            1435774512547819891,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7084047558779944701,
                            9626909257903347559,
                            9250987082530970809,
                            8801078100550715265,
                            9534484070328760150,
                            493548359054297234,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7254161078027797018,
                            13369984146035489217,
                            9616842106413199363,
                            2184292869118097084,
                            8607566966869653221,
                            568750848593883747,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15901971784311991931,
                            5756597955549135338,
                            9700260701042775840,
                            12741574641154939916,
                            10178634458464376840,
                            220826804012950222,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9468702904214068391,
                            6239078212046684928,
                            7161867955362909256,
                            17785989680722892250,
                            9865574747861867555,
                            1467277188581815213,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9958127659275201304,
                            15360861477991874964,
                            10123924482064440108,
                            17157333375013097794,
                            17937608932770869332,
                            1867001237015344352,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13968176466994135366,
                            5093473826063865659,
                            16513647053870722007,
                            5475981439086306460,
                            4545503042193042684,
                            1357221527254200606,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12930537219024873660,
                            7410900145155317948,
                            13282912267232063476,
                            1255727418175152446,
                            10175289421165377116,
                            860359874967180614,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5740162068019236338,
                            6205377666921376127,
                            4456010104034385689,
                            13012099264919685927,
                            8641203089394412342,
                            1372006608689547444,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9365256325042561559,
                            13473824556215842601,
                            7463735299289272782,
                            2119773591165422221,
                            11205871340214357749,
                            1137077072718624618,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4676781693728288546,
                            14647568887065035792,
                            14132140483406663355,
                            7825231661290057599,
                            819261293088754039,
                            864210717551415263,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6896426462585699730,
                            7279525779059087447,
                            2089668438770731545,
                            18430234912904148214,
                            18164228272439466795,
                            589025176250196430,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8066904534413120515,
                            11107773763295503095,
                            3165373730399334189,
                            16106587365370867847,
                            17488310015311944708,
                            40306167018761091,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14429102156842674961,
                            13739501427883300657,
                            11807842210337743782,
                            4483918912619648466,
                            17108837848864884999,
                            909446742166143374,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11461109822967215999,
                            3439733053692423973,
                            4153822966198804506,
                            2407657612757065990,
                            15215346696393807937,
                            809335811619583418,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12577792596589933665,
                            3636397963681197451,
                            8979642198983157713,
                            6467061689619318210,
                            10925018415394213633,
                            1019532484417326479,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2231885201267778989,
                            13406771189559235338,
                            18072141183735622269,
                            1487129837709726924,
                            10875962354990901662,
                            1209010187589422702,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7984821080738966426,
                            5719856785783239740,
                            8860597701099210262,
                            4920863900918890593,
                            14258364950512669019,
                            125340966872087079,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5128805738020324751,
                            16476916576174314996,
                            1442452774975542713,
                            6368352327765800700,
                            18094350238656923546,
                            427969321588608447,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11792183570448155801,
                            17848119796283727434,
                            11155557430152273618,
                            3334742214622074146,
                            15000064684649871598,
                            1182625415207740038,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11111526835812777551,
                            16380875126660333655,
                            6014265187412799351,
                            5315157657971051331,
                            6220127417582054131,
                            415169338083420016,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8890457561652514281,
                            16336197849867252356,
                            10696724146064442375,
                            2645705216965123909,
                            10223881345641528178,
                            1222543204380134143,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7191170992571602861,
                            12213494161358985122,
                            9866037464796415042,
                            3705934112773374605,
                            3016385753016521557,
                            385921809431968270,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16482954637898166134,
                            1141341707790941028,
                            9602402554324474750,
                            10185604952446518074,
                            9633585981084043879,
                            518819547267629340,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17965195730078322788,
                            12392394544316727267,
                            15032903271394778541,
                            4546801778556919804,
                            6261655323304072004,
                            673135665866127203,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8071415009007423084,
                            14216763050342008883,
                            4506692598771651778,
                            7528017385186778962,
                            13719211296604378125,
                            393738973054281436,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16427720866586134236,
                            15212373498309329420,
                            3643649688635862081,
                            8594002229243870298,
                            164069860849775524,
                            726311772985105951,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13646025530356321354,
                            4805203030097177961,
                            4477965191493873002,
                            2313118110565637423,
                            3761505352942693568,
                            1230120882559557730,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8998926740203506483,
                            15880038642362105915,
                            6268407000256145714,
                            5191782250763503027,
                            14237573602258474383,
                            484639140884232455,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9943111243254682744,
                            18318472255882475324,
                            13571450170926787394,
                            14853165478676020188,
                            17950516708013693330,
                            163001116655971720,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4304890601088043684,
                            13138991095591798559,
                            7377387516030395154,
                            10384181943284817078,
                            5013288565839709822,
                            287515778618826920,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16789189218846744468,
                            17591211352416738358,
                            7071502329548331428,
                            18001296795702038384,
                            10542670614720193692,
                            1812313695472264355,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1347313782638110912,
                            15563827670095966574,
                            8528222503263621715,
                            13798604610752650701,
                            6778837199264737085,
                            1011848413758700356,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            734817677876110503,
                            5854585256264582086,
                            334138430196872045,
                            6155601865791876810,
                            2652393225856490844,
                            375815759663713446,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16002147226574056424,
                            8053237224882802450,
                            3236670577204687712,
                            10210865371723962891,
                            13459509575287652401,
                            182570599403682143,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15898094034384353080,
                            6828080128498380283,
                            13545830842118254139,
                            12068103860928432101,
                            14173220372352601116,
                            1694111816045768161,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14366471346686951181,
                            4287153257986750142,
                            7986456117959934364,
                            9743385975877592747,
                            3388044839083390602,
                            96118368847976389,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13503863679036200327,
                            16547648953825637398,
                            6554048617768648459,
                            2144949389763425056,
                            11594343720407951953,
                            776898610673756742,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14655058322303047459,
                            12663630452867052329,
                            1607552166811780890,
                            13199456528985121131,
                            16009453028392312220,
                            1583990678679838955,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3255682889253732767,
                            16205722982681261314,
                            16685470358175725853,
                            8135237604720195462,
                            979465975292346401,
                            149726446797001823,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6317867241370351506,
                            6989105149313190394,
                            254401568534383422,
                            16085593705782647103,
                            2441584840233537761,
                            1470620779431431230,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10654996161109209561,
                            4913008374494045075,
                            830110283099472914,
                            237822853392351616,
                            6994949963944583279,
                            1508575888759217721,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            713338152734649787,
                            7626101469040709255,
                            2357858527393622267,
                            12851775181363330680,
                            2411035513330019007,
                            1536818054420239111,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11674810961136445161,
                            5139751116306511815,
                            17279289070027552053,
                            15222118387818977486,
                            2494153841375211675,
                            1154756602320974811,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15769897178124930931,
                            5545549438802126241,
                            12879995836034519648,
                            138496503735704387,
                            3701535853118223166,
                            1505742657035701761,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6049997757183208497,
                            6671192561513207057,
                            6156609390514919058,
                            6661866985712923436,
                            2699320262523097042,
                            1318394725772967427,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15757531954383357344,
                            6452627642910992532,
                            10533357798412181683,
                            1683449058595576534,
                            8769742629188678161,
                            299304671647571100,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4793668539016172811,
                            2415615960148147973,
                            10869456137131108372,
                            3801431787896192347,
                            6404957811218845892,
                            357451575432191537,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5244776855537895858,
                            3559593276311949714,
                            3398526756267480135,
                            174704202399055038,
                            8893836775364690083,
                            412040673102691610,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6743700563728528136,
                            15729429880143749511,
                            15261868516776415466,
                            2739872274899953210,
                            13667849105787592420,
                            1548726575338815177,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7743286655838215909,
                            18287732019564225306,
                            2000839571169973336,
                            10226041126781700750,
                            16519421059711517496,
                            1222945639384908193,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15092742031842063585,
                            16088538989226631047,
                            6476916101235260243,
                            1956121245703582516,
                            10272058724981952145,
                            430125553764659967,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9543467134579083332,
                            10277368540605392780,
                            6155278220592138770,
                            4384131266727665742,
                            16502566287356048644,
                            932245934753468159,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            728277409143107400,
                            8315848048817740892,
                            10779485302711982151,
                            7673811698248762249,
                            10756447624130217429,
                            1780297371298501716,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3707189436230605035,
                            7092300099250693560,
                            10053211640856464379,
                            4451179339382987962,
                            18188183595215930878,
                            965629733302617885,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9643260728059233000,
                            17698395208997584959,
                            17651180530927236801,
                            16495451099362550455,
                            16437577553700238695,
                            905809579651933199,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10762806880803719376,
                            9665840686566496027,
                            10378124170845148828,
                            14108407175350111053,
                            12466649603547076768,
                            74187771592352373,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            894390426989420211,
                            4915756262105781167,
                            17773407678371676077,
                            3677094084260779856,
                            12269604497980481256,
                            1673756327118024326,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15419269349106340257,
                            4590684996808076231,
                            7902584297827962278,
                            2298794303208685784,
                            2915523884566138853,
                            975518849555379370,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14729264152624729315,
                            13842421227595851236,
                            12149390552619137911,
                            2932442935058299725,
                            2572471806680179211,
                            1301179682580977864,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3602320274031637123,
                            4549120751730316390,
                            4577751040996779773,
                            5666007386798490733,
                            12701363054406401199,
                            950346392340607557,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3757798579998322393,
                            2687057174873694421,
                            8106286444258661411,
                            9565319758626399782,
                            7101854685347134470,
                            1066341188734641281,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9411974306085432162,
                            1876364169309533710,
                            9297244742336531561,
                            11790550411004924425,
                            110420279322888889,
                            1574835900168092609,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10135465715582669924,
                            5888745249316319324,
                            12574012923763850223,
                            15394457956171293448,
                            13487207971264497529,
                            323366419069305926,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11808084758017513997,
                            17542145097896666197,
                            15254852154039001416,
                            11172876529594247583,
                            82804573089129725,
                            1100393435994641122,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15792334261488091260,
                            15664370882096496682,
                            4328129064570109832,
                            13746153841324600255,
                            14622444386512042336,
                            1542240897425236768,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7114969781701025149,
                            15209578178090951305,
                            11910278753167397379,
                            7659299309910282074,
                            1049834275295697081,
                            105773786702422980,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16333009619424460127,
                            18402257492326244105,
                            15722177048311106963,
                            9678282164515557545,
                            5656878813561048989,
                            1711388897655013024,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14419915349325497910,
                            7625217460656094447,
                            12953389866437708099,
                            9737339247479167946,
                            15844059670003310514,
                            466280760752221564,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1809434212726303843,
                            3323875763088362209,
                            919357733044024897,
                            17249207329359921221,
                            4596458360269380961,
                            1609225490152881277,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15517659136916550441,
                            5696113384295266439,
                            6857964258985848377,
                            809627535603824413,
                            15000318224151708581,
                            749390806081368898,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9332939628460013116,
                            1417093594997992969,
                            4551946947395501966,
                            17396404858012074431,
                            3029564293180914811,
                            46078006863161939,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12117462210517030216,
                            13929813358380075838,
                            1596015817236698829,
                            10717529643351790357,
                            14283104359177705962,
                            327742863933271186,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16115216343004804320,
                            12233630647181341097,
                            3367436550829686603,
                            14623987983350015804,
                            4059474184591876732,
                            1377949123976948430,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9451746805276553127,
                            16609774532777155534,
                            8768608242852924296,
                            14784492530604263809,
                            9741514413170068570,
                            1659181642242133777,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9537393270191716250,
                            1397355729486116855,
                            7125016768837026989,
                            5210859394566944699,
                            12704010402184440659,
                            210085248337053704,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13893676821391894812,
                            16749098412694357001,
                            14081751926009191920,
                            13201158536951229592,
                            12094178806090527711,
                            517437589336507406,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            378743727516648407,
                            17648351188309641422,
                            12975926215877150136,
                            7573815506508353945,
                            7318633672851800605,
                            838708338386916095,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8858671833410818061,
                            15734057103885603263,
                            8174559681880847722,
                            11826193153290957965,
                            10993918273381808872,
                            1176305779383038052,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12154549900415474711,
                            1110750699660043559,
                            8573796403527537913,
                            12451163387896206633,
                            3909307498221762975,
                            516122164281208898,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5651259372124568485,
                            471466572676895219,
                            9302049074256594549,
                            10759297508168777835,
                            13934905161862610669,
                            337339592633982467,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6528374147929617120,
                            1769785864558557995,
                            15932425088751386866,
                            4681122383350660232,
                            4075851899407991734,
                            1482977009103686343,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5183286319329772906,
                            176418030640625046,
                            1501009277750541029,
                            5089933868902208430,
                            15072711914685130685,
                            1441588862219951874,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14069476943542182780,
                            17609119497167353850,
                            2832964331925873964,
                            16495327919870442377,
                            12030482305250964049,
                            1675925929427531265,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            529291349917868811,
                            12470120313203347870,
                            8880490905569495252,
                            4110309824621609055,
                            10436410552337467311,
                            944379703348586496,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10556168310153542276,
                            6113228765361567268,
                            16130009544633491333,
                            14648361234202272355,
                            14042717501470532053,
                            996492786119551541,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9512523011052657084,
                            12444813521601129359,
                            4700186509217862140,
                            11374964666032411519,
                            56099546866910421,
                            323145433017646840,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16962970681580352142,
                            16471161318579800395,
                            1018964678812838772,
                            8682068031693775993,
                            3448400605368972042,
                            640697735462612161,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3560619757227365473,
                            9239243271928898510,
                            16851756891694495662,
                            5468673555917513062,
                            8700786422073143531,
                            839552925230056061,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9131748704496754548,
                            7652964773799709102,
                            938714332355521978,
                            6746697541081000237,
                            11803113115771606090,
                            469738253972182323,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13063655576566652596,
                            14718358522606014344,
                            11006294343158719876,
                            5070073490566833786,
                            4243541238020681130,
                            1421320762938252499,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16616633371891427580,
                            13926270884137701317,
                            8865780346621894920,
                            17045906590149271031,
                            4004874915172225880,
                            229965255933045386,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15727020333284892723,
                            1408618811925835037,
                            2160400779427370507,
                            9084422006724154754,
                            7465610955758401799,
                            1111916939878300581,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4231063385204006468,
                            6692286242515971765,
                            12840106154291826465,
                            6308434671736959326,
                            7099587447396526880,
                            1028029982669721856,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13571610410760125990,
                            14018032570565574502,
                            3821321449206415206,
                            12045681115513281696,
                            11715798535000231907,
                            596498659151257747,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3161165881830155608,
                            14621547560863010622,
                            586642549319917417,
                            12621313459726468753,
                            15611155550499377360,
                            1472073315688039675,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17123945709495407279,
                            983080166914913308,
                            10548359586401616571,
                            199684120836799127,
                            16260990210560624856,
                            1687917881382393005,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15361311713497454850,
                            8791504742806747856,
                            11841949218505484641,
                            6857553999143553898,
                            12614022801591747341,
                            1159458459360487396,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1746232706613982183,
                            9321968290137173626,
                            3147531215437443169,
                            7495822192674426705,
                            5085699126872187752,
                            1202378474173246663,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14109292319323709941,
                            8837164345633341015,
                            16085914088913335172,
                            10606145229660013815,
                            13136333598762987795,
                            117030878975697651,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            206015072447997228,
                            6678084232404658769,
                            15512433192324750573,
                            1947246275073001113,
                            11868218730342628702,
                            898343431753349968,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13243949749461136856,
                            1456768495750214448,
                            5392853117218405090,
                            5149890048152948429,
                            11721680857179789157,
                            1635388107631303391,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3973267250237951866,
                            5441388041636904961,
                            2688227262880175287,
                            15054480845827152347,
                            17247725248213860602,
                            352239917168993980,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1207728109026388598,
                            14476710012185150693,
                            2246780294016035650,
                            8040152720630273383,
                            15441010369445169050,
                            1561817579327666993,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8938946251427727247,
                            16597471624015905641,
                            14777521400799040529,
                            7389997781945013160,
                            10625730081073723070,
                            449069910363110226,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14558829160461254718,
                            14079950709367841150,
                            1065784927039347426,
                            10196180361325571431,
                            14632448736227742601,
                            551543384160592678,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16401361693757855006,
                            9111384123277549069,
                            5415179535502693821,
                            16761090842230868438,
                            7065149778710016402,
                            326700431859901376,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5161187698881313462,
                            15407135823601577666,
                            9270750016512848597,
                            10831961254719748753,
                            16235285677107722958,
                            1314353825375987945,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15286911181595032076,
                            11405526654630506187,
                            1603227792067920441,
                            3208894901895109568,
                            15495411639620801511,
                            1040470953499882009,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5468435024657977853,
                            13247469680060670109,
                            10385145302207955786,
                            16230964911083068008,
                            5017235141787163367,
                            1481852877526385886,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10660705633345303159,
                            1994969116635853755,
                            13562983942719930317,
                            9645646741213478491,
                            18049781344630597594,
                            452266371163308633,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7571661775478230929,
                            14551836743519661867,
                            4622799633818304510,
                            9245186152632899517,
                            11012794527344968194,
                            512736858921116536,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13481694380377457528,
                            16685885696606617168,
                            14742797850045547646,
                            2126962111213475867,
                            17734367877784460856,
                            887544097761840258,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17525520458437974424,
                            7911687325048473898,
                            417532147825699589,
                            15677672547644698745,
                            14087139669875924684,
                            1752586674539455933,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13977551202433501289,
                            12984642978132260751,
                            1350456832944518575,
                            12097599113612379988,
                            16803922891152180597,
                            273066620017501672,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6297405972153664055,
                            12902774946485910994,
                            13138230030164868088,
                            10780926650763746832,
                            6797769461669410875,
                            1238457993593915504,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16904503979535321440,
                            1408910392891853049,
                            6037142385028834161,
                            3431984774353581496,
                            9599630797197640472,
                            1412728157748839413,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7038865756066255017,
                            12299452990964825911,
                            17010321623530923447,
                            8786134161077542211,
                            18365359851027786169,
                            709647176188727409,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5290708680087885228,
                            5983300260214320404,
                            2354736656674922373,
                            8154193338493432181,
                            7352054854753352652,
                            807112183606431831,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4527180954955170283,
                            15317809688666540922,
                            5248754679387965240,
                            3078313981575136394,
                            4723741570854940471,
                            1683438614474169462,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1902414411508588832,
                            2530297999347974788,
                            14749003414645755159,
                            15475221928703011698,
                            10663716132128396108,
                            1297988066892568624,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6614613281375555752,
                            1282743761923657323,
                            7043923039857527261,
                            12732999155888226651,
                            12154503124826837585,
                            1209217381524218249,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16549819809840464555,
                            2843763347348832098,
                            2813125057126697496,
                            15720883435851133298,
                            18271778644183848559,
                            1204286554256504400,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7109130842929914832,
                            1803790392705807387,
                            1271608437981737847,
                            15682633792013818447,
                            10287229682566749616,
                            665117965630175262,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16677421006981650064,
                            1353809749177007179,
                            2823006776672705092,
                            11997889663167429847,
                            13645198407115909447,
                            579219906856826597,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3295392738214448860,
                            602025461241339599,
                            10529090071170925240,
                            11014396106566593324,
                            3281859532403750380,
                            1023686700067908406,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4333070397982432537,
                            15271261348859370469,
                            367511550370076042,
                            1595173370854984586,
                            14823737049126087984,
                            123706930642833768,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17067910549732339510,
                            2675228548236584427,
                            3366984011790424261,
                            18293407976473074349,
                            4277244937192700671,
                            1360653507603512731,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10502157757803091650,
                            1136729077429196483,
                            2817952920149324208,
                            12462078344733826288,
                            16855115728600786653,
                            476775998752184792,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12466547238993612874,
                            14230232247552762226,
                            11032148330356353138,
                            17388341393312774378,
                            15962291913680472096,
                            769364543640477477,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9156594793750494078,
                            720099626785901314,
                            12159788476925010600,
                            8164616588957315705,
                            17483683783295944096,
                            1128501590156391302,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13568472953403430530,
                            10425567279225319957,
                            2858702026807031377,
                            17794196568706119115,
                            15029146524323265425,
                            1206824831631949963,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6027184781054211025,
                            3567124591125006918,
                            1455754456895461908,
                            11267114916138278309,
                            15366536353659424417,
                            894651456579924381,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9882683328822627495,
                            6233588290637950484,
                            12394612088406285135,
                            13269965119810265843,
                            2988463812925714940,
                            466135280360634319,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4671417466836588458,
                            12446023543066084485,
                            5835227541829011504,
                            4541155711362429371,
                            15011857688172476804,
                            111210992209802193,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10748952635887004411,
                            10593762535732814301,
                            3870727810516816661,
                            15935339202481202298,
                            482208230438523923,
                            1537447471243287219,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16478427816050564964,
                            13951178306110306370,
                            7821910697691839364,
                            15627073127185310532,
                            18133150159137840830,
                            320598979090218648,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5655057475721088625,
                            7152016465095975941,
                            5136724542486956931,
                            11510652293959864250,
                            12352084350816523931,
                            86459393876730119,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8167098127403838789,
                            10009544728165019089,
                            5281780780238363612,
                            5254545207112128533,
                            5291041501809772749,
                            1345794758377513160,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15407935611479727828,
                            17444373291685559913,
                            14858046868961498084,
                            10480498280602340126,
                            14799578371379936965,
                            1799770797561505150,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10383963341046486936,
                            6151654411419230251,
                            1446668829327413838,
                            8661930182144193523,
                            9460038173457368098,
                            1021728507160749981,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10732344871601567081,
                            4334673536361391169,
                            9275192152153963178,
                            16963048303554692871,
                            10497321073870628943,
                            1490316745275747933,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13368228803805399833,
                            233648479810044396,
                            1744316522751782240,
                            15636772238980171602,
                            10383665931125690719,
                            1607563114181844858,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6568085714437292113,
                            5101175589341546760,
                            7591833363917562526,
                            5836411255444437623,
                            15288032628559525278,
                            1610706357942007232,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6001630566030395757,
                            16129805276494871555,
                            7523417683147092141,
                            8776323131469443086,
                            17674263472204926407,
                            1380557021907819413,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8772970160173574340,
                            7637752877799773755,
                            17297763445606480487,
                            6659181028059823153,
                            12421196189321658209,
                            98602328712329484,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16553051718223743050,
                            3218562186620828586,
                            9833137852221944360,
                            2667369746885895424,
                            12028931757995944033,
                            950757411665912337,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12030030694574518889,
                            7931542603921300582,
                            101816645790800048,
                            106839734525290882,
                            18123056086791568206,
                            866172480240768937,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4299582374421754980,
                            4911522814358395170,
                            10792876498511164968,
                            4050684776833885301,
                            13536722109816584856,
                            1264687921304412012,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14253402898794693558,
                            18439357004671279245,
                            9051386666342325800,
                            1891210518001668833,
                            11509003499379517097,
                            610832256088481259,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14308615142097129630,
                            16623326968740845687,
                            1059676555700863543,
                            16218903140657921475,
                            18334609715041820650,
                            1562981183403800777,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14265831947459931709,
                            12936849133223139951,
                            14443276418286039564,
                            10247674956864454809,
                            9261461096719451295,
                            382971898310213951,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13422982461916310958,
                            5791945023783326227,
                            11766378699369540778,
                            6735874070020392856,
                            16867841614520325699,
                            378073988924542713,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            568080847528580033,
                            6408357865125127946,
                            4718860532948024976,
                            5285675181274058692,
                            4455822686012410810,
                            999791150999693474,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14902341354191165434,
                            2399981349484744670,
                            14741612180701683271,
                            9503973564303477071,
                            12643047586985358574,
                            1643142375783486922,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12987337560242163967,
                            2966937302222962418,
                            5364312612470833228,
                            13678303015294802026,
                            3699261205278052722,
                            1298154652391170703,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13647962696821389635,
                            4647655972148652231,
                            6974525783005341187,
                            3347156329238840650,
                            6353963009490271060,
                            709413528475114695,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5766442711547784631,
                            18292044916531246746,
                            7748107104524064152,
                            10324299033448649756,
                            17742860873905350030,
                            932989510525917724,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5732814184726408498,
                            1905669038607155493,
                            1725234534676840196,
                            3058629682070606784,
                            5405636245563067879,
                            694655108436057901,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12004215899791765488,
                            1173386383280046821,
                            9955519611908558965,
                            9761017236938582347,
                            9840737489470457618,
                            1751633212192667902,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8535274538249285941,
                            18255874256303527824,
                            7209800330484923578,
                            3027483556817002669,
                            820495281422851882,
                            148776460129766445,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7749143497653962735,
                            14031753154473303346,
                            7809437872328726922,
                            4560378531568944635,
                            15018703218703821543,
                            261996199556258592,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11076649602625253382,
                            15583953263696522455,
                            10203388756471500152,
                            9600420485263200137,
                            1071738251141265722,
                            700544702725449380,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15790206933076334079,
                            10297086256199083434,
                            15804807926673601075,
                            4833373735191513687,
                            12636865756548857487,
                            776231900821580851,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15334298496890743428,
                            2826685152164360990,
                            7175470919265348253,
                            5425665915173631694,
                            14632536189457243836,
                            1701022387535361452,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15541316952703968885,
                            11160997080806785272,
                            8746539955128385463,
                            625747746950577198,
                            4077951635570616541,
                            1471092185855862049,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5897846697653356645,
                            9321351151729585993,
                            7568852200058501855,
                            1804085864876831337,
                            11333356783842732127,
                            1330582394599085149,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12197497649925822766,
                            3029833677875758847,
                            9305838143291299092,
                            7597018948308741834,
                            16973480339130745518,
                            1133387070872371892,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14593563103302307974,
                            1829201435110481791,
                            16130184464017805513,
                            14560666697122324786,
                            17428144041703903970,
                            1073390042513349807,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14414903075805128189,
                            8902469104767411198,
                            4695729346012608323,
                            9565817119317933834,
                            4714977099061070373,
                            34275625141951727,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11575551943331378410,
                            2672507706477613786,
                            10416791481905448063,
                            15494766665438410851,
                            16704237863789430000,
                            308150495559285040,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14283382659346721287,
                            42530497902050799,
                            15390820371306482716,
                            17627590674407664722,
                            18298292587364709056,
                            1004858058192201254,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15593892003601633031,
                            2077932155293169495,
                            1522184259765659779,
                            11804207129545949702,
                            9078354854076132430,
                            692429891431248534,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4478047225735513661,
                            13411898304025400022,
                            6755740283454682689,
                            6000322998287227664,
                            9754130746044496211,
                            1280528449755599608,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            984476016254060401,
                            2431754274653309244,
                            7754577478243921170,
                            12880305955497648400,
                            15055166514941213761,
                            838623968196294350,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            257091101910803328,
                            7883021547568789660,
                            12589824078918633284,
                            17135247941195385925,
                            15674723618642751547,
                            1436010198166488565,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2117094966451289265,
                            484853749082556615,
                            12099182347415633329,
                            10433803614192845641,
                            9259015132199125866,
                            919051356237729194,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11675661611922998596,
                            10686683891376836409,
                            10649728010168079581,
                            6005217101701744108,
                            2284559317054356761,
                            431825079182241213,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15402226273880853799,
                            6696707583683680890,
                            13843409310781041729,
                            17324948297966165536,
                            9488539404567146743,
                            1719619610782356041,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7740694143082407503,
                            6492825187388212285,
                            9650884700497170049,
                            10168146683153001151,
                            18402117195135816663,
                            541775821806158532,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17582826872222881437,
                            2419822064210418524,
                            286980701277818449,
                            4471651494654395060,
                            5706430364051348955,
                            1335064285581651911,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16834149116444330620,
                            10066281195196696187,
                            12390560865895847540,
                            5082375909949764136,
                            4551273775178877262,
                            1190241473559022355,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10277476091594738129,
                            4769063059564626034,
                            13998782556996979165,
                            650516747165815470,
                            6486168155632149180,
                            1778522175311639524,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11350040998889391055,
                            8741209360794628970,
                            3760761206803157263,
                            4164028918244472551,
                            9922965434023860770,
                            958535836360016344,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5645982535784659398,
                            3259879960073694328,
                            4818173533411024297,
                            6989662266862452190,
                            15616552747090197116,
                            1483047994247159276,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6093156432645773383,
                            5321349270571331022,
                            8746127340911382677,
                            4003371938802359523,
                            5045751805194305670,
                            547292343156303991,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7402510376603328352,
                            13268803879435769365,
                            7578622836652749115,
                            8876911470440150777,
                            13412896860529519539,
                            551667771941000527,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12438582273312932419,
                            16842491264131298795,
                            14092745956873588624,
                            6442722621031209966,
                            5294941136156335032,
                            653632333364076808,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3649067416659357,
                            16566164308750056516,
                            15853625786235262764,
                            11985555263714426603,
                            8446413818196758461,
                            1007597375591640764,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9134530811377558452,
                            10743273482383861598,
                            6003511514509806997,
                            4974166568332422256,
                            10040325193874578021,
                            1156935684280498729,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10791658693579402575,
                            13806017823607500567,
                            6894145059311003048,
                            10595062118108192384,
                            17576731900753995157,
                            1234398446434964638,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13447386888886297167,
                            18439482024947418550,
                            2451274134805851798,
                            10400855478581261058,
                            8411936957719574558,
                            428495666531752333,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9236824069501882906,
                            1305176338002587906,
                            12895116492717863092,
                            1022148700742803578,
                            10665444195044054698,
                            8854770771357378,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            855549409580140714,
                            15384072111494308871,
                            14900855716610383648,
                            18414070532024735541,
                            1995871966295721341,
                            962918627183374713,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15930356490932892387,
                            2315819748477261679,
                            8448726659242626688,
                            15532405289547574449,
                            13303151266315856433,
                            484812929629932941,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14706104739967909176,
                            7328988080856465483,
                            12152197807570747914,
                            4935687581244095209,
                            4500829633481699227,
                            1272180142451776014,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7982195887344653672,
                            10199301322289390962,
                            12852509751386764296,
                            4405014795784432800,
                            1421573529248407225,
                            730165052592497303,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6148276544739069826,
                            200898987545952172,
                            7329066224807584416,
                            6814208882660762165,
                            11270524602974582531,
                            1122774707915077937,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5063801738640317349,
                            12163016638935551861,
                            8244346646936683651,
                            3264563404852292506,
                            9032795079598396400,
                            1122099347997592482,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10356345323289111159,
                            16950739724928955967,
                            11103534038032159228,
                            13965664353190328059,
                            10100036684770641184,
                            1729016462296915556,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17307851119994987692,
                            1221354035216668064,
                            15140895173796432824,
                            13236691990190486308,
                            14239194363794974421,
                            609053241222380109,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6708668983795643944,
                            329849456319455276,
                            8859301544212381332,
                            8423689021633516986,
                            5392638759356677795,
                            1445140635026565048,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14981688028507330186,
                            18416860402212265487,
                            15705858966326119583,
                            17394853409661734436,
                            10013805154136700281,
                            745068460821009326,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8991478696616932473,
                            7405567657118568546,
                            11776595659247412236,
                            2652420477662958979,
                            6126292633064641998,
                            508899619119953282,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16278430403796657036,
                            3512389402488507324,
                            8522323250054531412,
                            7503432881349374948,
                            8854603850894434951,
                            718757310017528748,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6463605063516530148,
                            11316774844684830737,
                            7604983210990288548,
                            11693318139774632409,
                            13752270205179779553,
                            134177615803577934,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11586550250770058033,
                            13396651550621967113,
                            894824845114436760,
                            12256276494114898444,
                            5828443320809628173,
                            427808104210664979,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16798953408681408750,
                            10486478302611091351,
                            14763511786583104597,
                            14443348201406372252,
                            16026553302000184502,
                            1222765188098221098,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9888771659460338876,
                            17499150657474902220,
                            4876241848200880028,
                            18375553134608775321,
                            6237962292133196721,
                            1000023303178807068,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12451115650411409360,
                            4720047205640314474,
                            9131235393603010651,
                            17178767008906572156,
                            5338719232870238327,
                            325825136352100971,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5206554609916517299,
                            4788105876976337453,
                            16178942110654878503,
                            7512537901304220635,
                            8287547278454499397,
                            1206475975924514656,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17162012041350376318,
                            6596276686994201943,
                            10035858116207546753,
                            15459680986883647576,
                            11953804491592686982,
                            64624429952148489,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7244578673560695435,
                            14774925784527111673,
                            3139989656966554203,
                            14409958096347625139,
                            6162974383380312991,
                            787381125083898041,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            964376610664718148,
                            17905905576048319289,
                            13887044201204775612,
                            13240783434975279039,
                            2091855244573505818,
                            595805578109848180,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14197117873176997436,
                            17699835358721724855,
                            16338678874568709841,
                            7823775105559628562,
                            5523110817520296745,
                            790396415935820694,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14677488522377884755,
                            12258932925743408650,
                            10016052490406587741,
                            18398290098649741259,
                            15462818823084337489,
                            1233217459840653084,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7591999615884527548,
                            18075223874460873643,
                            6684714364703593520,
                            14662462173734567430,
                            11930202137675577378,
                            765975871438094116,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15471201705039400309,
                            3455001272911433511,
                            7973550011377549681,
                            4095731330693508195,
                            6658201555331874357,
                            909606139064288151,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9816550220121165754,
                            2463528024718824214,
                            6421902019950259771,
                            9938773670540370117,
                            3431246167526572269,
                            1617962201769260112,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9857092290450040357,
                            6446251589387434650,
                            8500788398331470638,
                            11247867793881600724,
                            15351112529316923967,
                            470615213801508513,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11848847478068824019,
                            15953193637118012864,
                            6642406716687539943,
                            13682072572904438113,
                            12168646851410753558,
                            660585667364292820,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5195864754440114583,
                            14717989569474406588,
                            764013234210717328,
                            7460283496121946334,
                            10360087603789119755,
                            354707759774127734,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7283007813424736303,
                            14698149941513445542,
                            5419861808116010362,
                            4869141838988423694,
                            2565472000233618703,
                            499011439464952642,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9620852391959830978,
                            11797267328051158786,
                            16849350027406478471,
                            2042679277934559074,
                            15649143285186293510,
                            1769660967008521564,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3474443786719318305,
                            13895979291839746103,
                            13503327681116120987,
                            8352521575920348064,
                            4899554656393003458,
                            127318890018687026,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9740156985040527726,
                            14062519936970733147,
                            5935950652366044751,
                            17259881294645464631,
                            7648168830526439805,
                            1313876073053663622,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6077792753366829399,
                            15814872151667598022,
                            5881630449629336968,
                            2340328437602471916,
                            1329029073324277560,
                            179096513479161672,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11691107325823545188,
                            3951943650021198707,
                            3004084068414263872,
                            16177149676890026442,
                            545025422486732044,
                            373130671928335613,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14436485247562212145,
                            14390055785079722082,
                            6802110372354206318,
                            16054961682617713437,
                            8291693710065610287,
                            460133088284168333,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15609894277874790830,
                            3520877652784040202,
                            16889355961098636722,
                            13428200175552106151,
                            4405081207892889146,
                            730827225125088990,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7683270448196653420,
                            965436757788688520,
                            5624465021819746026,
                            8556217125032881652,
                            5565431464602624887,
                            100070634253081236,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6730370609936033294,
                            18230382748432162571,
                            13969116106781629372,
                            9246842910486474949,
                            250907748553802583,
                            677551781897198955,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6229008888401616029,
                            6301999709520045790,
                            10844063232126285971,
                            3875008105431359871,
                            17602636867466643287,
                            1748199371629067432,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18019562834591848403,
                            15445540857426442977,
                            1262876873371340907,
                            14930248245065870240,
                            5813676975993321716,
                            1228361843338730421,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9154888051995123482,
                            11793583978376683526,
                            14155003584632594933,
                            17100458224243617658,
                            9466735011427984669,
                            1573070905980026530,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2789924502477989988,
                            1789805223858413848,
                            1292265366005939739,
                            14932001982744470614,
                            12058586488036579126,
                            502241590254173281,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1760117820111699695,
                            8806335062100170015,
                            7969864783681429809,
                            3263926016537925248,
                            12982346120433958748,
                            423804028004090838,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1617383170528027962,
                            6720128694741609344,
                            13638550289413158828,
                            15587030654054476679,
                            6157979994712108220,
                            148250245627288425,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1564877982363645952,
                            14980429569400212009,
                            9817677317336117744,
                            7092343800806901058,
                            8283769966364960000,
                            337354760694218127,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18236771844017101081,
                            2027844651191072111,
                            8067314733565541887,
                            15873326412431961469,
                            10975003867493323403,
                            161205323180982695,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1906755284805073794,
                            1573543250341168977,
                            11221009663296529581,
                            15308037042778060131,
                            3397692999072696152,
                            775134971754681931,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15113901146194846714,
                            4753818568876326387,
                            18276725408831568933,
                            1420752961294252962,
                            15237402137953730112,
                            1540839359007499459,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7617021285281298138,
                            12351790618547599267,
                            15198736351846765325,
                            14276494012955162928,
                            16250023746087810082,
                            38630579497506633,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9644967813107739153,
                            6685462290336533045,
                            8524733862507764860,
                            12940096764591469181,
                            12385684114319791140,
                            1025697457303688653,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11106304733771437209,
                            1969847532000629640,
                            13557867517137585998,
                            13587320034588603358,
                            8640931887011803461,
                            1518179492371639071,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2604064966728506527,
                            6922241088526427662,
                            13462436602954247859,
                            13902988014958064733,
                            15944190895403533825,
                            852871283734068303,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1959521018691567996,
                            3071933930264841997,
                            10819956055610031722,
                            10198975020982422947,
                            13306756698879195840,
                            354277697405995235,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2905815252924093418,
                            4804612582505997119,
                            7832772939198209269,
                            11666263092077937670,
                            10794921094467513417,
                            992217338248509696,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7772088985534276417,
                            18008902553255892813,
                            3727504315264615681,
                            294163955478609274,
                            8917445582098125211,
                            471347578296707255,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6906814344118258259,
                            11889188607612695029,
                            9305070740297416538,
                            15074722697990317176,
                            6116560248409091163,
                            1831544946012720252,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12653720154581985076,
                            16784494276165199171,
                            4621511277319125862,
                            3460459809000322303,
                            15688782065787574795,
                            1802464951454614029,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17242796131169191713,
                            8917904062924712457,
                            4555598213891193139,
                            15595654696059543635,
                            1315360184078117865,
                            138210125766055574,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15354582944054503961,
                            7317394003156761504,
                            7319782037230520281,
                            17038608445604340532,
                            7021778851102260068,
                            245313859056525632,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9628570159603646819,
                            6646982870404769129,
                            12601143354358445599,
                            9967928343784489728,
                            9213154740704683486,
                            842080761970843494,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11171253505777851014,
                            14251689094357063877,
                            6420970113759501168,
                            11545162438443094787,
                            2910578451333658017,
                            1481107841370450217,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10334545241538730777,
                            12710596646710957529,
                            6533867814023423066,
                            13710105843226245206,
                            3362464128605720166,
                            526116607093607704,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3479343087439599832,
                            8995353238416155858,
                            2352857322168379713,
                            14358886336930181561,
                            2882816849039941350,
                            65282257514800343,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            693975797316198830,
                            3216817924022043647,
                            6783621138782180451,
                            9229998978382190633,
                            13817701674833300799,
                            404014521367538414,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2631181834796279890,
                            6865465244614601451,
                            17252136365302676587,
                            12862400938923291835,
                            5009644284695612685,
                            166275576348799099,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11771817948593270070,
                            17051819898149434979,
                            4651519519412105420,
                            15687339730920273267,
                            6522757887572037526,
                            116192281375660481,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2260184264528173778,
                            10210787862957987037,
                            5867832714893279665,
                            3637168092714147709,
                            15978506368345710456,
                            1825465386532779562,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18187183006672127841,
                            15720481874882462599,
                            9794208433577921835,
                            11707935485459927674,
                            10950170517229711882,
                            1417423657790619462,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17052088819402401794,
                            213187642644976780,
                            1566455377200019844,
                            10267856563392647696,
                            3850829850103395684,
                            1516081778816594409,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13309462842643256329,
                            8745328686258936320,
                            2138077766523380040,
                            13609941151911358085,
                            13110158721747718099,
                            843136711938873032,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3628575059070082902,
                            5375562782356704418,
                            15292822915148574734,
                            7357477466615566511,
                            11470097017019735639,
                            575678184745249416,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5800101597459162187,
                            12897919661554396175,
                            5749018385061640798,
                            4973656397069680698,
                            13716183325370422894,
                            1583674049482086203,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6333586568831189090,
                            6857820192021945499,
                            14834330572356798546,
                            6456021078102150762,
                            18046342692640677075,
                            1603559214115201550,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11689689939579325663,
                            11524733982066878886,
                            14106333322073883248,
                            1369672476919584106,
                            5303831343735155145,
                            1815540583824087056,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6870629436957946676,
                            8395593508713549324,
                            5227624762738955341,
                            14472231088637639239,
                            18245476472116663505,
                            677607514594141188,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1759238432782850804,
                            14148320875021722375,
                            161156132222717039,
                            104704321651436098,
                            10852416503715883073,
                            1078808954456871014,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8002890436442123161,
                            4707101301849745190,
                            7502855836593894425,
                            2063358170362279118,
                            1109004709534639634,
                            913600046300823696,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7745237912110697675,
                            4568048020342522995,
                            6827833430308072742,
                            3969409800068017743,
                            9363562798800420577,
                            1712621992524464364,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            503733034548025058,
                            1224161582931573184,
                            10813708582833491543,
                            13966786455092580102,
                            9748872238365194426,
                            410042249140929369,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11574098333606378763,
                            6151438856288008040,
                            1295852700522942520,
                            2480475359445764733,
                            2111496808227083529,
                            364257510085786748,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4890663561164336212,
                            6982661080163290211,
                            1408815182583458273,
                            7777116839910071313,
                            12732205372306621194,
                            1612730721922604476,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12271107747833409169,
                            17903244240046918779,
                            16941321200990957765,
                            4657788724433299824,
                            6386794740662264765,
                            1801352275058639484,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1856016738856337889,
                            4913877744720570242,
                            15436782956713450199,
                            10382644047489435419,
                            8943072923259388842,
                            870034538836119805,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15403838252298797488,
                            1770571998306181896,
                            2493324830803603003,
                            2389040677011876755,
                            6149917141661798131,
                            812872656500164213,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16986351607259536291,
                            6432911767354423774,
                            11067544335167525263,
                            4169538258448236833,
                            17066567244911162322,
                            271057970575896220,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16673466327403992069,
                            8728811735723560212,
                            17316992312888886968,
                            13171528736499246323,
                            4306680176197053045,
                            666655519655859676,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1576568291874371903,
                            5674265165640857418,
                            11655223367693189788,
                            7608576459283826141,
                            2100647013342745848,
                            1082028654992544356,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14614724378806630097,
                            1211385095296391493,
                            3372346757588805839,
                            10120413208437550071,
                            14657655015256657365,
                            1245925135878060710,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8048756261548760061,
                            3752326594358919206,
                            7785367383808924560,
                            6895867163714182022,
                            7712718917190978635,
                            785804053985723491,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9935478279955039797,
                            6824993079362036886,
                            8474168743214332635,
                            16283744502545759966,
                            4563586482932533308,
                            1441135089347584616,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3717366999113342264,
                            8715459760807463484,
                            3169992774274832445,
                            6998437019195679477,
                            8932487288973081945,
                            1851108503345162438,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7231661550374710895,
                            4346473901003033216,
                            6025258602223109440,
                            5000161988292269240,
                            2426665970001413168,
                            1473774334147908245,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14885591206670401588,
                            7669225956167260167,
                            4204367430279692624,
                            12557328967124909068,
                            17964420834779509475,
                            856009350598164592,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5592788438311016733,
                            8539495963324453842,
                            15174676731382858195,
                            15184144502380514546,
                            15405138936132558623,
                            1592204956221888645,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1428144629205055718,
                            14309613097265020352,
                            6218531945239583359,
                            2613570837696211461,
                            15956831273077321572,
                            613688129859031185,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17724459694961980044,
                            1712687769489594330,
                            7176341114697117557,
                            17544354775463871602,
                            90189646883834560,
                            1833621169398558972,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9024162772283717526,
                            1756273913574908891,
                            2221117933523715276,
                            11531703024810712619,
                            15340309474866999163,
                            1138474323323125470,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3347536680028821906,
                            11000339386775869064,
                            10088246964071034140,
                            15451838769483404774,
                            9852012535391158801,
                            1600911439501110386,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1889742963113312087,
                            12395130136904023340,
                            15079324122845583450,
                            5213238838767409819,
                            17480637030792910868,
                            1048088086678376766,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3967695283270242628,
                            17314619670179671838,
                            9412560239054202087,
                            299232575354085940,
                            5994364371205128721,
                            105188487112917482,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15711480746501693560,
                            644620387137773212,
                            11413043432702558063,
                            9133591798898543386,
                            4714688826404300983,
                            734895993419826543,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10016143352503192912,
                            12056594488974465489,
                            10690119421400085407,
                            6063061598035504228,
                            18131803956496134921,
                            330285622802132447,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8176494531631940641,
                            3595017461147257851,
                            99450318881478714,
                            2722272514917504019,
                            3480322516442732220,
                            1562400622865690590,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2239141410284766695,
                            1252379756188326153,
                            15015276588277667986,
                            7017858399445707837,
                            11589255690585861507,
                            24403915446220137,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12142707544617236018,
                            13424914098329006047,
                            12948508768865320148,
                            6126051138541539569,
                            4653308918464384708,
                            509905904864260497,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11074469526781604125,
                            17835779313400850281,
                            13016894874127377800,
                            2750720492476478164,
                            9798706872956081920,
                            1462689213914250047,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            657823640975140288,
                            9081943589268120471,
                            12185298188276038795,
                            10131394558727255803,
                            14646672380985019787,
                            41121574443442097,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13780581423017258895,
                            12112106572845675511,
                            7199128607260094097,
                            13123762757086778277,
                            18380350472118354916,
                            1450763121436993636,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2168295911937557577,
                            9417927238004244394,
                            9038204375940530146,
                            3641775054100273448,
                            14706844732328537620,
                            539431996302168642,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4811445544608193624,
                            8908432936072753304,
                            3910843857381672728,
                            17837696035146587574,
                            2691568608182271399,
                            1746937004146828591,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3779468776893813160,
                            15113507542579561546,
                            5647755718044925043,
                            12784918757261412810,
                            17570984365615050899,
                            99033306831554368,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16695773489227624055,
                            13454214442513659800,
                            13718160544872091267,
                            5448583397800426656,
                            8077231275047511552,
                            978381129161214731,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9788396266923176742,
                            6246759682444571246,
                            5797872549001865462,
                            2833887941380116127,
                            12897153641356681461,
                            102092579119955864,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17452801798817584766,
                            8250921907676490894,
                            7024091394224743796,
                            17490302059595655412,
                            12817158211012880197,
                            797526746071361221,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10627710397289688813,
                            2145038833685487687,
                            10115520745936693014,
                            13604262030508249937,
                            5995166391442006699,
                            192913778019192436,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1038614807573142718,
                            2339529296952016484,
                            4216937922333943838,
                            13736764648592262165,
                            6922026429426666523,
                            569031779721435531,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1146094057598018040,
                            6058936810588890758,
                            3494736087099947236,
                            13983677117266594632,
                            5933596276314286058,
                            473649367739216809,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3380951182420483659,
                            1354234010036100486,
                            14059274565711145033,
                            11432533891532624594,
                            5939977328433094167,
                            1276541122996103506,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13630406259103455851,
                            8406508743301047285,
                            9028170919419454349,
                            11359579923001526623,
                            5076078439152849498,
                            1542524018236372624,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6524815426517626267,
                            9628473004726583872,
                            12961036077053706218,
                            8720303087634976491,
                            1625745956205284954,
                            245210428302159529,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16014167616761426267,
                            11391023543046801469,
                            8477294296535933807,
                            17140181195246614544,
                            4992338115617028834,
                            234463410569124982,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10702346424655560519,
                            6973534236428619603,
                            2132447154126148428,
                            7884138117132204986,
                            16738848807302566801,
                            238591324268840391,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13495778902260784748,
                            12674641593491168722,
                            15238190638477862417,
                            5091852271291966328,
                            12887187546193535546,
                            465577747828933929,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6132818351947590565,
                            5224239168541998153,
                            9364827248415327367,
                            7500465297502493303,
                            15683258250742973249,
                            1736689139982775664,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12619349705358827933,
                            11589986684012701599,
                            8478358343294014181,
                            14150336504331880937,
                            1098909976801765368,
                            1837944017979616175,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9295925073123906153,
                            16789948123543045087,
                            9992995173666932493,
                            10384186267964862469,
                            189092933295164691,
                            928421216747693343,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5657227261502783884,
                            10481575992825755575,
                            12852534117680265352,
                            6551390174100456647,
                            2861733810370052715,
                            616973362766585847,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4726257432775496252,
                            18282342031798436986,
                            8888804775890718024,
                            5727139763788355103,
                            10636595209552455173,
                            109181679635951316,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1468675434059786388,
                            9013679820600335613,
                            198551958820584871,
                            520240110933810562,
                            13932903522520032071,
                            15498170066324210,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            903617550355405586,
                            10882033145266524563,
                            14992513647901952441,
                            10748253053672168457,
                            11393295244825418940,
                            188549837416623849,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14149360224756068038,
                            10118893560323355021,
                            89962033538707349,
                            5038818549706589299,
                            17485136638700572415,
                            18661879015767733,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15543947721599611528,
                            6514437771861411141,
                            18278234815917518609,
                            7506190055397539751,
                            13259247767143916349,
                            1267365088274497062,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13817272620911592145,
                            3959217330069749302,
                            14110685520752818295,
                            17999954551793050606,
                            14922300306742255299,
                            274465417703529089,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15398533160777794603,
                            8988724776762614424,
                            867523300488023544,
                            10383613868885390134,
                            4001366584719817008,
                            1421627386999959757,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3404121002601250642,
                            643715100164083383,
                            10357726059280565892,
                            11947460431038854076,
                            16007756234291862893,
                            321885045600372225,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12813210219520704993,
                            1147998089278713958,
                            7423644060386155257,
                            5300110017972969495,
                            14430431292957282483,
                            556837840738587866,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4901749189344106900,
                            12842680847425107242,
                            18049575134012410863,
                            14334924552243050742,
                            4831874013491645317,
                            1231736870337211725,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18385481721768868622,
                            14366108526485732405,
                            10033931138508181194,
                            18248134782010508977,
                            12524226921007405283,
                            1504999656186464664,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2921674558598841225,
                            8149617458354309328,
                            1994766075555320964,
                            5059609041575062079,
                            11509320826069736403,
                            1563838037813555461,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            408179174090127167,
                            2310198664916344449,
                            13623630568683445440,
                            1225479829839464461,
                            1314211268778660850,
                            98115091526803905,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16844638163573555693,
                            596358288822251692,
                            3197558008374948453,
                            5343908631482809806,
                            4819092800530130076,
                            149506110552740081,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12099026007114374188,
                            17549248948593877427,
                            9851405802010023164,
                            9818509176288490327,
                            16672862849878518738,
                            1537701962298020243,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3573509671664851199,
                            7297653006721172559,
                            14141267901476864826,
                            11184589881444202325,
                            14631835788022587814,
                            1071875822122882262,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17301229731300534455,
                            11334943526650262539,
                            427798726286065318,
                            12405369455114056537,
                            5944707807112146633,
                            1716053518123094536,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5174312471639608792,
                            13589055377058869589,
                            1389558569579741012,
                            16187965611024249573,
                            18072377302636297603,
                            1757870565843133525,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10799799951639595154,
                            16237111341652665737,
                            5164736259233629738,
                            2246011626603226390,
                            10023699186773610821,
                            1317939572584929167,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4204941000402455561,
                            1256891490954464229,
                            16647219675294149416,
                            9342534687073572843,
                            5617263917922806560,
                            301768037496288370,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1309184372418157388,
                            839417882516872731,
                            11452553845728022649,
                            10959458418503596755,
                            13954201371421618646,
                            1815671464480448808,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2655236239667186542,
                            7259251914163771429,
                            6932269439847718805,
                            7014061408234067279,
                            1711888469007624341,
                            1616457193800027410,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9037264112046879551,
                            15547670444712607929,
                            13677391531115667061,
                            10415445572415629737,
                            5089363296566237467,
                            32537888183217779,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17039155197893239304,
                            15808380855740269416,
                            14246697385100382652,
                            5121749173244302614,
                            14281549853660958908,
                            853741022092446672,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8611808659443045587,
                            18389054823498408955,
                            596030688651975355,
                            10486438957989503657,
                            6990539099961680544,
                            1470256765658782558,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16935735538129886967,
                            3914586943009327996,
                            6974658085656322958,
                            16348659526546713116,
                            10004441471175197605,
                            288285004897277149,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6004689603552514733,
                            13687783477445567893,
                            9065325517762943974,
                            3153594263269357662,
                            11883830541493721035,
                            1603080297547140678,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17250016866135545774,
                            15359686955786944565,
                            12771401794703437522,
                            4870732488407493073,
                            8086462307128859778,
                            830425717566023706,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12472474932509282500,
                            5015254865519534055,
                            1046532302408555644,
                            9088557284198436700,
                            13605605617688226452,
                            912615786447090476,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16164031551506548205,
                            11115460024409552460,
                            9650047394105950371,
                            3885859036376147041,
                            3179795826723439296,
                            800276339049225864,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1952257428944937061,
                            4765933621196023621,
                            4354548780225760155,
                            7183148180993179018,
                            788802990981038856,
                            1513312483938989143,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9189539231792771724,
                            15452682771193780034,
                            4460941027996314770,
                            4676757286723579016,
                            13952147609359274381,
                            699578579415700671,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11729111138800370692,
                            16459314413374052088,
                            8174432340604483462,
                            2457989749254560923,
                            10917110052085036639,
                            1259603197583414186,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            18225274810136733185,
                            12946438279241636953,
                            17015819716612461141,
                            2190767887779724861,
                            7427758516457844662,
                            266545412094517055,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4134843357627639107,
                            3614189885401481216,
                            2985246351220113184,
                            3214404360499202928,
                            17966045811405645424,
                            1161707924831675554,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3505862282315702665,
                            6928139799962796090,
                            11088259690874814376,
                            3948529839947695732,
                            8340353925297545557,
                            1766915652075343978,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10526687703230050682,
                            2608628397398007043,
                            15046483330140355434,
                            1512483995284632460,
                            6281705785584325064,
                            882377842414631025,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13733701579950713834,
                            15443500586259955404,
                            5020405524050314196,
                            17609811499297800802,
                            10686190099098444404,
                            907935720499953393,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5219497437650863647,
                            8914910322466332856,
                            6960264873503194952,
                            7521337706427535055,
                            10435449137177110054,
                            1855116562948665153,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3947971213813819225,
                            3985672343359261800,
                            15741925706263554293,
                            3490655800486633623,
                            7347579991141980421,
                            1697568492355534089,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14173238338997795675,
                            6435235678700531995,
                            4809134439838286924,
                            8352431182993115427,
                            11819769208528215913,
                            1672011249579288265,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5090581916949263251,
                            17322283451937994092,
                            15175209780312723017,
                            12241231124103099608,
                            13172170630554217583,
                            31343410208816024,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10590929945557649397,
                            13389094510321847133,
                            13921777874409872497,
                            7119002176226726347,
                            14955859283587817705,
                            1761614462693490606,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17375925468298353919,
                            9652898321685399945,
                            13816741675743216756,
                            11603114438153812563,
                            3295208075794502531,
                            32635347862028275,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8028750343586139848,
                            11449484256635855255,
                            15461282071678093591,
                            10925220089703344411,
                            10130279599246422101,
                            1817195929208381583,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12601926062286917072,
                            15899078716378871585,
                            8724664095645831093,
                            10027043763850929713,
                            8269797299854171227,
                            839256083651064962,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8760797105365171336,
                            4652595850720728706,
                            7750461893121410942,
                            2329717933672960504,
                            845496713747931444,
                            446861739887541797,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4918229137218181462,
                            7507157771599521247,
                            13897537939032697259,
                            9251210598584951262,
                            5128879514976160436,
                            420275362641386338,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10934448794607816686,
                            15366671333045242822,
                            2450156526228210548,
                            3532091855015075767,
                            12158079228444849968,
                            219623913380634838,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10601273475020169674,
                            3267923388208515397,
                            14067532599840757145,
                            8755646368691085684,
                            12648232626657575013,
                            781173788210717509,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7195934384692936389,
                            9264503063949455592,
                            11490921913245392651,
                            14473107412237585735,
                            15085398434996602579,
                            1097789344695160382,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13144905322119668681,
                            338926503536273694,
                            7334715624773456723,
                            2341349602276477687,
                            6389403802860322939,
                            916951698347559100,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14713195902227473990,
                            11340350047156062896,
                            7184957490715138390,
                            11467712195203988715,
                            9035846137737188599,
                            331924350185729697,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14091021038604436676,
                            3579886937146050202,
                            6462782730510095848,
                            16070072743477008021,
                            3171416459613772883,
                            1413674921736990226,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3836202597954322206,
                            8767674938116416773,
                            5998032352453840054,
                            436828849444488567,
                            2651737937588967897,
                            196843457547691190,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11775585960971368653,
                            4944570035884304478,
                            9777081632668553221,
                            12532206013378397289,
                            10416077636982140675,
                            237967815125606091,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12796144687285966258,
                            16792907346337183841,
                            1748634166801502867,
                            2770416065004473031,
                            8684566957770253743,
                            1125763907515840288,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6965197344523728656,
                            10032376769352613873,
                            16857111790620621062,
                            3003609568677781698,
                            4093808801510054481,
                            402747877809757681,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14224243587048502267,
                            279303328981674604,
                            18341715871466323221,
                            5891052763451618193,
                            13837721834073534283,
                            1730729440054882025,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9265166481282088648,
                            14011484668677791233,
                            911572026762428283,
                            6585497528024296942,
                            13215815476591079485,
                            950707015315105415,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
        ],
        infinity: false,
    };

#[cfg(feature = "proving")]
const PREPARED_G2_BY_TAU:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared =
    crypto::bls12_381::curves::G2PreparedNoAlloc {
        ell_coeffs: [
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18224921920243037867,
                            16963770909527695949,
                            5750666909137969540,
                            6161224692404551343,
                            1779667657432873794,
                            876975598140104716,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17031392770447353241,
                            8347243996717321168,
                            13637856449354377907,
                            10779456231282798217,
                            6524819496748122126,
                            133672546881804795,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11169460466474763443,
                            16541113849791986472,
                            18157258669354112740,
                            4670277874380365461,
                            6308897172577454413,
                            1453219972434381642,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16915316941324633080,
                            6929210588322574971,
                            8046845609944923742,
                            13465826404040537395,
                            14641472736206511944,
                            218517007424029694,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17004275010030791416,
                            6886288035289633932,
                            693632534468802136,
                            5643457850259105219,
                            11821411702346265098,
                            607627877932214575,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6089656385827115660,
                            5791103514104218182,
                            4078729188458198728,
                            4145928541233359833,
                            3749521653594911683,
                            23976174838776978,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3484290394869898944,
                            3340633585033722815,
                            17930800035169753263,
                            11941031283403997603,
                            3511686102450541365,
                            244400625670965891,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3400249205652249471,
                            3605260308113300151,
                            942696137966592067,
                            3798810728325294958,
                            13689108681756147758,
                            155709277517970750,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2250827024878690431,
                            9816057176282534816,
                            8712803972187857374,
                            5899355345021079657,
                            13418479232299621997,
                            1293277701318389578,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12036006052099125838,
                            3700925562646565776,
                            345500549045550025,
                            17163839783288038704,
                            16372573073137346944,
                            57056869733269883,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11570007658818551920,
                            10048697446723432352,
                            9879427060604544296,
                            10768308492827554173,
                            16656027119278124576,
                            1203632325751657597,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8907419218176342511,
                            1776012211739587451,
                            1311228790963909606,
                            6553009295796878527,
                            6588715355770690975,
                            779661024965383477,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3407047717810488313,
                            1141378490915682439,
                            14723159434454964377,
                            9428307792150386838,
                            17667317324284165043,
                            1388012089122892252,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8206737354163542306,
                            2747085725749573616,
                            7274874395313749161,
                            4321691458098418690,
                            14988036829348695092,
                            153807182245114242,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10205474264356380278,
                            5648951909883922848,
                            5901882852471479563,
                            11270578652756219415,
                            16596020704212016247,
                            1551468028598773958,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6902345645543514969,
                            2233830121602397560,
                            7163053857775157969,
                            1161568851134297062,
                            1775328417239519623,
                            655077872367247919,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4423995686279571207,
                            5430375670809537692,
                            322690540765201676,
                            1111617052000379964,
                            17827150657938423071,
                            1154083982071071472,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4939342530227888869,
                            4809682087106630566,
                            1159719011802309246,
                            18232720220921513326,
                            6664002055928931026,
                            1427410198545954895,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5124694146960789200,
                            10188786392370950951,
                            1467013830598429745,
                            12578632130507870628,
                            3192043900369628320,
                            1305277161335651780,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14085434351501177381,
                            5955665746621367979,
                            6491426391467322948,
                            3263017054051139443,
                            8083695236882769366,
                            360092074312304790,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12878554161521367434,
                            16004116564281596579,
                            5042908963784476677,
                            8018796535673409650,
                            16111840984345269468,
                            900678512631140660,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11037867144951181021,
                            14388547707444955431,
                            8442237624900298809,
                            11821709842616055192,
                            8716221447547051359,
                            1847698803686899312,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7754654138356729663,
                            12204548797848641278,
                            11696640774377778046,
                            2659147012723254477,
                            3239979536971372011,
                            1019497630109983832,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9644364304708729225,
                            3990130509994833012,
                            12909045453998044372,
                            14020619909950325632,
                            16209079635438457338,
                            703559621829185138,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            521570785793274175,
                            12404027862411124815,
                            4931117276198269118,
                            6406226738391172544,
                            16978164789443697382,
                            1466646186048471314,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6369444957486195873,
                            13401562769647497339,
                            1415577575641429197,
                            12950170093148885741,
                            7467537918588629961,
                            525265662562615178,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9683891409327803370,
                            10017959498877575697,
                            15738352953123636737,
                            9268535294623722973,
                            14134148843199533758,
                            854167626694229369,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5189756454962075004,
                            3501592988243799300,
                            14141210686218352921,
                            2412684742995210594,
                            4786102726184043015,
                            1358831899578570599,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2682124014904516618,
                            4244245301854816209,
                            12685457260915855580,
                            11785156823431467961,
                            4267864347976941081,
                            595813251103569173,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7364063293712008106,
                            12471029772158807800,
                            3342271630049838790,
                            17175780719102638159,
                            5913384578854058953,
                            221222134797533293,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7205993492005089978,
                            2036712367098471051,
                            7570607233112494568,
                            11569596427422099143,
                            6970857398857291165,
                            725176844204550684,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6005565266186309318,
                            12482572669684424062,
                            10262083292538430329,
                            10601152579822478562,
                            4099353575147380456,
                            869163636127439223,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6239783908246374097,
                            450045265194096615,
                            5606674444368488401,
                            1327537000889327356,
                            16084780994075496133,
                            1334500056675070690,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10605871278473531979,
                            9135984603060305114,
                            241270738292452010,
                            13419786466557394744,
                            3248981344493167606,
                            1710816401947983321,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16167974822407890865,
                            14689826144721018088,
                            10376987218136324220,
                            4583685378037727088,
                            3505472045211471485,
                            54627025631437628,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2050827882553462916,
                            594250610035665750,
                            2518043122942970377,
                            8894709905443930469,
                            1645766990524457905,
                            1106095359447071707,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3758501633341042599,
                            11462242976391931513,
                            6201412642550203359,
                            15504535530598067316,
                            2661355270434921423,
                            873272241935483950,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16401219585552266351,
                            18008461639933310933,
                            12217125206449652562,
                            5903520424482151120,
                            9923210668374318005,
                            1000477819626246033,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10975385657225696960,
                            11181651068654221137,
                            355847784908137714,
                            16902285089692119826,
                            9641272264320018245,
                            359824809922211466,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2851440656742385648,
                            1230619420857375851,
                            6155121985155583224,
                            1245108839174225610,
                            16046505728263529475,
                            1008221681564371387,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10479775368060405226,
                            10122840672322492284,
                            852619325199600261,
                            17666546167084609510,
                            4352255636663450137,
                            805107438418495538,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15797208644828685991,
                            11535302793342446510,
                            3049445466110554953,
                            16941303234103131061,
                            8350844141847619831,
                            1617772006564176342,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16715651649328085071,
                            7304417540693398222,
                            18095532323422457358,
                            7407399996396137861,
                            6699646023390499489,
                            1251570944214912321,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8530459539286371109,
                            496344283265439822,
                            15881954623470855999,
                            5713718989037209361,
                            17400785332830072828,
                            363183623536061292,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7941335846990478532,
                            12180159092814656818,
                            7071759709197086383,
                            6547266708116200308,
                            7997081955092525206,
                            1273490915442949146,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12284058898697354217,
                            4386892510910142952,
                            31329226496008718,
                            6044498179237592792,
                            1379524638040847638,
                            613295971451425540,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13827296025878007916,
                            12918839068372257559,
                            15967093291970752785,
                            17439067018441205825,
                            8842921037316501461,
                            272592691257701131,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7106682852896079949,
                            733279532362536949,
                            12600886512146702086,
                            14969070760735717722,
                            7479171185138273103,
                            932704770564453332,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9446771951302749932,
                            17428987259647436246,
                            4167057343882312664,
                            13391826912862829828,
                            7832075961701158755,
                            561586019676639032,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            316393679405781128,
                            5691069122067286704,
                            9935705868496101812,
                            9187441437135626577,
                            4301536559310963702,
                            256393869407154216,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8676386141610251463,
                            12661136323934043884,
                            11339766753545024048,
                            14028293925521533286,
                            18362600735863831926,
                            255459776059276708,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10615303616352915582,
                            15050438809462279235,
                            17557058219476473105,
                            11946224410177810956,
                            16281370447620096284,
                            425321681076965315,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10446212320761833329,
                            1026731385478323501,
                            2366764486811875264,
                            160189944832515513,
                            931781359680516116,
                            1859148571574098850,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7698460029191652095,
                            8615549064715288194,
                            10622381980563899295,
                            5571394233394763677,
                            1373825518465732172,
                            1263235425302582159,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6002676170175787067,
                            12121410416672258813,
                            40316775438886922,
                            8145007044074013318,
                            4556807643828526456,
                            161995735955794890,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7606980043778855591,
                            17937151583074055490,
                            9997772446709126371,
                            14698011731395524765,
                            12027625623578449187,
                            1287950432354042674,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8346936315609994790,
                            4995455257392546402,
                            18057400567885651792,
                            11906099631993096781,
                            8075565949645759187,
                            923804830269221345,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13966862128171710475,
                            15638883887559144612,
                            9453880202985090081,
                            12926818221768370737,
                            15372297807044008578,
                            874851818651331552,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10630882475075833403,
                            2949612633433729267,
                            17858855241800989363,
                            6303169227078002664,
                            5848056443139694547,
                            848424508126827256,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3395692183407230304,
                            3923069112423637350,
                            11776078573175767156,
                            10293444205909427703,
                            2273326497408843376,
                            858664959922176904,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14223288236053495135,
                            13640342854149663094,
                            6003554278130424936,
                            4767590822799549678,
                            14340532282828748053,
                            876026687154923953,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1172064007834759010,
                            1518886498482895630,
                            15681675251077189154,
                            1676668847148281686,
                            8816506638519127863,
                            792527051609122724,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17334764738295049702,
                            3950535710313686437,
                            18421983508673331442,
                            1646967177776227990,
                            10320811204666731981,
                            890667717965081435,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7011355965432919169,
                            12965003468038668775,
                            2236713308897809486,
                            9792564390237250842,
                            3476257900504351056,
                            896462779609780136,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17510675175101443227,
                            9607648012023711150,
                            16814541521559061456,
                            10513950899249779573,
                            14056131454840696700,
                            608974589780839287,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            68254755976021275,
                            13627039527070419308,
                            9294381047346897420,
                            13554535746101203258,
                            758305286261276233,
                            822785707106552219,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5558123840884964354,
                            22165123852183458,
                            3184855218011355486,
                            14222260621266892913,
                            9162075556927238727,
                            631069426745691298,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12180995559826214260,
                            13250749096872300826,
                            9512534291587183784,
                            13760915172672982679,
                            6091450227349793138,
                            1454166316785133972,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13848115304358180649,
                            8428755612591069885,
                            8976708833098079672,
                            11025346688129374462,
                            9053237835820407402,
                            23429796087735114,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            507259760432531851,
                            9437690467768972631,
                            13392836511327507697,
                            12186249868078985180,
                            9139905540625112533,
                            313078319678103995,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6712932192756402522,
                            1980406708206318167,
                            17737248225051827855,
                            12608782461731358859,
                            11044318293378582102,
                            400048704072855662,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11195258147990490807,
                            4333222269194020551,
                            8419854405712007370,
                            10791680155783604508,
                            8947477387950856462,
                            697751077534142571,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16566722484945411617,
                            11171775714982692723,
                            16793358444165111621,
                            10719221084320871479,
                            1536231240942106734,
                            1667553480017349150,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15250701489052305942,
                            1246439802665269425,
                            12530773988666316113,
                            10024295570021294474,
                            161780560200097458,
                            441474959492149743,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13698869803834151614,
                            1826511158127443596,
                            5154993029877197877,
                            2969842095770378548,
                            648954980105002236,
                            109966954703369541,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14799378493778239420,
                            12848298807796260062,
                            828537812591013043,
                            18139036503946395124,
                            13356088904638179908,
                            412675462396887820,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14153673796610603288,
                            1392957724067217551,
                            3408399685394106113,
                            1135750038257243562,
                            15095115932619168155,
                            1765313619571181001,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            868567606822016596,
                            2998478295119321540,
                            15512415946926356421,
                            15665801294270186304,
                            17080272696105501930,
                            206251387482263187,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5581150188929121915,
                            15613041204130469312,
                            12593521971388510790,
                            1486725595670459814,
                            8838712029435920939,
                            1750787135667016644,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            173775623303356468,
                            13685044199321349735,
                            2420029312353719584,
                            2548511872438503932,
                            12485350283164324504,
                            1412544822587096416,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14646315801613413011,
                            17771118696092637093,
                            5317808000225717795,
                            14145343188662143233,
                            7131334939058092051,
                            352453884047204994,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10868433151101887672,
                            17156083443970819614,
                            7937247232170699227,
                            15521482206834836952,
                            3773424494027551205,
                            1552632014402865638,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5524756812962325798,
                            759361929155796108,
                            13895715420426182238,
                            7628273294216716924,
                            16509457081153723877,
                            718879206939213122,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13898465135542066713,
                            3617188092800302218,
                            11603146641614077174,
                            5093621330290258596,
                            15759157324378892775,
                            869569389104932276,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8758567428210609917,
                            16885678526976584956,
                            16081377099376555192,
                            14278380459425722918,
                            18159814850406977144,
                            137695384832388439,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            268086193690876728,
                            16552091092254286031,
                            9208806890186085224,
                            6219769294040895728,
                            12570012658957617840,
                            1057942020078076828,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6074194495596775667,
                            15155830299096206662,
                            16101259119622917374,
                            18373574692368155685,
                            12168111145619928437,
                            286167544723456250,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7271620752751665209,
                            11022802743216265057,
                            15447367355557620334,
                            11732155241026167283,
                            12138665303930527930,
                            475638527238766104,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1003180387311734873,
                            17510417030598624172,
                            18027063950908803352,
                            3871593532404139144,
                            16022144506179509258,
                            1749815762781805006,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12673066229103688399,
                            14497929189548215064,
                            11614549541672142829,
                            4595414922277152893,
                            12745465902483169405,
                            1320939607222272261,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8511524649749406013,
                            9794583042370336923,
                            379910379981699181,
                            10633307686876529196,
                            14852336526997721644,
                            1410777649445143467,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6725872407221267215,
                            10040174663012217669,
                            2291717355382406406,
                            13227423857207170245,
                            4822915509310985502,
                            1744811744579650373,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15056804215128145659,
                            8424517300947335052,
                            11496692345500075017,
                            4867542291787385328,
                            9635665559752848600,
                            1551505636687840396,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6846101186532675758,
                            13832619053719231907,
                            4162300891528642969,
                            16724383432324066393,
                            2319342748651941558,
                            1147667831838058659,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8292563806782349267,
                            8301790012444776127,
                            15602178319920051345,
                            8040500567194275895,
                            2678789450244414597,
                            137883935525887921,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7930396497054068950,
                            44428382134414444,
                            5068320004155425808,
                            9709961646554583265,
                            2961328667386644552,
                            216069978353891823,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2946753149072042569,
                            8668781307647786828,
                            6708734244997717601,
                            11020126333496898265,
                            16541571499663390781,
                            1474335972888519219,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6324810725764810415,
                            11944347055139451862,
                            14884795786068263010,
                            13180644548486102199,
                            12871920207488664628,
                            1085681213614294251,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4901222948037995836,
                            9253350513095170147,
                            13561492450749554099,
                            16103412106858822959,
                            9370146867679952548,
                            1544553875030823323,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4898660779608470511,
                            8493417877133092987,
                            13702835158872647855,
                            5711629723844529878,
                            3680958497687599456,
                            1060192575184999237,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            646236390499477086,
                            4073964563500468083,
                            4428385534894683636,
                            6094567376899658284,
                            16160071652588484201,
                            1436119850394769309,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            18372827396709910438,
                            14750869468241062585,
                            16916885318022012126,
                            11533924945759516796,
                            1810552611583028232,
                            688306370613501617,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18020249584119733868,
                            15195251662859124459,
                            12506098919330613494,
                            8951358825679545390,
                            7250500665836601182,
                            1056937672772883723,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17862094789221289535,
                            8157722400120698102,
                            5204392810542660969,
                            12407146087858028076,
                            17448914825075639653,
                            820009240697553156,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            878181651966555461,
                            2827309392871424721,
                            15155758762489813114,
                            3337655804304442666,
                            10699116750239460147,
                            541390124425843743,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9994874443423247195,
                            2844636514415479812,
                            6025651419422408155,
                            10485290748101659614,
                            9467968968044140391,
                            539250950017591498,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1249765444576997530,
                            10017444713599648257,
                            4751095969806092812,
                            5079089904441801672,
                            13818193275887051162,
                            1654378821906229147,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14032188997897238904,
                            2554773594843096931,
                            6507939392117790644,
                            5792980338141397298,
                            12976465480269139471,
                            1407532836390758545,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4308177722724292171,
                            17651314459951928139,
                            12188081093447004051,
                            10351271592578650234,
                            16950281205776374843,
                            1459617705166509563,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5297158950579240218,
                            16813055052464236056,
                            10830064416831329620,
                            13722196157006652360,
                            9246319595300847952,
                            953540093009264749,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2807918904136244330,
                            78885605839843410,
                            13332627947027212140,
                            5159415259991596345,
                            15262160841680523805,
                            1240591024803281091,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6675243272447077693,
                            18006057728550986540,
                            5699254238647723119,
                            5729672011656974906,
                            3228053882267489535,
                            1649062278536166704,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5129909721888071781,
                            9092752477352870802,
                            15985054705200528966,
                            5288304962789684010,
                            9811390915461325610,
                            624991152895810204,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            764061627737063435,
                            11510516090839090048,
                            9601667557087190466,
                            11573693605534131062,
                            102067636357319131,
                            137503257236096698,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6644837180613539853,
                            12947941454170514202,
                            2861669105865672448,
                            10489708839068375353,
                            14269234475900858001,
                            519237404386431967,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9224013866331906676,
                            2892554330601488786,
                            11440013869440333037,
                            17168828313346942552,
                            3616023035581198445,
                            1130003263090468522,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            666687315774573797,
                            13028193826424652624,
                            2386937359980419368,
                            12943231552974798183,
                            10522500459346234357,
                            1553146866505358801,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1641894446703109739,
                            9286896584746457935,
                            4384201782494475012,
                            8896773716103273971,
                            9382694630468641237,
                            566984219624798391,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16624347579174681068,
                            562048381020155130,
                            11083198172646851512,
                            6885565830775795432,
                            12579149521723374926,
                            656140515921878116,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16914742315272516182,
                            5531747554354068516,
                            16229508061238155172,
                            9048050390244196234,
                            14991672568044021684,
                            1290259648009982280,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5226989928743103098,
                            16131830629938135745,
                            4039110593285500800,
                            8673398258418086697,
                            3100510133008675430,
                            405848920466426091,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10181418879944970885,
                            6684820937880736983,
                            7313921032468244000,
                            6265814168958053574,
                            5710751572578221014,
                            1635068360013653978,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13420978024692612304,
                            7815419477779242260,
                            11041162735575108298,
                            9543151368073226569,
                            12234352784053392262,
                            97895999631442828,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11751607905087320461,
                            15075470721276793527,
                            5327318946394480782,
                            7983921436713469322,
                            4749890618825080382,
                            362663937458763628,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8319987923319934803,
                            16962096837626352269,
                            6809866037678754154,
                            2251085435364045154,
                            5351422558429667625,
                            40501398144649535,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17767983710528978174,
                            12011908915214498520,
                            6509087700473837418,
                            11246970329614441176,
                            13788975799652838531,
                            142433475180665600,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15575260662712619165,
                            12726649802934624943,
                            15371645751813557811,
                            4207986984477332247,
                            4402016633270072750,
                            1392418701262055534,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7681016974762568276,
                            15617853746082924026,
                            12884096685924261297,
                            260028401916560442,
                            16113539469786799362,
                            1267388871127035424,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15858662236423574194,
                            15168847970851621348,
                            13499610817214345078,
                            17037511167856034412,
                            7765198300560903264,
                            762290233927243870,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            398162318930918265,
                            10251628751891029819,
                            365985343748180913,
                            7598362485699337099,
                            192008833863958468,
                            1032900520324347542,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3140671486602582309,
                            6234278285144488560,
                            8765380575469104728,
                            10546450695513673585,
                            10329441293696541164,
                            1815620277527962398,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            358270601505035525,
                            3547525556145233527,
                            16825145467500526695,
                            5058817127919811310,
                            1272003960759669079,
                            1692101945509907750,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9404700040889086876,
                            8141526065324572373,
                            4560916125519073282,
                            11115280773450849081,
                            15443742421520664662,
                            786656674637874126,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12494186608837715722,
                            14196819732517756777,
                            8548709630800169977,
                            4960787536724909242,
                            3476766208061539513,
                            1450346975077714733,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18328845712550239965,
                            15841172226061905630,
                            7102892710140118173,
                            3827594088236310095,
                            1484947220123014600,
                            842799528108619562,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17882837560489413801,
                            2697631512084633209,
                            11410285113819312652,
                            1367337686699996954,
                            2331690271697427860,
                            1633454247177195300,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6383211374867810791,
                            9310199691179705028,
                            6803773844064978849,
                            16436034629295528810,
                            5602708608480051164,
                            851894785183603651,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6337636100558526719,
                            6270024689566462190,
                            12744202703652335020,
                            9828765686012287258,
                            11042857824159970736,
                            1641697590735289948,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12755059922844064732,
                            3842933527281312964,
                            6840175547142643679,
                            16418054444668630891,
                            3642026116628954712,
                            1075010357252250328,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14903373931043929072,
                            10771971962638112797,
                            9928694236991379542,
                            2546665444140090579,
                            13168896722182768657,
                            1524810020156042653,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10690506755237789145,
                            10351863493084944117,
                            11442631661278390472,
                            9717193028182849726,
                            13649538716391678258,
                            607830192622370534,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9487080889684668395,
                            3464994054160352940,
                            10305915541408335756,
                            4938380227639577395,
                            6318468910487409971,
                            1220246988737233180,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9211704606407539782,
                            18411558539468557717,
                            14091712120718188147,
                            1234074669490960733,
                            3916394679396752987,
                            1608104988263108785,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13611229370564482638,
                            10356037101482062985,
                            18432309519524730782,
                            10072615660586954190,
                            14984675040598092684,
                            1198111650269129082,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16862835211525477682,
                            5614423967069705898,
                            17994304290237003296,
                            10185206470027975238,
                            11010200881522993337,
                            1627366138296930208,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6904716980533169936,
                            5522083241173840405,
                            9702206955325055086,
                            15386622343724327424,
                            15377265206550190832,
                            1291288146596392315,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1010581764698583918,
                            13477453595753129860,
                            13517786003285137175,
                            2698185252897494972,
                            212568387045989582,
                            1191910288598618897,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3848498151561825752,
                            7822503973597565030,
                            6076254401756865354,
                            10847735078876141017,
                            9883377075078835483,
                            531339841517689659,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15417001634945680516,
                            11127687097390465066,
                            5123782143826954958,
                            14518732392918561288,
                            1517478475323129111,
                            1038247759636850042,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            822254545392158291,
                            12578612739097400998,
                            6411611333701254368,
                            13930915737130650829,
                            3407680845962003572,
                            525476616784260925,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16513733824565017109,
                            15951826826584136922,
                            17431517357618551334,
                            6638611513889522261,
                            11090893733323217409,
                            1143245975447070658,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9509197883597843726,
                            10264415293609138910,
                            11169371642408428745,
                            8205885982503875593,
                            13620635475178221238,
                            463585297552670128,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            52414919677958177,
                            16571108006939050049,
                            12959120386470321129,
                            582818224032364702,
                            16489694845019446899,
                            1758328501218486591,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15348154220439911124,
                            9494351055001936847,
                            11413428706166584797,
                            13705531394393840798,
                            13967747373993694084,
                            881170885172159416,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18082041377437735119,
                            13945802348275421927,
                            17155560746771218274,
                            17355907709005851303,
                            386671177415760043,
                            1101405870256554084,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13325333665028862633,
                            6463226842578550044,
                            10945385118157632114,
                            16679699629039922103,
                            3065001301813491735,
                            1655531964513841798,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6620042783646048429,
                            16807165034734569982,
                            17160000155737123332,
                            10073418752785981837,
                            11601104169383958004,
                            997467076302752425,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14618460784391714582,
                            8072192410838700329,
                            4462103291702715031,
                            6451929927910728998,
                            11771107563387159558,
                            152431503207452814,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13467343394026015535,
                            14519960211291222478,
                            6116364849734714382,
                            13688398542554134706,
                            9053392718483088865,
                            1167462993733720311,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8448581603188355159,
                            13266984937238089206,
                            9657442967513657034,
                            12121249192315444912,
                            262245123761882561,
                            577374942525312668,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2207214411818758996,
                            12654130473268507617,
                            3977104691942184916,
                            9024838205932948774,
                            5451590555608660786,
                            1863687486026288386,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7107873856129714547,
                            17198252256560057250,
                            15284848822223980465,
                            15804928380113376675,
                            8366936326304800324,
                            1373103283356059440,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18110536828345177458,
                            13624472210239331647,
                            6689361682488940956,
                            18257864249249281757,
                            15534349310532231078,
                            1422021768962839412,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10767787872910370574,
                            16890762820329617951,
                            2180023298593083562,
                            14070312842136630826,
                            7419627981135724782,
                            394934622900887362,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18417889437257235454,
                            11565377227087440178,
                            16260541837755609358,
                            4485061219711800013,
                            12436044473951107193,
                            640976644042518311,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1187857770421444303,
                            4395428741753565192,
                            3479508211556087807,
                            4905145659377949630,
                            6583122214595540679,
                            67742927980383264,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2871403828040574063,
                            18259484367684264968,
                            3444570270062548031,
                            11802114171821572119,
                            17600153571368458436,
                            516637575294275423,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12471098795256608845,
                            7129995268381337505,
                            8447252122988299941,
                            12342679509488972842,
                            1227451083397175896,
                            1794342206557930553,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15916857954501449044,
                            10803379849704067859,
                            3694701591275356307,
                            16969385224350841383,
                            11373233374542957317,
                            1352367240753911528,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4166244555391574630,
                            2453653214205019198,
                            3379008268327460607,
                            10929836250155821488,
                            14388768655822677364,
                            806153207456434096,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4315866960841088046,
                            5560969776414603372,
                            11045266946809663535,
                            10335775319471925580,
                            12690746112265182045,
                            1305145106642252790,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10582461693322713833,
                            1421651495134135589,
                            5673218480937693329,
                            10867079431292804068,
                            3642304739188457523,
                            1728810185918808504,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9848376785873128031,
                            11177701484982686262,
                            17766589492701921194,
                            13067857598859585315,
                            5985778829411617190,
                            1260383003902580233,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10024946729293106072,
                            16738883806082697691,
                            10010930609323866105,
                            14108970806662829783,
                            11343723084797541490,
                            534515111028459029,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11523738319262622957,
                            7740257256863538759,
                            6339744049776731506,
                            14840741166519676184,
                            706542259598889080,
                            991701450459139222,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2314115340595819600,
                            13898325771599312239,
                            8952213961262281210,
                            2319843824649743478,
                            10119258365144196202,
                            1496606691039906840,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4569053027382756022,
                            16808542297769161495,
                            11644180451975136520,
                            1544029036923589818,
                            9076906934895066721,
                            309588988614570138,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7578356054780461426,
                            13617367227855072229,
                            11194453660702040383,
                            12590510554864058734,
                            3029438799499073408,
                            1426436228144070707,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17283957384820237353,
                            4494835591303435971,
                            15539940631737173776,
                            15168686384549870790,
                            16475285860385070976,
                            1006161318151870235,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1478816007717340746,
                            14014538796467402905,
                            12336091508940659958,
                            7031277857736566963,
                            2854421217255070344,
                            362229673411298044,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1300041852819733258,
                            14621108287456610273,
                            2728568006292171703,
                            13300257463201020955,
                            4719439211793416304,
                            1801073788786922345,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14756481427183722613,
                            2236459028675075376,
                            800934356683803777,
                            17166474772945626109,
                            7392735296653440149,
                            292116860272170572,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3529181534332451079,
                            14785028590225652542,
                            14029703585631357293,
                            6515223415824637933,
                            368400046391788290,
                            1490832248585583538,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5824833846548848848,
                            11395942507388126087,
                            16504216008016843127,
                            3107068389176603865,
                            3984160858027330871,
                            1547630685041163480,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2550897000308849200,
                            4425918360010079498,
                            9413492900770120144,
                            10521469855269549420,
                            8789118225241741635,
                            233087270016588230,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11654130807560830053,
                            12365293757958425726,
                            9113786441048714063,
                            12513266310954477565,
                            11238810873360029693,
                            268563522197316187,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5761559293749561305,
                            17429803534604531027,
                            2171425178302867056,
                            13297820184467406768,
                            15287872665619137041,
                            83232806340482704,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13308270199210710103,
                            8756922041902009430,
                            7217587539036245026,
                            9757693488256261677,
                            640609962995055469,
                            1700750094725999395,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8789782568823966391,
                            16554542327134432683,
                            9942611966892757186,
                            18317797438343030860,
                            5426573831258621048,
                            1774077237922160223,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1971903592417757003,
                            9051380446086213612,
                            4879225538680534183,
                            8000843671497682360,
                            9238958824761543848,
                            1022887598423128364,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16950704595241643804,
                            8454367624401995273,
                            9641663680129882690,
                            17482647574971164306,
                            9457701009293008259,
                            1377874901933196392,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2628553885832069024,
                            9323050185443751273,
                            7797393169380040750,
                            12259715252507814163,
                            12243141725798862591,
                            1237259886829163513,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2019793253152780762,
                            6208584158529324747,
                            4156915506134058339,
                            1051781123635220400,
                            1675239580451457498,
                            373864107849704445,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1135526659102842556,
                            13947618642946041407,
                            17143955487804976059,
                            6599715072713151566,
                            2831343747770372790,
                            1360583162082452246,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2420739821469435949,
                            9572031620790523894,
                            12705897566129679198,
                            6263459072032811217,
                            12398224324953892060,
                            275953161232432981,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1807449594497835798,
                            12072039966873120184,
                            16027980004702278365,
                            5647517832333076176,
                            5224296596512541178,
                            518623419643079104,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14450819836747970954,
                            776908227960242261,
                            5036100075309736420,
                            1748971143197513910,
                            11403707966513290413,
                            1716098299983364952,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16470746187821406250,
                            12477055931596929283,
                            1439195669684951613,
                            11835205415092796362,
                            3769621740155449050,
                            1174058899073253899,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10361001716200388234,
                            15913774155802301939,
                            3832179598560421642,
                            6631658669006131562,
                            18231162779254170103,
                            153978153231651036,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2532816590532388285,
                            18418007074623057836,
                            14876379914049171252,
                            10949798261286706323,
                            6450199992500703418,
                            1380866928858766288,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3505409636955575335,
                            11659803657327424287,
                            7928183387101122045,
                            14432429507089561837,
                            6747119498675511352,
                            1512910714180303210,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16072105086373277838,
                            11264716059029615762,
                            4863790618384846308,
                            295556596306197760,
                            3858927471229060521,
                            1857554617651525518,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9709323546952538708,
                            6128731605552925548,
                            5193807813976663028,
                            6440271142981778710,
                            9398016188601800179,
                            72280616411116262,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7308037641079580339,
                            15989426284036336247,
                            2283786473773907257,
                            17880162482149852100,
                            13728632358043475279,
                            33211343759004118,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17709760115528776667,
                            13458947110973479089,
                            167375196546327497,
                            18337240012879334191,
                            1617865131254012135,
                            1110587551801850192,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6894739809028020557,
                            9954581358854027137,
                            4505957568602194850,
                            2372360724766210391,
                            7583674635909314149,
                            521858049319865698,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11751633987849725502,
                            3824150350611343807,
                            15763953394925035681,
                            3967431297660772651,
                            11554991520440437079,
                            342747916109663704,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16269453512264180167,
                            8284925182658806340,
                            7105068912916409864,
                            6846470837196586123,
                            2798785993790409964,
                            1535776915658459113,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1347405218661338308,
                            3616705963637136268,
                            11808621222802927910,
                            1977790853925079136,
                            12474755774999526375,
                            1682566796545772916,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10970491095365911776,
                            491180423110296350,
                            9145625143541587813,
                            7780656760370166253,
                            8609508757375954535,
                            1074763081940305279,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            273302092082609265,
                            11288320152427334948,
                            10196786423941273766,
                            10398531016481890397,
                            13885486983667773314,
                            1406536252404748128,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            18163459427631113928,
                            17620650646384238368,
                            15242494606726597733,
                            5124676651866623044,
                            2477312505047751882,
                            1478393518682627551,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2635816435284932509,
                            11193841437421194152,
                            17655621614795434407,
                            905881093666135867,
                            14732045110343100699,
                            1260179273723144543,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15531923520776689042,
                            10939417411215210875,
                            11886581412960953088,
                            393351792223882709,
                            16958872878598443557,
                            1840689855787161061,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15411677510323232837,
                            8011416456004692033,
                            6451056347125552952,
                            17826317509324730949,
                            12979653557694797228,
                            395891008762203795,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            902243416528007788,
                            12307003495260170888,
                            16941368428221203558,
                            17093763012717962522,
                            8861107750417129037,
                            1845652010162269993,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16702302197264388687,
                            13329784506862280391,
                            651190366820980542,
                            16954646127597446905,
                            16372374756950421112,
                            790528752435626277,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9832582955612742606,
                            16713664150433920230,
                            11630601031307185658,
                            8225624013403821048,
                            5533108273241750219,
                            488870696058317489,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7093757205524807374,
                            16115333825248257926,
                            5432286634366508181,
                            4746549143406321796,
                            10198645512208316053,
                            1463351733819343525,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12562653642027112829,
                            6153133235628158883,
                            8503270406043932801,
                            12133093469160874603,
                            17490921749119206133,
                            1694383758542141690,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8328307160671734367,
                            17025186070726567227,
                            161391150224795321,
                            6964690415163198216,
                            18179471273604771651,
                            742999165608141064,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7138840994162046524,
                            18092231953034477841,
                            15371600124197746589,
                            8036440365446923086,
                            11822833741850143712,
                            427939719573436143,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9866847913847843239,
                            2874011828241588089,
                            5411182748451114530,
                            4056300487272846036,
                            4253559059574052483,
                            1310715453106327508,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            921376483356364081,
                            8904855143264868467,
                            5131353141241161696,
                            9338863313790827235,
                            2448239921263245284,
                            218736114241983843,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7877317291112105085,
                            6383554603033964313,
                            9314087459312335304,
                            3827966683338148268,
                            16095519809653084297,
                            786683255175591043,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16717889742354187264,
                            11621793570749585829,
                            2979039280760479685,
                            7558335187962575611,
                            14665134273371575317,
                            1651842112567288734,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10230581729368344345,
                            15017446822880318685,
                            1352520305374122990,
                            12484100770319536173,
                            3157752342743161442,
                            1543267160227254990,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14066274715231130710,
                            15471623429652580653,
                            15910143342803994456,
                            7675753009521167797,
                            10801262376768541950,
                            215532025390617006,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7513852181006365661,
                            4347005562308066540,
                            2330123869934145226,
                            4148169022910607011,
                            11803172665265736828,
                            1681999729986028127,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1686253487944201345,
                            14459263991888274587,
                            2837209461939537978,
                            9562714092221938966,
                            11810942071253350769,
                            509386285145095090,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16896120013345922063,
                            2015412759505212505,
                            3775655609227193108,
                            4306189755620296098,
                            8810287271482684867,
                            661570206385308111,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4943868888310011873,
                            640553403676780318,
                            2272312131426990210,
                            6674336632444905071,
                            207193693057578159,
                            574494455884395580,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13444301857312211944,
                            13307653527912892756,
                            8259656047107840248,
                            12241408775735668264,
                            8646441624791105022,
                            164651162342091548,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            2914147990781397770,
                            13464155694463248656,
                            7893943030204674795,
                            13878599743243780837,
                            13984194953390596069,
                            457715547300131728,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15968752406305239378,
                            13196173160477609205,
                            6829167694882396160,
                            6399562110932456156,
                            7112087057133942162,
                            85359274720893104,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15821679821104304720,
                            13661630146772471319,
                            12540702131337096500,
                            3187807835295994566,
                            17995618927156352188,
                            156140200610092966,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8880351356837985561,
                            6077554441802302267,
                            12567493849486885145,
                            5688947788839532304,
                            8324143845537458856,
                            833820836060649643,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4725217272230715017,
                            11166057522677408116,
                            15346334818174374510,
                            14559767943363177738,
                            14526328453107513203,
                            1702697747204937618,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            999808702288960074,
                            11653537408947295076,
                            4441122851485248500,
                            6669856488833190039,
                            15056649560122044917,
                            600094219550012930,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11016139238069432380,
                            12886053514613261224,
                            13996042521099085741,
                            6684507750598416381,
                            211236594472132863,
                            111324687765614382,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7827867860968150508,
                            13709715460588626967,
                            773127136413142573,
                            9866229592328774359,
                            1995958895942128737,
                            1577004374164239863,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1487089895634605300,
                            12329824240642153694,
                            15717799266468408322,
                            12268452327606578417,
                            16957960556059609112,
                            1840205402176869410,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            273940847073639940,
                            9317711995771492933,
                            11939310581435867123,
                            13384257407375693468,
                            7379720874751054366,
                            586920359916203135,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7733087565519903034,
                            2714702537414862647,
                            9001781259792699600,
                            8670452907289762856,
                            2674476231026785805,
                            1510535181681380993,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13203308686520666544,
                            3697976354806753395,
                            6819886112471389274,
                            6776341440202676184,
                            18189686687518441333,
                            851284887294549976,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12747898024142236476,
                            6951599814704662131,
                            11169010306090335279,
                            6879691424378406656,
                            7652253764784521238,
                            253334100448444524,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16446676748251266104,
                            10082276747657428134,
                            4353878644809993783,
                            7516958613202366348,
                            17042291501041099643,
                            735942598773117833,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            18131859437184299635,
                            4707500528900736502,
                            9173972144964420486,
                            2217797159770472244,
                            13505583623193286023,
                            1278487444518123476,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7235387926634379098,
                            7506553406449065154,
                            842733387743077064,
                            5604021012948211229,
                            13627376142779118090,
                            1364465935712553921,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1239791519728426771,
                            13626757135229156293,
                            10124840788609655278,
                            1735796870948222103,
                            2551874351407039554,
                            686613751297443281,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3143680729994791809,
                            1981199770166695218,
                            4742256039787364846,
                            16378087098423789095,
                            10670182922377613692,
                            1664884387466088584,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4868629058947666502,
                            16403082561971213430,
                            17110385730293127081,
                            13990649526585914687,
                            11068517966108566119,
                            482471922991500900,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1640386564514583591,
                            17706262096160842184,
                            11423531618934968565,
                            7469177445646766004,
                            12190046234484419867,
                            523257893686507424,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3314362024773756315,
                            3157458906761065275,
                            9949531380843452482,
                            4420517121638693627,
                            13469709788440962749,
                            1582551223257480893,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            850477398521361634,
                            1226024139617458148,
                            14406695314341069160,
                            9690753824858676077,
                            8844003618902991368,
                            257209171933590070,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17599997595449443990,
                            13423466953922788131,
                            8464899228622968383,
                            7886824424197144308,
                            5884380370285466669,
                            1633756787518280552,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12488511013179109847,
                            16182849604887475606,
                            10869569917260546107,
                            3883906897773232022,
                            3786773448390666059,
                            1715816396981428705,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9156869255291287657,
                            10758605500044185608,
                            422553296392574026,
                            3150952449875928287,
                            6253737991241622494,
                            177410464059451242,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9203355707417622557,
                            17673548030136837312,
                            14829851992080343769,
                            1489448167137871810,
                            8156160076446036564,
                            664057040378237426,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12238853554802259048,
                            5401570296405622550,
                            12775672749111861713,
                            6451600090633202818,
                            1299844572645007964,
                            471782713435473731,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7116673912154464983,
                            6890666270866097154,
                            11523525191668709372,
                            7858688801986355354,
                            6691342903745152029,
                            1272800137069087603,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8147986179841197490,
                            7037834274532145888,
                            16513667397917687242,
                            3684753190796303960,
                            4319291724066268920,
                            570638225356986820,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10472758233008575058,
                            8203407871264495081,
                            862273195455646918,
                            12296962400093884365,
                            2433941693624358179,
                            416427949986411059,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6346768162266040504,
                            12908051617675404232,
                            12015246469850114469,
                            10360310874699368184,
                            16277727464737162303,
                            776277140478594879,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17132544051798434927,
                            1502480067604325507,
                            5675558064925721625,
                            6277661675936686087,
                            5448140404250629557,
                            1677148357499647005,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11472140242462146808,
                            3028247269348704778,
                            912100754944147963,
                            10520024616482710456,
                            11554649586339460021,
                            729394326686031929,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7849952756476637454,
                            14836128991949242091,
                            3164799842095240837,
                            8539118838606103990,
                            15417600034333421456,
                            444714657939925424,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11897685078271997898,
                            3979793691320478104,
                            10635814669099757003,
                            13508541148396749490,
                            23456926366438676,
                            1508789501131098114,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            725537276168582567,
                            13279020612273236920,
                            8600503519229662845,
                            2177687990414282083,
                            14481720689263504093,
                            672717684785105856,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11877488030569984235,
                            4606734668180044618,
                            10200219210635676961,
                            8754906850011424633,
                            12241457073894028662,
                            1525390095616836834,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7601639232399030014,
                            6753710675237660646,
                            9794594093496268882,
                            8211575312494972654,
                            12236998915761720516,
                            429376505455146738,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13455466768492496324,
                            11933138028659254740,
                            5742193169261664739,
                            1563937210027874184,
                            12627565421707921683,
                            7607375195881987,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4472628154608418474,
                            12450401936546064011,
                            8906676456411017829,
                            9054091377596680601,
                            17715119733273911311,
                            666426954173471519,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8668280722600209713,
                            17005470759847722974,
                            314289608437059049,
                            10120739948457627043,
                            9469830028873656960,
                            1435056595383618816,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            14307937008088431853,
                            13928445934998666935,
                            10639493752946613990,
                            2228189168206579526,
                            11194494826006038460,
                            1697434480657476869,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9156021816844158820,
                            12952379316085467898,
                            8052544463366476366,
                            2423485466288300663,
                            17992569736400566586,
                            1344230883382702091,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            463349153552070258,
                            13180214413375236763,
                            1811105006019531030,
                            2086652870695795810,
                            2804160736430674653,
                            271255611018440993,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8074036664553918131,
                            14504155619738471703,
                            15955230564717103906,
                            9926205275366045464,
                            15300967417538591191,
                            826894316073335139,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17712605827481588017,
                            9409548113693686589,
                            15893566436708991972,
                            15464681893979410272,
                            4097430218971375579,
                            1078521177403778777,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10667412366341817961,
                            8800423225125185141,
                            15313799842929688,
                            4347341023889040717,
                            10925522359050011639,
                            1085379683691922914,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            18139988860053041321,
                            6924956073732397977,
                            3332474035774308525,
                            7525948743673931197,
                            6530409666150962961,
                            648583640087129521,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16708692049433933452,
                            3704362550268671315,
                            15397897714466331873,
                            8796601966776907507,
                            681184695804999468,
                            103773565434204354,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11707492670116299961,
                            16235969377071942358,
                            10604511742965759751,
                            16772824760598781859,
                            13533709776709192242,
                            1255064333890870785,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14002404230897725169,
                            2048840499265678734,
                            15819518809772376274,
                            10789708341389556526,
                            1277665599617264290,
                            1103357742108997903,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10517125507059116897,
                            16806547559419822641,
                            10877113035762219109,
                            924542467244004018,
                            3066828346619845599,
                            138250838830239327,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4606893069377441602,
                            14595932593423141097,
                            3162678333226302647,
                            6614283208277980524,
                            4505995971867519283,
                            1560847878469161637,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2989462488832847337,
                            6014060526861555223,
                            6034898806531525139,
                            17524522156494237629,
                            10475882534618484881,
                            1815701618185474125,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7034483930114708060,
                            10673098338846687236,
                            1976827414659344154,
                            4350952234990922470,
                            16101002989440050674,
                            615607586626774522,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5730332253645086288,
                            7258740662575138060,
                            5697092052406480296,
                            9632902895066875414,
                            15535204966655247582,
                            1571136775547144247,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12899067055708288314,
                            4443256329076763665,
                            12642170914166340899,
                            6901977068799990571,
                            15620689885070924537,
                            1130553614992758459,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10857485131992628584,
                            17503751600886613249,
                            10303782554794108357,
                            8407645970255525190,
                            11536477980761550447,
                            20096210566812068,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8456955960483856687,
                            504119307649462889,
                            6738680323214540601,
                            1060325156758868224,
                            7224629145319592707,
                            1498193062684977843,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16345753462906147838,
                            7498383959421992279,
                            17029831935643470086,
                            13708612483506376477,
                            12450288622624091964,
                            1790194938355402421,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1083158896825713243,
                            6021946106756788125,
                            4469228809221934372,
                            5863031858435639779,
                            12478422863957141145,
                            1404103260604143351,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16109728002770477265,
                            16150876314766411972,
                            9025248167612410203,
                            1702552369082553207,
                            2456830397440131140,
                            147883535557300333,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15741594309865198427,
                            12844824008626352908,
                            3396890556579778846,
                            7870983673109773650,
                            13267348792570502130,
                            1480636970106159031,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9146931771288393191,
                            4917759796730437231,
                            13774538674547011708,
                            2412369411114492794,
                            7532143508750199827,
                            1715653920503912371,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16733551056434501112,
                            7758181290713139029,
                            17274682555689820423,
                            8063340014223902367,
                            10374722249424609317,
                            1532636191410158852,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7469983737984262576,
                            13444162346299449071,
                            5640093396606158052,
                            4930134741729356561,
                            14351178775664990289,
                            903744531402739160,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10921820192105236531,
                            5710831249936970604,
                            11883553636382379495,
                            5329528963048660501,
                            6389474642049074655,
                            1227183968935239089,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8268458218560045735,
                            1247747948912575974,
                            11309995967745110659,
                            221412634410368068,
                            10511326602031662205,
                            1356672976286742493,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8851811869251792960,
                            16852567963343802301,
                            8477822487198921232,
                            8657015855325168748,
                            8356960315330523120,
                            802256233794573856,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6838860473957403922,
                            12313877437669464187,
                            11153687972978960664,
                            17520606900287398684,
                            10955047001174645291,
                            1277185041053943198,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17974217981234414389,
                            15760668031679006843,
                            4473082252882836184,
                            2472343102452889228,
                            4217629566377302317,
                            673939370591314449,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9547197942153139461,
                            14599218097482818187,
                            2843424473587308532,
                            4226145058210495005,
                            10528402275553705847,
                            66122988350080075,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9174728851225898486,
                            13098075324649165694,
                            10439813280218057912,
                            4149789530302721259,
                            12467003151662508860,
                            559280030928336852,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11925776125368288844,
                            4594781337306308900,
                            10343994527611316053,
                            5321523789209385989,
                            1468639944565805316,
                            1716945381951747461,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9309706874336969640,
                            18249156080332192890,
                            7400324714245133725,
                            4600715183174911203,
                            4434458543846820965,
                            705013663117451900,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8161792535592809111,
                            17180321596014781518,
                            11110225832036956334,
                            12794711412350872121,
                            3998086829560658135,
                            410530957324496823,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11158746874715176214,
                            9208255443169355771,
                            3525806261835140759,
                            9263604495479796005,
                            9267449553772993635,
                            1663084033262685090,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17230894163406706274,
                            3344852640084939849,
                            9408400016641730870,
                            9520580550898937799,
                            11225681528642350956,
                            1454837095430890026,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6465044866426696949,
                            14736799700080398479,
                            7329547077411965229,
                            13898441080210442724,
                            8586763158249649979,
                            1272685349044293355,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6379549295073248583,
                            16758432761952000119,
                            4938155452083238053,
                            15340181483282266020,
                            3150190246406712413,
                            1521258000049588256,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5572866548566351342,
                            17927456433915292456,
                            7591578908053469326,
                            14441693502703011309,
                            5804666988297494132,
                            807982453740109106,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7571107375264872895,
                            12730319594500725330,
                            14640450400108196727,
                            16281298608282915713,
                            15003677693930998301,
                            1447523451147133719,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9308475065587204712,
                            2769831683736697891,
                            3081672277450570895,
                            12710300441878610747,
                            8736197740052817053,
                            1082428885685039865,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4053285363018068426,
                            5295934290818197343,
                            2359295125788737404,
                            11506207648153771931,
                            9453312492117080046,
                            1522713550491505863,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6530972073717980429,
                            12743165011542503462,
                            3789082390943235041,
                            3957079655578167314,
                            3633564338851865523,
                            1058318646105736954,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11427697266105918501,
                            12031999123617252622,
                            15152335280862710845,
                            3577049759309012956,
                            17469102907553288838,
                            173547675806560419,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            754085458948058147,
                            18042240791334696580,
                            7121536542901390847,
                            13809093444230709261,
                            5463424614508194633,
                            1629764644556285880,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17994310442554117752,
                            10107427397169532768,
                            4025118507520359153,
                            10835594492169326674,
                            1448813335077624154,
                            333241649140391369,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16610706687903488718,
                            1707580806403275446,
                            728494585461945183,
                            8148665692689227089,
                            5643358409827969455,
                            543217172092246273,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5776130648454740791,
                            3255482387510895526,
                            12847710318252084872,
                            12418294285422967743,
                            17545714928967286236,
                            280833451084954420,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6198987211828323893,
                            4888250708161812321,
                            8966130244611407524,
                            4860040864074970623,
                            6730031969028530820,
                            1330513539224022817,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4534817001571825772,
                            12595716462508300135,
                            9245703737032421130,
                            16555921426334908167,
                            9149713329238225679,
                            317279687175405929,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16563925515999609675,
                            10531842265939216597,
                            7924226105056259118,
                            18194235837551438752,
                            11969680315092058080,
                            779855209608535420,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2285333833311126570,
                            18414073996087285656,
                            16510222202050493027,
                            18045308875833994807,
                            7608031966827369797,
                            1446431002718582921,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6253016062559400158,
                            1768983528656753799,
                            1015339732087435126,
                            5228119126441997756,
                            1634425212234871080,
                            920908828571853283,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4500701681989414853,
                            6227194851926023849,
                            17018201798777582441,
                            5766400526536322294,
                            15759904084295137794,
                            1125285327242384625,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17369365987605149548,
                            1434713199030345536,
                            7070005886771227291,
                            7905328907727893782,
                            11803141667835592928,
                            1017833926334691231,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15157767008353352235,
                            988448996769034773,
                            13072216551743013591,
                            393705439870790308,
                            15781442629164416747,
                            142693179101010845,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10663000213834161527,
                            1460073168411595726,
                            5540896151500075742,
                            14684908760323824929,
                            11461785783146903473,
                            1857823597189898311,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7358969635759224566,
                            11841563867005616098,
                            16344779448622422489,
                            10472968046889294191,
                            17546668645014862916,
                            515919024375442511,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16955515099411435689,
                            5067337249753307921,
                            15583505317958350893,
                            9305526825430761149,
                            8100831237909139246,
                            582867427597403813,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3681257299493616733,
                            4826659180959162233,
                            12656298562926018191,
                            16792924134630657021,
                            14651615011977838950,
                            1565335433877586795,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13787786996709334901,
                            10975114728941621457,
                            124584769900005169,
                            844059334191713034,
                            15939624550526366751,
                            1463726640250757497,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            4443506881466555460,
                            2956392977786541522,
                            8220689090610529475,
                            17357507452789258642,
                            11868148611142839928,
                            178794679203704454,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14248296018633900296,
                            4126975261765305621,
                            13827746014159220025,
                            15448123387090998618,
                            7542794996450245000,
                            368205065738235697,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15720122355877725430,
                            1582389844810680393,
                            2199527486499290972,
                            3016405396344309126,
                            11180079832262800109,
                            1030886821834642714,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17796355467397790947,
                            12187717252760210269,
                            3447012880927566941,
                            14823908506665314409,
                            14480765944362515404,
                            363107683874242149,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3548281729376862672,
                            11311470420872033740,
                            5302547431284724156,
                            16835127487048093637,
                            17133853903532575805,
                            36712461652184054,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6698401212490142508,
                            15487762742334237664,
                            4984389744721378051,
                            6009671930720933469,
                            17157258401101446749,
                            1724686059218506474,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15844237932345543796,
                            16186298051791839228,
                            16566643060018305018,
                            15276968832228345858,
                            16620567767955680473,
                            650222433851520260,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4785564359001527831,
                            2909823080243461215,
                            2753169488332247032,
                            114888029710444669,
                            2030653512856828996,
                            549214194084709772,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6631350357785754962,
                            16666751545913807032,
                            5773599724759850454,
                            4048931462191968268,
                            9396947479687796223,
                            600370974478135790,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17960576473717921540,
                            14512205177801195977,
                            14282583459249268121,
                            15530496582300288464,
                            9472491823924114365,
                            1050571418627084176,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            15211666919795432067,
                            8910028194059406707,
                            7937715742839600418,
                            5620794799648192753,
                            7671348663241508550,
                            1131803267913424091,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            13275786633886416347,
                            4711744178323869697,
                            10776722311107389904,
                            1232839793409698212,
                            4321320686480936824,
                            584456672493082073,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3040411644167830961,
                            5860445139672575436,
                            10672887391475300698,
                            9652017140410942632,
                            14894169694358197969,
                            979298791077723251,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4560400612561866517,
                            7843096702520040146,
                            9382052892317184037,
                            4927770526942548355,
                            14211807329986844328,
                            1390365408084295992,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1212515474621441072,
                            17499562995772434632,
                            3281603427005694766,
                            5681209058719508225,
                            11517712904912006270,
                            1765272408863006524,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14317232285583823062,
                            17617236288766523936,
                            2490656453904653705,
                            16399230600342409727,
                            10504291162066533115,
                            261596799080919077,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            16735422357982411172,
                            12328870570985204695,
                            5462012185289186183,
                            5063533723672183382,
                            13122159980109442292,
                            599097654935426055,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4636258825827132770,
                            8777126499509071323,
                            11426336185763660458,
                            1323572677627443622,
                            13178701509642075845,
                            1847073006776399214,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3703435513976449794,
                            2427920074584593681,
                            1187001887440090597,
                            17929725113317508096,
                            2553586082896065831,
                            1051895174957439116,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8505375425920060127,
                            10760409971215563764,
                            7396904385399139461,
                            9398730898180591323,
                            67274873258629391,
                            432643262860193069,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8889410584014077973,
                            5057374905924467754,
                            14535209226845822288,
                            11809906469035282107,
                            11835029918342672279,
                            230949379007949368,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14385603525363571027,
                            8263822703552057223,
                            6906780269319869044,
                            12812025926571262286,
                            3329299218355856650,
                            902342746687733742,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            6381480965783195886,
                            6801982685934082584,
                            9552723459907582945,
                            7227621655766437663,
                            17811978417029793192,
                            846117289818615264,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            975726921832491674,
                            11233079937918564088,
                            5481001837594413369,
                            946276207425882044,
                            11599909668745395529,
                            1295288560779515403,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            628270652034388064,
                            13071978312974934462,
                            12268988975586922342,
                            1365403283267513221,
                            18403609400736329894,
                            1009240925248173493,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11826331766538704116,
                            11531226093389379047,
                            9111665245447751820,
                            8790339655240654640,
                            1936904705935344705,
                            924136873903360319,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12649374665513263523,
                            703214327952916357,
                            7352584555912960209,
                            4565915610365726442,
                            15154213348689850867,
                            1007834867915836837,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14693490133877598167,
                            8615264843200990998,
                            12556985069971522790,
                            12400284901901091327,
                            16687750692498812734,
                            1838204510003351877,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1826791330213159679,
                            11684022647749726879,
                            3108142437823195200,
                            4934346929776005528,
                            4604210943387210687,
                            1604572807670231444,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1462506450014411263,
                            655758990936382180,
                            6148871309207543431,
                            8714662321385374408,
                            1098599797420132952,
                            1060496077950246492,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            2563436361517803729,
                            2086304706931456414,
                            11715193541827493647,
                            888198487856615222,
                            9207084805341545155,
                            174027670248643091,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9827454925245405832,
                            2973488614315490117,
                            2461724470034388780,
                            6780842959972496884,
                            10622610979084267003,
                            353899653154493117,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12676626184817416026,
                            8292384690968177193,
                            17430451618179891712,
                            408055503507517563,
                            18404272107617027909,
                            1182778590049801251,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            3157712026389725358,
                            3544853804642685428,
                            4514459677514929764,
                            17823790537699770739,
                            12164392682423297797,
                            820216143636894779,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            13956408150969688229,
                            9451298002192353074,
                            9379724752019650009,
                            3690734344453798861,
                            9581937276871546016,
                            330273375435908480,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            15938800307609680076,
                            4073580330103918405,
                            4363739710177589984,
                            8789260225996336130,
                            14177230097139219620,
                            1660072885847588159,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11331303573192847712,
                            16203679867609638099,
                            4754011587778013520,
                            12915674019060459994,
                            1934468492811139780,
                            925523587156519663,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            14360131041954642393,
                            5556414396717581453,
                            16197214716364322629,
                            5415017693890800849,
                            16051715855394774104,
                            5926209182196985,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            10026595668372541439,
                            4882871068180769652,
                            3389278771160120158,
                            12876578977130711726,
                            8244263136501471427,
                            338836335078967683,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16978885488928895217,
                            16022136308687245155,
                            11833373713005267618,
                            14867193813911897263,
                            9555372035682819694,
                            556931412857622436,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            220660142471319347,
                            10920215827626514177,
                            4241488085351173849,
                            7798905176976138206,
                            8669723379235868712,
                            1462936359634599539,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            6243503383328877837,
                            5727578834340876985,
                            15411217530341751300,
                            2353084929975756036,
                            13251693223881612765,
                            444726686851215067,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1517633259641896216,
                            2398309514230827916,
                            6779465744513889562,
                            2651039224042114000,
                            5382010513811017680,
                            1720010634583036866,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4169553049677853829,
                            5361009914769511499,
                            10119034315274181299,
                            18016282188849693057,
                            6767807590206143173,
                            1022578163456845962,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            12057857944781906687,
                            15591744129202116859,
                            2132656656635181080,
                            17009067558206246355,
                            7118144858046480585,
                            792848088863435438,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10303366621885392257,
                            1307897183080767395,
                            5759698657122936012,
                            5642011229054825968,
                            7449027764383047374,
                            838145745014431619,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            104876992545211052,
                            1471350652733741482,
                            14612197389809315695,
                            10191063893926040701,
                            73225968401856968,
                            297425489642142734,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12436425181864991594,
                            1004477940629415688,
                            4815289319108558801,
                            10750531686397049914,
                            10704839728835215677,
                            732237864471822729,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9681097773874003812,
                            13321356925228721436,
                            6572430093742575873,
                            9163058826547466559,
                            15240346725831411024,
                            1703825983307289059,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8280304832243563177,
                            17986463224104421364,
                            2025090693591655146,
                            17033354531853814807,
                            5618237026847530436,
                            747879086889921948,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8642200438376305573,
                            14303195194181054287,
                            15563936043761425331,
                            6917829317234673492,
                            16200076384395373396,
                            901465699924026374,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            11362883218285611559,
                            17819939515262828627,
                            13727126749891484254,
                            11310440304246503056,
                            17404812200047194810,
                            761501084855367490,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3018452406225875855,
                            6683238952834015149,
                            16872005001676025513,
                            15769126677443767377,
                            15577069375642124732,
                            355826045107891519,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            5946829177584488917,
                            15949957918290496953,
                            15073412346258939909,
                            2775952368836878325,
                            6632837609711069911,
                            601170566626596795,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            11861346849605103917,
                            17809980588897642557,
                            10747901601473276829,
                            3402741114897080533,
                            1762906362552051044,
                            1426712533373456627,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            9937604969890750199,
                            4614668782331763646,
                            11349996676328764269,
                            11668104715133397677,
                            16800513839194531786,
                            1258294751596811946,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            5052176037179002848,
                            6980979362516030011,
                            17668920266962814733,
                            10889348687570753558,
                            7874824218173420371,
                            1594647829750025752,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            10590331424924699487,
                            3593841465027830783,
                            6202841319371080366,
                            754912769073075899,
                            4602276352583923740,
                            312561330974708198,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            17380423081289664267,
                            4663446029249487065,
                            14093773292687509780,
                            526952317123990329,
                            354477309655992936,
                            1251842376018489055,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            16115314256723127776,
                            17376371271663601819,
                            14764871545058333610,
                            15748381964306313420,
                            2371563845675854682,
                            468236638430983773,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9519258557963545623,
                            10218805620159700249,
                            1734371048176574199,
                            14667465644903844798,
                            8827837969750605053,
                            457476084849609844,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            12180089775124528777,
                            17692306419441560886,
                            15562424393493290067,
                            1112141997765877690,
                            10160371303951815042,
                            1039629367900369726,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            9421164036044390086,
                            279020753926020825,
                            10725812785889227206,
                            905571621272799120,
                            14814457884764197086,
                            1588792423425743363,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            1674383127733047014,
                            12633139798948271696,
                            17344989456241582541,
                            2386419018435288287,
                            13625834384214223916,
                            191689017744474030,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            1462838877538247708,
                            8464432251473953558,
                            13827599480777523525,
                            6534154372025212797,
                            768433104806516168,
                            388859280029941669,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            7272604296928691358,
                            10077790980595287304,
                            1562683634096076231,
                            3269222634621611106,
                            4238700277701255352,
                            916207750433263517,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            8386313369719099530,
                            12194282181274218945,
                            18360985931761351216,
                            3540365973775214649,
                            4184998451581233512,
                            518150923124624951,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            17578227858366831484,
                            11456109190348289421,
                            437599011680526949,
                            6168402616437002213,
                            611200559863145297,
                            98610199661395868,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            7945315586259569061,
                            3273063287875655232,
                            7173238583905590061,
                            9610239530230900185,
                            7221327398198084279,
                            899270980372471388,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            8731211054787217311,
                            6384331869277595912,
                            11680039171373002168,
                            16304385507843577442,
                            9069083726310112127,
                            936897104328785261,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::Fp(
                        crypto::BigInt([
                            3703388115706493751,
                            14895905089655133838,
                            18419205411682486641,
                            11718932807822355454,
                            14866683930838041711,
                            779494058690467402,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::Fp(
                        crypto::BigInt([
                            4904201642943464699,
                            12387695678074990063,
                            4729241729481440363,
                            16884602615433592440,
                            6411054573889690659,
                            969148383752546584,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
        ],
        infinity: false,
    };

#[inline(always)]
pub fn verify_kzg_proof(
    commitment: crypto::bls12_381::G1Affine,
    proof: crypto::bls12_381::G1Affine,
    z: KzgScalar,
    y: KzgScalar,
) -> bool {
    // Original check:
    // e(yG1 - commitment, G2) * e(proof, tauG2 - zG2) == 1.
    //
    // Move the z multiplication from G2 to G1:
    // e(yG1 - commitment - z*proof, G2) * e(proof, tauG2) == 1.
    //
    // The two G1 scalar mults are fused via a 2-base interleaved
    // double-and-add: one shared 255-step doubling loop instead of two.
    // Same pattern as the small-N path in bls12_381::msm (we avoid
    // arkworks' VariableBaseMSM because it allocates internally and the
    // proving binary's allocator setup breaks it).
    let neg_z = (-crypto::bls12_381::Fr::from_bigint(z)
        .expect("z is canonical, validated by parse_scalar / Fr::into_bigint upstream"))
    .into_bigint();

    let bases = [crypto::bls12_381::G1Affine::generator(), proof];
    let scalars = [y, neg_z];

    const NUM_BITS: usize = 256;
    let mut left_g1 = crypto::bls12_381::G1Projective::ZERO;
    for bit in (0..NUM_BITS).rev() {
        let word_idx = bit / 64;
        let bit_idx = bit % 64;
        for (base, scalar) in bases.iter().zip(scalars.iter()) {
            if scalar.0[word_idx] & (1u64 << bit_idx) > 0 {
                left_g1 += base;
            }
        }
        if bit > 0 {
            left_g1.double_in_place();
        }
    }
    left_g1 -= &commitment;

    let left_g1 = left_g1.into_affine();

    let gt_el = crypto::bls12_381::curves::Bls12_381::multi_pairing(
        [left_g1, proof],
        [
            crypto::bls12_381::consts::PREPARED_G2_GENERATOR.clone(),
            PREPARED_G2_BY_TAU.clone(),
        ],
    );
    gt_el.0 == <crypto::bls12_381::curves::Bls12_381 as Pairing>::TargetField::ONE
}

fn point_evaluation_as_system_function_inner<D: ?Sized + TryExtend<u8>, R: Resources>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<PointEvaluationErrors>> {
    resources.charge(&R::from_ergs_and_native(
        POINT_EVALUATION_COST_ERGS,
        <R::Native as zk_ee::system::Computational>::from_computational(
            POINT_EVALUATION_NATIVE_COST,
        ),
    ))?;

    if input.len() != 192 {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidInputSize
        ));
    }

    // Each check without any parsing
    let versioned_hash = &input[..32];
    let commitment = &input[96..144];

    // so far it's just one version
    if versioned_hash_for_kzg(commitment) != versioned_hash {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidVersionedHash
        ));
    }

    // Parse the commitment and proof
    let Ok(commitment_point) = parse_g1_compressed(commitment) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidPoint
        ));
    };
    let proof = &input[144..192];
    let Ok(proof) = parse_g1_compressed(proof) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidPoint
        ));
    };

    let Ok(z) = parse_scalar(input[32..64].try_into().unwrap()) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidScalar
        ));
    };

    let Ok(y) = parse_scalar(input[64..96].try_into().unwrap()) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidScalar
        ));
    };

    if verify_kzg_proof(commitment_point, proof, z, y) {
        dst.try_extend(POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE)
            .map_err(|_| out_of_return_memory!())?;
        Ok(())
    } else {
        Err(interface_error!(
            PointEvaluationInterfaceError::PairingMismatch
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evm_interpreter::ERGS_PER_GAS;
    use std::alloc::Global;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    use alloy_primitives::hex;

    type TestResources = BaseResources<DecreasingNative>;

    fn infinite_resources() -> TestResources {
        TestResources::FORMAL_INFINITE
    }

    #[test]
    fn basic_test() {
        // Test data from: https://github.com/ethereum/c-kzg-4844/blob/main/tests/verify_kzg_proof/kzg-mainnet/verify_kzg_proof_case_correct_proof_4_4/data.yaml

        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000").to_vec();
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9").to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, z, y, commitment, proof].concat();

        let expected_output = hex!("000000000000000000000000000000000000000000000000000000000000100073eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001");
        let gas = 50000;

        let mut output = Vec::new();
        let mut resources = infinite_resources();
        let gas_before = resources.ergs().0 / ERGS_PER_GAS;

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);
        assert!(result.is_ok(), "Result: {:?}", result);

        let gas_used = gas_before - resources.ergs().0 / ERGS_PER_GAS;

        assert_eq!(gas_used, gas);
        assert_eq!(output[..], expected_output);
    }

    #[test]
    fn test_rearranged_kzg_verifier_matches_reference() {
        let commitment =
            hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7");
        let proof =
            hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c");
        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000");
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9");

        let commitment = parse_g1_compressed(&commitment).unwrap();
        let proof = parse_g1_compressed(&proof).unwrap();
        let z = parse_scalar(&z).unwrap();
        let y = parse_scalar(&y).unwrap();

        assert!(verify_kzg_proof(commitment, proof, z, y));
        assert_eq!(
            verify_kzg_proof(commitment, proof, z, y),
            crypto::bls12_381::verify_kzg_proof(commitment, proof, z, y)
        );

        let unrelated_128_bit_z =
            hex!("000000000000000000000000000000000123456789abcdef0123456789abcdef");
        let unrelated_128_bit_z = parse_scalar(&unrelated_128_bit_z).unwrap();
        assert_eq!(
            verify_kzg_proof(commitment, proof, unrelated_128_bit_z, y),
            crypto::bls12_381::verify_kzg_proof(commitment, proof, unrelated_128_bit_z, y)
        );

        let zero_y = parse_scalar(&[0u8; 32]).unwrap();
        assert_eq!(
            verify_kzg_proof(commitment, proof, z, zero_y),
            crypto::bls12_381::verify_kzg_proof(commitment, proof, z, zero_y)
        );
    }

    #[test]
    fn test_invalid_input() {
        let commitment = hex!("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("0000000000000000000000000000000000000000000000000000000000000000").to_vec();
        let y = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001").to_vec();
        let proof = hex!("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();

        let input = [versioned_hash, z, y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);
        assert!(result.is_err(), "Result: {:?}", result);
    }

    /// Test invalid input size - too short
    #[test]
    fn test_point_evaluation_invalid_input_size_short() {
        let input = vec![0u8; 191]; // One byte short
        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidInputSize = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidInputSize error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidInputSize error, got: {:?}", result);
        }
    }

    /// Test invalid input size - too long
    #[test]
    fn test_point_evaluation_invalid_input_size_long() {
        let input = vec![0u8; 193]; // One byte too long
        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidInputSize = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidInputSize error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidInputSize error, got: {:?}", result);
        }
    }

    /// Test invalid scalar - z >= field modulus
    #[test]
    fn test_point_evaluation_invalid_scalar_z() {
        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        // Set z to field modulus (invalid)
        let invalid_z = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ]
        .to_vec();
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9").to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, invalid_z, y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidScalar = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidScalar error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidScalar error, got: {:?}", result);
        }
    }

    /// Test invalid scalar - y >= field modulus
    #[test]
    fn test_point_evaluation_invalid_scalar_y() {
        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000").to_vec();
        // Set y to field modulus (invalid)
        let invalid_y = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ]
        .to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, z, invalid_y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = PointEvaluationImpl::execute(&input, &mut output, &mut resources, Global);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidScalar = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidScalar error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidScalar error, got: {:?}", result);
        }
    }

    /// Test versioned hash computation function
    #[test]
    fn test_versioned_hash_for_kzg() {
        let commitment = [0u8; 48]; // Identity commitment
        let hash = versioned_hash_for_kzg(&commitment);

        assert_eq!(hash[0], VERSIONED_HASH_VERSION_KZG);

        let expected_hash = [
            1, 176, 118, 31, 135, 176, 129, 213, 207, 16, 117, 124, 204, 137, 241, 43, 227, 85,
            199, 14, 46, 41, 223, 40, 139, 101, 179, 7, 16, 220, 188, 209,
        ];
        assert_eq!(hash, expected_hash);
    }

    /// Test scalar parsing edge cases
    #[test]
    fn test_parse_scalar_edge_cases() {
        // Test maximum valid scalar (modulus - 1)
        let max_valid = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(parse_scalar(&max_valid).is_ok());

        // Test minimum invalid scalar (modulus)
        let min_invalid = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ];
        assert!(parse_scalar(&min_invalid).is_err());

        // Test zero (always valid)
        let zero = [0u8; 32];
        assert!(parse_scalar(&zero).is_ok());
    }

    /// Test parse_g1_compressed edge cases
    #[test]
    fn test_parse_g1_compressed_edge_cases() {
        // Test valid identity element (point at infinity)
        let identity = [
            0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(
            parse_g1_compressed(&identity).is_ok(),
            "Identity point should be valid"
        );

        // Test wrong input size - too short
        let too_short = [0u8; 47]; // One byte short
        assert!(
            parse_g1_compressed(&too_short).is_err(),
            "Input too short should fail"
        );

        // Test wrong input size - too long
        let too_long = [0u8; 49]; // One byte too long
        assert!(
            parse_g1_compressed(&too_long).is_err(),
            "Input too long should fail"
        );

        // Test all zeros (not a valid compressed point)
        let all_zeros = [0u8; 48];
        assert!(
            parse_g1_compressed(&all_zeros).is_err(),
            "All zeros should be invalid"
        );

        // Test all ones (invalid field element)
        let all_ones = [0xffu8; 48];
        assert!(
            parse_g1_compressed(&all_ones).is_err(),
            "All ones should be invalid"
        );

        // Test invalid compression flag (neither compressed nor uncompressed)
        let invalid_flag = [
            0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(
            parse_g1_compressed(&invalid_flag).is_err(),
            "Invalid compression flag should fail"
        );

        // Test x-coordinate >= field modulus (invalid field element)
        let invalid_x = [
            0x9a, 0x0d, 0x51, 0xcc, 0x7f, 0xa0, 0x52, 0xe0, 0xc9, 0x9d, 0x3e, 0xa2, 0x42, 0x78,
            0x10, 0x5b, 0xf0, 0x1c, 0x29, 0x94, 0x3d, 0xa1, 0x8e, 0xf2, 0x50, 0x51, 0x73, 0x37,
            0x8a, 0x64, 0xa2, 0x61, 0x05, 0x43, 0x48, 0x44, 0x31, 0x15, 0x66, 0x5b, 0x5e, 0x96,
            0x4e, 0x9b, 0x4a, 0x3c, 0x7c, 0x59,
        ];
        assert!(
            parse_g1_compressed(&invalid_x).is_err(),
            "X-coordinate >= field modulus should fail"
        );

        // Test point not on curve (valid x-coordinate but no corresponding y)
        let not_on_curve = [
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ];
        assert!(
            parse_g1_compressed(&not_on_curve).is_err(),
            "Point not on curve should fail"
        );

        // Test identity with wrong infinity flag
        let wrong_infinity = [
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(
            parse_g1_compressed(&wrong_infinity).is_err(),
            "Wrong infinity flag should fail"
        );
    }

    /// Test parse_g1_compressed with known valid points
    #[test]
    fn test_parse_g1_compressed_known_valid_points() {
        // Test with the actual BLS12-381 generator point (compressed)
        let generator_compressed = hex!("97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb");
        let result = parse_g1_compressed(&generator_compressed);
        assert!(
            result.is_ok(),
            "BLS12-381 generator should parse successfully"
        );

        // Test a known valid point from test vectors
        let valid_point = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7");
        let result = parse_g1_compressed(&valid_point);
        assert!(
            result.is_ok(),
            "Known valid point should parse successfully"
        );

        // Test another known valid point with y-bit set
        let valid_point_y_bit = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c");
        let result = parse_g1_compressed(&valid_point_y_bit);
        assert!(
            result.is_ok(),
            "Known valid point with y-bit should parse successfully"
        );
    }

    /// Test parse_g1_compressed error conditions comprehensively
    #[test]
    fn test_parse_g1_compressed_comprehensive_errors() {
        // Test various invalid compression flag combinations
        let invalid_flags = [
            0x00, // No compression bit set
            0x20, // Reserved bit set
            0x60, // Multiple reserved bits
            0xe0, // All flag bits except infinity
        ];

        for &flag in &invalid_flags {
            let mut invalid_point = [0u8; 48];
            invalid_point[0] = flag;
            assert!(
                parse_g1_compressed(&invalid_point).is_err(),
                "Invalid flag 0x{:02x} should fail",
                flag
            );
        }

        // Test infinity point with non-zero coordinates (should fail)
        let mut invalid_infinity = [0u8; 48];
        invalid_infinity[0] = 0xc0; // Infinity flag
        invalid_infinity[47] = 0x01; // Non-zero coordinate
        assert!(
            parse_g1_compressed(&invalid_infinity).is_err(),
            "Infinity point with non-zero coordinates should fail"
        );

        // Test compressed point with both infinity and y-bit flags
        let mut invalid_mixed = [0u8; 48];
        invalid_mixed[0] = 0xf0; // Both infinity and y-bit flags
        assert!(
            parse_g1_compressed(&invalid_mixed).is_err(),
            "Point with both infinity and y-bit flags should fail"
        );
    }

    // Sanity-check: the precomputed PREPARED_G2_BY_TAU const must match what
    // we'd otherwise compute on the fly from G2_BY_TAU_POINT. Catches stale
    // const literals if G2_BY_TAU_POINT or the Miller-loop precomputation
    // shape ever changes upstream.
    #[test]
    fn prepared_g2_by_tau_const_matches_runtime() {
        use crypto::ark_ec::pairing::Pairing;
        let runtime: <crypto::bls12_381::curves::Bls12_381 as Pairing>::G2Prepared =
            crypto::bls12_381::consts::G2_BY_TAU_POINT.into();
        assert_eq!(runtime, PREPARED_G2_BY_TAU);
    }
}
