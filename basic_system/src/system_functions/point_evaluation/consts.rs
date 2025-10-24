#[cfg(not(all(target_arch = "riscv32", feature = "proving")))]
pub const G2_BY_TAU_POINT:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Affine =
    crypto::bls12_381::curves::g2::G2Affine {
        x: crypto::bls12_381::fields::Fq2 {
            c0: crypto::ark_ff::fields::models::Fp(
                crypto::BigInt([
                    6998771983072852473,
                    11736241389176950350,
                    14652389186963586383,
                    7123021877941670904,
                    207427363641627917,
                    1666061032901291221,
                ]),
                core::marker::PhantomData,
            ),
            c1: crypto::ark_ff::fields::models::Fp(
                crypto::BigInt([
                    1270972800850449493,
                    331328462692285148,
                    9602917463918608193,
                    2816806383447892978,
                    8933573566397811232,
                    215261465954158607,
                ]),
                core::marker::PhantomData,
            ),
        },
        y: crypto::bls12_381::fields::Fq2 {
            c0: crypto::ark_ff::fields::models::Fp(
                crypto::BigInt([
                    12255148049650361111,
                    16300459039673357879,
                    7278512065901627776,
                    15013916996328221833,
                    6959599066670318708,
                    1753751357774418949,
                ]),
                core::marker::PhantomData,
            ),
            c1: crypto::ark_ff::fields::models::Fp(
                crypto::BigInt([
                    6097766243631356938,
                    3657144287806647550,
                    7252852235594748032,
                    6043526089682840990,
                    694068262573112211,
                    1355366081521641917,
                ]),
                core::marker::PhantomData,
            ),
        },
        infinity: false,
    };

#[cfg(all(target_arch = "riscv32", feature = "proving"))]
pub const G2_BY_TAU_POINT:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Affine =
    crypto::bls12_381::curves::g2::G2Affine {
        x: crypto::bls12_381::fields::Fq2 {
            c0: crypto::ark_ff_delegation::Fp(
                crypto::BigInt([
                    15222373064398286084,
                    13305997496817878699,
                    6179074517294182750,
                    14794871321375031765,
                    2834697192260086091,
                    387745707543054929,
                    0,
                    0,
                ]),
                core::marker::PhantomData,
            ),
            c1: crypto::ark_ff_delegation::Fp(
                crypto::BigInt([
                    802106297986366494,
                    7763332301576374198,
                    16078281631408652708,
                    4142264898103746401,
                    12005984959834078047,
                    248731809877450469,
                    0,
                    0,
                ]),
                core::marker::PhantomData,
            ),
        },
        y: crypto::bls12_381::fields::Fq2 {
            c0: crypto::ark_ff_delegation::Fp(
                crypto::BigInt([
                    4900293511062467887,
                    17213741567581943225,
                    16312230343184456439,
                    4417609035285159901,
                    8724769964152345554,
                    1569984678681432578,
                    0,
                    0,
                ]),
                core::marker::PhantomData,
            ),
            c1: crypto::ark_ff_delegation::Fp(
                crypto::BigInt([
                    10357602823164305765,
                    17761333828174651100,
                    14619682016189758143,
                    14389745726652808402,
                    3537342951673246453,
                    1861810530228151377,
                    0,
                    0,
                ]),
                core::marker::PhantomData,
            ),
        },
        infinity: false,
    };

// println!("pub const PREPARED_G2_GENERATOR: <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared = crypto::bls12_381::curves::G2PreparedNoAlloc {{");
// println!("    ell_coeffs: [");
// for i in 0..prepared_g2_generator.ell_coeffs.len() {
//     println!("        (");
//     println!("            crypto::ark_ff::fields::models::Fp2 {{");
//     println!("                c0: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].0.c0.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("                c1: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].0.c1.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("            }},");
//
//     println!("            crypto::ark_ff::fields::models::Fp2 {{");
//     println!("                c0: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].1.c0.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("                c1: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].1.c1.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("            }},");
//
//     println!("            crypto::ark_ff::fields::models::Fp2 {{");
//     println!("                c0: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].2.c0.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("                c1: crypto::ark_ff::fields::models::Fp(");
//     println!("                    crypto::BigInt({:?}),", prepared_g2_generator.ell_coeffs[i].2.c1.0.0);
//     println!("                    core::marker::PhantomData");
//     println!("                ),");
//     println!("            }},");
//     println!("        ),");
// }
// println!("    ],");
#[cfg(not(all(target_arch = "riscv32", feature = "proving")))]
pub const PREPARED_G2_GENERATOR:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared =
    crypto::bls12_381::curves::G2PreparedNoAlloc {
        ell_coeffs: [
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5141805939031916900,
                            14881501738404200130,
                            5676796723958628022,
                            12444886031927648209,
                            12625577537049268217,
                            1648088581216593497,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13479113584676927735,
                            14085014821599008390,
                            12897808079030738240,
                            10758929507152881398,
                            16000768153542152626,
                            1172245526740373679,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2148348102093263616,
                            12232197882281708926,
                            11330363351339265390,
                            8919790901940522406,
                            17524282994943615806,
                            496043075549758450,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            406374380327561821,
                            16222300308001049590,
                            2744191801523148582,
                            9384378502465456106,
                            6088477103101489105,
                            1478173219842328727,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2384913353902199319,
                            7760278026988209995,
                            10908782382662037359,
                            17048340870424330239,
                            4284142373730869649,
                            1799494596850233552,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6808605746120772081,
                            17102333730072771654,
                            6473213958624321251,
                            8206845943439019082,
                            9001841424686257916,
                            263896703973961396,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            410815106749295669,
                            14671497359075212928,
                            5247007732706898941,
                            17995312000949087238,
                            9318564848658720568,
                            830601423731554979,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5581652307577543765,
                            6048112805369680744,
                            10863925005663083759,
                            13723116258784921171,
                            16395951836908287261,
                            576183509239513289,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12359103411754323777,
                            4484518591789401692,
                            7553773553318136256,
                            9286007000674216720,
                            3828416673463588070,
                            1125060269472609951,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16270530695471883024,
                            7398690485593453415,
                            7661592259938731084,
                            6115274743727668626,
                            11646088452095310487,
                            1172296607029761726,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13799898119383583275,
                            14835071743299103237,
                            2637313866028438562,
                            18274788277677870627,
                            13617768339621759057,
                            1000507734120925728,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17140326549953569073,
                            6405224081483886159,
                            4267674644342821653,
                            1752649571364847099,
                            17969610602415877411,
                            1346486438763963099,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10372035707327665855,
                            13033699652831669377,
                            18001351867719217419,
                            17683642914756429388,
                            9556503388952039857,
                            1329751260307086052,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2344862993485028348,
                            8828144198172398125,
                            15447134648735934574,
                            1255270699924723893,
                            4886950282422369309,
                            381483219941628951,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11860626640653029501,
                            10904903791216459764,
                            13503337534485290667,
                            346033757805938283,
                            10106952548686363100,
                            1513842329147353970,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17736603614208695589,
                            7622430165596287250,
                            10019522101254057211,
                            4957042600265288097,
                            8842339425901560598,
                            1070474381731750456,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3126979498979956446,
                            6683277335328149417,
                            15253880732281146257,
                            15099174266347432605,
                            17950830287545815649,
                            218127290741943543,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2868616128842733870,
                            8017633674659729200,
                            6433577224475778073,
                            586972730864327558,
                            7642330394223174199,
                            1383486225402290655,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18198021688502592557,
                            13511431506245271930,
                            10721587455469480135,
                            7759712521116984140,
                            4136567657482394076,
                            1322321864369983077,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5385833590826609721,
                            1997327134082967041,
                            16887896265967856133,
                            4670107666134914874,
                            8408948779659614002,
                            966249059862278969,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4238264172116927440,
                            8585427175208453787,
                            13391259632752185786,
                            2243605050137253147,
                            12543049565771064759,
                            1744764348013310000,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10362084567046626758,
                            10816154860909344542,
                            10573511294357547907,
                            140128829695140438,
                            13150599586971606962,
                            1132304614921786427,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2154321054217735380,
                            8325838573332693936,
                            9961626471649444564,
                            9464856130848169875,
                            14569577991616642249,
                            1187070117018910587,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12768121148781055196,
                            18245949438107747183,
                            9780133711340233987,
                            14985049703492470765,
                            7812515833497248970,
                            995044817595131952,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            381452297110706448,
                            14888063619316195299,
                            5903322847657577559,
                            4192955803278209460,
                            17451506546072907376,
                            146610250215610338,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5427213429271106499,
                            4333616755713581723,
                            3271300846951325985,
                            18251281755642440121,
                            11279254981659276170,
                            729466300166508568,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15793577716612855564,
                            4668995723578274815,
                            4384187013721788325,
                            5005115314512922510,
                            16580401117618392401,
                            1061089863156435375,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10938483931507615313,
                            1819128771204318317,
                            1378166881235931358,
                            13169265358183292197,
                            8495579604450832241,
                            1658457572327554200,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5606543612283953239,
                            18041223158520881442,
                            13479600114443068848,
                            5715782193307494167,
                            11021015089636409126,
                            216773321964843711,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2929533507575320551,
                            3443042620097897503,
                            14715630991325653192,
                            14609224186819678494,
                            14472777980914477678,
                            1241469894408677550,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8573336919322429465,
                            17992052812250631772,
                            8731453543706499896,
                            10664025860267270772,
                            230615772046556522,
                            266962162194865915,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17761807615271640858,
                            11489295081018607087,
                            10788634817114068653,
                            9639855738763174930,
                            13638905777067884774,
                            996032760442672471,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            417385912334655687,
                            16112303583556661074,
                            4458661395407831437,
                            9526451496016245577,
                            3386449271250125514,
                            730353849140560753,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9000093142487517991,
                            14134083959924237055,
                            18046389804750210448,
                            2943907734641935785,
                            11874227498753799708,
                            467415405859937517,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6399931434183934664,
                            2348668329180844113,
                            6545865515938198761,
                            3174039629982378188,
                            8156147867999962162,
                            265247106625245813,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3685955571729247289,
                            13154819674840828100,
                            2572300054611671044,
                            15295760164169411393,
                            4605338285028612462,
                            777648684134450039,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10847380501819118332,
                            9997176788977361883,
                            9145374535949399277,
                            2988305573081111221,
                            16532364284655040017,
                            22369720389082729,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2128613708163288559,
                            3984312897046484124,
                            440690462454569345,
                            4447100961186936601,
                            17011428506723439829,
                            464381823536933373,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14624486359648699748,
                            9996471425625772906,
                            7959195681278883241,
                            13187373556334020786,
                            16378006220526491063,
                            1102676257961916954,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10565173302830080041,
                            14557534785903372419,
                            15050302666547062955,
                            7876976764100481255,
                            4104086943124558954,
                            561283378587482515,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            709562408010554375,
                            4267992058656169535,
                            11687232113456093746,
                            11896453304250293406,
                            16082573870442512457,
                            1474877609868443671,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5000373916021640150,
                            8070258887298854009,
                            4784603921999825690,
                            6187343649437885286,
                            18164391822004564300,
                            101412407463769652,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13471431044201719510,
                            13652983820932105600,
                            16391697356344427978,
                            1909258935459139345,
                            4675889529846639014,
                            403289222787366231,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9211307257257923428,
                            1458284542262832952,
                            9868036486545772136,
                            4852373548617371498,
                            352514220866497285,
                            1364013305721814584,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8005481355175942977,
                            18325163562340905436,
                            5390710820200297131,
                            15110003324963209796,
                            10874791894834869540,
                            1392509089925744797,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7471236998666819885,
                            9992485353963224673,
                            5586811759391298758,
                            1425063232120675711,
                            10061366176295910900,
                            322974810213550416,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4777449248121411850,
                            295102903503592931,
                            13035937688357174264,
                            1734138769696788538,
                            2291149507807446168,
                            1849870684541128156,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16975918388064884856,
                            12834867506673749379,
                            648523850022296783,
                            18069076335058758742,
                            9876590207854679329,
                            1346654090519127856,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15747728109020361321,
                            13725681923363316770,
                            5324450391747943978,
                            774517115391587398,
                            15069895026598897351,
                            912503434719372401,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10149825125588160762,
                            11820756380776900023,
                            15159495854088463628,
                            2632153691577698988,
                            8726849390783566255,
                            1418039180389332657,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10949388138415984785,
                            1653345621151405861,
                            6989611558301588196,
                            7094158119030897521,
                            8092598974865067730,
                            344903474317610242,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13179427330210479777,
                            12627360700998004214,
                            16078285943569855890,
                            11503046501803791217,
                            12020735899017724573,
                            1587835784911466986,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4736315383562732347,
                            4857279745231940348,
                            12084525333989199454,
                            11935005340671850738,
                            5292246436264611071,
                            1464813745154178522,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9671749090904758079,
                            4011208751311086456,
                            17258777116205478546,
                            3988062033120029806,
                            7555416602408608721,
                            1230456088786320969,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4359473842744791483,
                            15229196376600208182,
                            16390865079379463113,
                            17304456992172995643,
                            8241645050374294242,
                            1048598515423429339,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5030196255765617673,
                            907619662631850037,
                            4908147951400445547,
                            4565498463706034311,
                            3222701189937984820,
                            1137793701347911568,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18266295415946367821,
                            9218337022030772249,
                            4573512429414012619,
                            13221532010045438499,
                            5438782004406746723,
                            1173984545771228727,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            592313367490451090,
                            6144941370159173573,
                            4415719859404102956,
                            12295351402307904606,
                            14201585298553426051,
                            1792589111712859574,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1520984947060149855,
                            4670730037180607416,
                            5364080033783607774,
                            11030375954854580964,
                            6606500366184623640,
                            1015849247070053398,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1860475962392426848,
                            6414572385931489470,
                            944547912718495232,
                            6207857425764014254,
                            16082638886377558425,
                            435828459834202715,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12415664568316923969,
                            14153586859668198792,
                            8313325181049576803,
                            11856236819688804952,
                            8670376532791302504,
                            882355792588751782,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16708868432318978244,
                            5890490433612928589,
                            18409737640877154902,
                            122052811551250142,
                            12310803993302720147,
                            251506331851000332,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9109251818875808493,
                            8434405903908160911,
                            425240150822053835,
                            9399371041275642928,
                            6287726593966843142,
                            1081706261152752478,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7807978281705858889,
                            13014064301541624918,
                            10752549970233376499,
                            7038868237437625253,
                            8518172012238343172,
                            472844660464331547,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5428618434409641282,
                            731074514690730256,
                            4909671977543008161,
                            10685029362744112834,
                            13579270631866498815,
                            670612544934760906,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15779443715945418536,
                            17234857151081093038,
                            3340207207509495517,
                            14906529178208012763,
                            17348922171258971904,
                            160543278375823719,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            572813010612831175,
                            17229932213218921994,
                            153412800968486396,
                            8382115482058901058,
                            13947916826235297912,
                            1060434287659930064,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2909189869389827583,
                            16283414275911366367,
                            16367387944566926439,
                            9066777010217601610,
                            12074685396270575418,
                            740216266393022223,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2221509424448989182,
                            8029152828048580472,
                            15231187909079953032,
                            16774319742105172202,
                            6628826656573200829,
                            1595226693061121342,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            887879947476000210,
                            14916108503088875124,
                            4255290164931162535,
                            17187108110984313117,
                            12416729357880053981,
                            506145233301733206,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6688122711399903377,
                            956534379015613198,
                            2306064226352684985,
                            14943216876664937687,
                            9922665993481137055,
                            449473707222814818,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7366760327299120827,
                            6723698819112066476,
                            10691768784174752242,
                            14405362052410758714,
                            3186794622910528358,
                            1628256393874806324,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5510538203572665789,
                            11026297340378949532,
                            14224245128751044848,
                            15089766391051904644,
                            10215127899568402056,
                            926540898138293552,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16523419256453396264,
                            7548530122574515251,
                            12254353138466430556,
                            8191411044395027797,
                            4584362460891999351,
                            1248660522713978812,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14379946145017925561,
                            3327578865202539495,
                            15983014440733985013,
                            16918401789285250319,
                            6804175225703239091,
                            629825587369830914,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5238084445534838637,
                            6349281509122043694,
                            15029679403126853381,
                            3543305929084475098,
                            8043862043604620304,
                            1774530426378903388,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8705402993275412290,
                            10309994477918659991,
                            10194982214192083307,
                            9832470544682507482,
                            13235909624056037536,
                            1213280542750553218,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9858695170949036472,
                            8190792129305206992,
                            18316361779769977680,
                            7065501875819745623,
                            12512614800124303252,
                            424780995124882112,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17091230753738073939,
                            5742407283561017981,
                            11206869815880854568,
                            5039463592612019006,
                            5706493686670480942,
                            1869829432548034141,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9918794650019348421,
                            13764666769916554807,
                            10148964309945121407,
                            6669075856076202274,
                            8950652948676731827,
                            859093905425356226,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5801944926310406828,
                            16606365636472891299,
                            11510564985002054245,
                            8618221901061351118,
                            3423654271244297912,
                            1254790916538181892,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14351542720162538378,
                            13077311130522818611,
                            3235161037234002053,
                            7772420373540310536,
                            17687285537178996666,
                            1112662618840671728,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9034883416908472689,
                            5293397624124428877,
                            10246982669704808706,
                            6986607116401213771,
                            18044558202738306921,
                            86588148847963213,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5370662249630643241,
                            8474350153879675377,
                            11632312534461028740,
                            7535231338191187612,
                            6586752577274218807,
                            1264764077468486862,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8280979498285331584,
                            17019750325519637000,
                            7076069628962101063,
                            7563539879491222615,
                            6172127473873583107,
                            1854881132180286239,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2071835565841741777,
                            1956669178919454095,
                            13011612914992615232,
                            9082336034965990027,
                            7660270005804839215,
                            1377361658900982072,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14358037576172456719,
                            4870327049814028108,
                            10919817122507308475,
                            16861828022847424174,
                            6937130395560086657,
                            1706623480645447844,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15667541763677614986,
                            8885513261058484931,
                            5248016105658358130,
                            6461060351251084201,
                            279338902814023444,
                            1859033859549368815,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18184577908890743160,
                            18356792231154436931,
                            10133245481394331698,
                            15644737709399520633,
                            9330036923167937703,
                            1147182018750432947,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10394453543147136379,
                            8191410394774562711,
                            165994681954033257,
                            11805522380584290897,
                            7981452714904861724,
                            1823514220346625232,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6246772424223103206,
                            13535707299370117047,
                            13571982035269134435,
                            12465208524416615863,
                            6166422329514466200,
                            538700431202549478,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14764571985258794292,
                            10322901757714296314,
                            277697261275397439,
                            13500953434633153854,
                            15469838882117236241,
                            1123610510755063835,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12029239395035277093,
                            11002805357522421531,
                            8999246049032739379,
                            13120287455103617564,
                            13472516720644507470,
                            323554578848932618,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5809710973960547215,
                            13458204376064628589,
                            2487973437886396009,
                            9176715112333432629,
                            7469121850432659151,
                            1801907694856932304,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11059972478455275483,
                            9255163636708247029,
                            16604769607715476492,
                            3656985343498043274,
                            2115634676761475788,
                            1251991146855816309,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17657908188424387426,
                            11073798568900799444,
                            4455764421800215857,
                            5801378529356094867,
                            15616431480564134346,
                            1796496794381845238,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17519954294993772358,
                            9688472811803864561,
                            4772609518940248259,
                            1486397643694454637,
                            15796702127462147232,
                            745061313639134949,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6492332148656391120,
                            11892647445697960374,
                            4956028036621043758,
                            15873228217171977725,
                            7088847408232978988,
                            884458387948228392,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15266144299652080222,
                            4435468330079935905,
                            3183565181247742018,
                            6437035485349062759,
                            17688765152509296440,
                            1868715196972502551,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5741957419475912963,
                            8713011334077104954,
                            511029427401626408,
                            8519202553144504147,
                            7704058049079094113,
                            626976432701717203,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14685537232609441349,
                            12817215031100449805,
                            7062803415061165613,
                            13061056474912282785,
                            16292628649392942774,
                            1726411621393573495,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7342034101471681091,
                            5981831773055475131,
                            15751998565050527759,
                            6827405799516296521,
                            9563311848952801517,
                            64756942261636680,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9312245213474993364,
                            763253257073167467,
                            2533148859694067490,
                            1557031086110277901,
                            8508168896787537761,
                            77324790895599152,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11234318394460893824,
                            13080925536205801339,
                            2974438010892781680,
                            16798346758236335495,
                            853019663129303221,
                            1553116686383780998,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10672509030427770486,
                            3411471258178267916,
                            15147115224899113286,
                            6509466393481970020,
                            789267814514693398,
                            759336200565465981,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18044316525074213694,
                            10583433656152181769,
                            1369264306974089385,
                            4568949137998859190,
                            8676791248189369854,
                            599810496658723989,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6592321761334491193,
                            17371940768472854432,
                            18368764698137791841,
                            4573106669022277271,
                            17136105106659820995,
                            1552408082806665468,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6679852690623383792,
                            7238344873834000234,
                            17655879693118300498,
                            14744078909744608320,
                            9873788307834513723,
                            216056756403160835,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8737197971574139456,
                            2470794839196658321,
                            10167394307573068236,
                            4990015158953624993,
                            3762261760425381855,
                            1117493865737123846,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2809336211689955115,
                            461364399510825826,
                            13512322279040218911,
                            4747323802367068145,
                            17008695436363516654,
                            1752048705458801231,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14061116903649502484,
                            16775564879532606396,
                            7762197473669358824,
                            7279166923317389734,
                            8458737303118396614,
                            8541471356393900,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5370719352741660522,
                            14190399276457775395,
                            11856188007549148553,
                            4290771095642050714,
                            5685314401789769717,
                            236157616473756757,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8101700174229828763,
                            14734600490653211235,
                            8320672592581831775,
                            9563374028059576675,
                            10324661543306959069,
                            1492437861443819879,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10865908099406512244,
                            15866458552322176026,
                            13549264719367926426,
                            4076155156194156988,
                            805455541784346072,
                            1448155476839248250,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9428156655815602008,
                            4108443916605113695,
                            5336647495022411943,
                            11345695112801438703,
                            6695223989892280346,
                            612625508919689927,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11119920249334032857,
                            1554550422566802323,
                            14975928531586102399,
                            7684854428208335140,
                            1185344066296825033,
                            1838127699743186716,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13180385423122124352,
                            1480508394386712760,
                            7616313138304958378,
                            10703476620031135522,
                            9291484443676454627,
                            1296044813488827620,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1292122395223995643,
                            10547441950937890577,
                            6288558001390788576,
                            3266695088792339778,
                            16340091016898498587,
                            305104804562104473,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12614657435499982406,
                            7321760421299539600,
                            8887870714814981984,
                            17308926768123915024,
                            10907982983608707333,
                            254782569586417921,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13443662230275057808,
                            7283338715646491712,
                            8762652993539391275,
                            9568118572053462216,
                            421911577020099018,
                            651333865719837370,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13558817906561336108,
                            5851162910471130994,
                            4839547741286309451,
                            14916751504369247656,
                            1551420211443359333,
                            151100226727178033,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12072854520021699150,
                            12163738072090558741,
                            8790616344897720272,
                            17628405401840194959,
                            17914738184224504104,
                            1808478666771880843,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14972613684404784505,
                            10854582706172394085,
                            17054500206331327498,
                            12676207388239629423,
                            13441910894169340413,
                            580331324581542656,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4556215054000136015,
                            10993047464222978897,
                            5917325015059164450,
                            11431004719854752055,
                            6777047625237478073,
                            1594739766495549815,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8368524145996765235,
                            16792128449470148343,
                            7451501716641093279,
                            15314826531557276399,
                            12289973506047041927,
                            1736559317069223212,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10334865954750154795,
                            12276715663254174420,
                            6192749220561928606,
                            12959596323639225997,
                            15985863425125289966,
                            1822630449876104576,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11562904017718506633,
                            6579608218515679898,
                            1836041044185674882,
                            1764269830347154390,
                            2220099293563482223,
                            338555480421158471,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10100356761369142420,
                            10424655188420335441,
                            5734489069780548435,
                            11804531177499496491,
                            15540257393125983282,
                            540426617410161771,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3903728819734581724,
                            6017989364983983723,
                            4347149887966132417,
                            15724076786717717654,
                            13422800188273302224,
                            75114195302985353,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8927408070343603731,
                            4744055380721541335,
                            17356141953351775670,
                            220262449755162239,
                            8762371885422412323,
                            875041224433176916,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7805613614301416893,
                            8211924119434414988,
                            4780969413207420175,
                            6054971174804680918,
                            11789086836721105818,
                            364984441896091125,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13707114789309096817,
                            6983770212939376625,
                            4652999218495815702,
                            15390124120475979572,
                            11425897909266781607,
                            1815736988543237201,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            756429358325308477,
                            659541189319221444,
                            9076774734801098537,
                            7423351133737467281,
                            8587430796292973973,
                            354344686039546896,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8409542993120211214,
                            4381373496129436439,
                            16433219173975623284,
                            18413920590028294169,
                            8153936644811655429,
                            1129864039932514084,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13104989724667548822,
                            16376820145402192197,
                            10318058046867619731,
                            347271512513951709,
                            5037624056388832095,
                            766809507996547074,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13673623915494396761,
                            11178100410220631593,
                            4169679698331960833,
                            932618932010414866,
                            8093871536244890828,
                            422732936255767862,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2248031022531194288,
                            13182615268354766205,
                            17159159755157149353,
                            17466312037965417492,
                            3947940995603806712,
                            1805221247387915355,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11650022333419710480,
                            3821434330496576576,
                            11067122868503029041,
                            4710264436379844964,
                            4608125004352504360,
                            377939507088559664,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            323427229854884849,
                            15059129573007958674,
                            10108807310890432816,
                            2192584389865522553,
                            8603319949293960431,
                            1211995643565977375,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6617523029171881322,
                            11383322448186641350,
                            7559578974833833008,
                            3205173471494285020,
                            14392352251207536437,
                            1216518386488752117,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13974112595347915982,
                            18220925998206931233,
                            5819973887253711430,
                            10731293668836791307,
                            14884656678543918401,
                            553607838101055965,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18342847706747975145,
                            17503336598929234156,
                            9082996789852941719,
                            12254582692588866283,
                            2652816587616976547,
                            1651037606221886301,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3806695740296916359,
                            3621254523957269984,
                            12563782648420016231,
                            10854775247040384554,
                            12923280103678034507,
                            334333064748926871,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7986793279595160847,
                            3115107781589608662,
                            5549751392645077837,
                            16637642137988243682,
                            10778592284957097351,
                            1112601254317530668,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14276054406963632957,
                            5334816257639817066,
                            16918079679538189141,
                            1256889063742606781,
                            4429197869623683683,
                            1052564087951581473,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6587815683562939879,
                            17061663948376645289,
                            11987879735554540077,
                            14687742048052059379,
                            15871762594691957824,
                            1400545276768732750,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2857182733695150298,
                            14492502368509374315,
                            6394066382421140984,
                            4873260557352586907,
                            11745621581369190709,
                            101070833293163892,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1830781412772034822,
                            11065996574206904394,
                            443407900315515204,
                            9864221378907827448,
                            928707473418445932,
                            789312371568169664,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1631293021391196217,
                            9348919760617510194,
                            5157507494237347818,
                            3412204342022241830,
                            9929962472745352202,
                            250781686293569324,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8725641782772531504,
                            6655947609054648794,
                            16333363275704951817,
                            12246989682647476912,
                            14833870257003335928,
                            1360627535322727843,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16479828748373544357,
                            288126515347333515,
                            8466738550426328621,
                            9581686375219113341,
                            132931039437676687,
                            1093670044843279481,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13247169693856882953,
                            13854495156863289204,
                            14916431986517228713,
                            14891433230462954539,
                            4630658022816049516,
                            7946960084262839,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3968197022474644210,
                            13691032437123974025,
                            3257576533498966575,
                            17424029346920503662,
                            1219026027825904483,
                            432630522521653764,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18415896655570881926,
                            15471807232790398093,
                            11105822436321302454,
                            1714823536627667804,
                            13286914906765947930,
                            1671434534756521091,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1948713421501294402,
                            3666068177285254931,
                            3302136597595848638,
                            5774412565041263881,
                            11858949347321653231,
                            445026608623954694,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5191139556204333805,
                            9820054992783904697,
                            2586059096705926519,
                            15740220772239480604,
                            2912029280360817240,
                            935213602419325709,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1657715447292834868,
                            2965196754196176708,
                            3507960075236678865,
                            14257122211122657297,
                            16195261890988014160,
                            1359438125688526049,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12813818712711389308,
                            10204629540719373260,
                            12017802221556548665,
                            16454940640503084174,
                            13481633468586520142,
                            682815932022050192,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10936361258559783826,
                            9568753568013478204,
                            5069569775606198918,
                            14636241003936320403,
                            2766342499692823628,
                            290192532328908336,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6065710111562110300,
                            2669228301371161303,
                            8791675820899482243,
                            14422205176293018020,
                            13603583659291202708,
                            1120871229977348719,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17992903148930215522,
                            8699796732563272905,
                            10856881822414778484,
                            4897040836582574597,
                            1364711228382736183,
                            593678163703515224,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3065481212355142345,
                            16980518859180403147,
                            11449642751002013335,
                            15493100692066621770,
                            816310608108964677,
                            157154334540632315,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11827439129624723879,
                            12960531134143634868,
                            81100493982484384,
                            3881019493167631986,
                            14505070854566559906,
                            1009142801207247198,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8806400921730365629,
                            4604747154207809521,
                            7212994567852887195,
                            13789244228662861640,
                            5618793575574262131,
                            854116858602998744,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3659507673664194883,
                            5029714648683569775,
                            2930917609321627441,
                            47952116182632615,
                            6307287339046222113,
                            1318306840534101160,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15612234697892050000,
                            17724285545119211529,
                            702481085194978052,
                            14919206601691285639,
                            12661058390066341103,
                            1133561129594690505,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6825797417508498353,
                            7601601940903371561,
                            18086777943531459348,
                            10915505851005145273,
                            8295310039502550522,
                            208526734251212012,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12809080936152416425,
                            3363446487535372208,
                            1593669525614429479,
                            18366712507472896252,
                            11294551264162408956,
                            746850206714260505,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11896272653453487718,
                            12262211153702241855,
                            15900558098585718890,
                            3969340192269706987,
                            18116125173439016592,
                            638156942425780148,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17819551186075410805,
                            1831090028451772986,
                            16446626650644612008,
                            6877510800182892895,
                            6107112253482296948,
                            949924039180853543,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10289249185598119887,
                            583806682878801538,
                            15243531005351046092,
                            4191309864744162614,
                            12134446094112313096,
                            1253555380725307195,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14208315524873981697,
                            15726946123006866023,
                            2200825923806428221,
                            8928071804522453729,
                            13691968997549726781,
                            188718982116483912,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2817695138244938028,
                            17942739338936562790,
                            4078976393999431199,
                            11066789245987752446,
                            8777372617925054103,
                            565981311492096061,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10447940287494510448,
                            12907759181700104722,
                            15177912804949353464,
                            6032826097982879706,
                            16231974647869412816,
                            724404737633441835,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            273530767975359809,
                            640960343280265344,
                            3206660265780635369,
                            10627544832607645001,
                            7924183912727038157,
                            1031020725916350848,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3916993637981712528,
                            4336206702819842879,
                            4752647567496437063,
                            12411432589668429002,
                            3797020205905694526,
                            575240292666795475,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17906823193119094511,
                            14885792574463768817,
                            1055186485892938401,
                            14891180091066733776,
                            4312935767833134159,
                            1259051711244060747,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15602357124582241437,
                            14680316575002632180,
                            8438191597647786843,
                            12517236504392829175,
                            4040605163119008780,
                            1021972437210527156,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10219967886937106821,
                            12963678402262470926,
                            12756161081647870463,
                            10134451477496366268,
                            12215456402301222627,
                            1470621567471326550,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            154401509468976557,
                            17584684348770715131,
                            7543898025024632959,
                            18099797128522450533,
                            9337011953917962307,
                            1490382734303727912,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4458984523275779495,
                            15613803599795879682,
                            9503372989488317926,
                            17421070354463841259,
                            2041345037914509133,
                            269118574277721126,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6344353256113505955,
                            3622962565745525717,
                            5606919033076814384,
                            2696263065597548087,
                            11639795274690362850,
                            1161562882011478630,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4412980905618251017,
                            15537880068613179206,
                            369513134308123982,
                            4564375012623591815,
                            14433272751693670981,
                            1580844509527173911,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2762808031102482190,
                            16150872093625360011,
                            1331101168933797021,
                            6590314366537614994,
                            12691269149061414756,
                            982831943855863273,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5376934285612957961,
                            16876177625654590350,
                            11973740203103571480,
                            7060895647881444494,
                            13153722607967103553,
                            571864000812697046,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7246820349047722207,
                            4867979251579790772,
                            9021375131075207640,
                            12970184790434113824,
                            2553229341212193248,
                            832490965875941417,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9274201758346276486,
                            3788672265227661435,
                            1855235771841319911,
                            13957589706885013341,
                            12829857355013233963,
                            1058395368379912176,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17925207873879493365,
                            3719844925213384705,
                            15566239488935555669,
                            15697338598725600748,
                            1056501701568394255,
                            1323913789671533448,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16693810223377953954,
                            8992979349074895166,
                            6230675067474713795,
                            10250757017916809653,
                            11589010918628184101,
                            1153472976462911857,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15238598884229569208,
                            10803660612249481426,
                            9218191100816051507,
                            11340704888317265653,
                            17361440015823407755,
                            1720441625680538654,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5441093851164674869,
                            1800963049840822269,
                            451736645629520554,
                            3476225213562820998,
                            7449469664746243912,
                            1598790789065845332,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17644793259946941268,
                            15090693303518547252,
                            14904814944164261900,
                            3388360116089914674,
                            1215944320002867760,
                            871622829572904421,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3188436387852423243,
                            14993756172975508805,
                            18130395671291577498,
                            5774043178238259953,
                            15336268316706529154,
                            260671535524585275,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1909807180595145393,
                            6221295890287774404,
                            8841757198385160332,
                            1473027161152451999,
                            11073410034004334514,
                            1840888323905894817,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2803714541482159866,
                            9337216940067663510,
                            13691847009863426938,
                            15285614033467414566,
                            17722449335888191406,
                            862483000481405739,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12049355098210974057,
                            1326104058858507219,
                            3708382304498040404,
                            10303147766405142052,
                            17121523616779111396,
                            972180771722297409,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10375654766556953472,
                            2487054310364910604,
                            7877220313556469568,
                            17935294906001081987,
                            782136806585869042,
                            174783090445631703,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12157421733708710033,
                            14144237238048503245,
                            7518540404899857534,
                            529844606823728640,
                            1237451933311754628,
                            900738385531471495,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            447499281329623709,
                            14479152397836594719,
                            7461142143690023183,
                            9700092266766488397,
                            17544865510199315743,
                            1375287079732080800,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9913628416916474269,
                            8390439649295024054,
                            1270962387064159511,
                            8692614407920393918,
                            5659200583125508553,
                            389876469424149391,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4538883002515732662,
                            11001466556661716354,
                            6501875823728744525,
                            5193067771817008024,
                            6548593115572784407,
                            114550064927459080,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3971065204850673297,
                            8863508345468882221,
                            9302332606512685031,
                            570550130368381465,
                            16439498351906305598,
                            1631908396184674497,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2135920517443507378,
                            13825023977572313533,
                            16853422899225116596,
                            9300531534110891403,
                            8761160236662485095,
                            526931700001302156,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15445271443545116080,
                            4890507702034957078,
                            15159637508638863599,
                            2044740287104007297,
                            6909379997148374399,
                            1747560486961801133,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16395155563036064542,
                            16118493481324474667,
                            10598595526989639487,
                            14108600693041977859,
                            12123471075139739434,
                            22761574491135085,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10681254512753394319,
                            4136080076831107785,
                            3959828045334609588,
                            3812814985574108015,
                            13467601519655943212,
                            1395264649173308771,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            574913706145012235,
                            8436060835033739005,
                            13596647401322912938,
                            9323610082217101238,
                            18270504072132355342,
                            392088878688125872,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9931059746520888646,
                            7332998347018661869,
                            5803276161270726044,
                            3433170595431564336,
                            8723478007701976591,
                            264220272549826459,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5285241078847621520,
                            11962514263092929059,
                            2202143964754956553,
                            5691997987866788542,
                            15798436842476235793,
                            902353860156111516,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12624428720858408614,
                            8765762813120459871,
                            11880377705517380615,
                            1550404360018567261,
                            11898602958633805843,
                            1609733032427676325,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6373465907492637063,
                            17811666536366150282,
                            2460704908256228608,
                            11755791054413605069,
                            17916415863562502748,
                            1037079854987328209,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14919978742464217634,
                            1741257500454590564,
                            15633257012491385946,
                            1068912165064278574,
                            13704360611864819076,
                            213127566294379095,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14877620808295892171,
                            5904048013482912894,
                            12225876173656649719,
                            12788976918807240667,
                            8467813405004346992,
                            1163130203889086667,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14877247712565120860,
                            11235942375535900845,
                            8180257424437776314,
                            5574880854503671831,
                            2954625920323569521,
                            1836333400700562338,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4687407716328619684,
                            2085255772484791463,
                            15463630759112731974,
                            10908085673071208893,
                            11042459575812361094,
                            206085496107063010,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9447127269985211075,
                            5273059548574530004,
                            17046020599249028614,
                            13457955683260923947,
                            8617667592455421701,
                            1024744763154092460,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11728810668201589824,
                            13860043067999472965,
                            1213434209717171378,
                            11824196852601883395,
                            6823859597543923657,
                            1507393704166490576,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10921490262709603640,
                            9837397298248190298,
                            1745433799620417038,
                            1301121920924094449,
                            7207585168146122369,
                            257300796630018037,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1061348541736409971,
                            16656949433737689916,
                            6631090845639787437,
                            15522185084087965540,
                            6017061975726002327,
                            123635952079412331,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7740004307559021012,
                            8561083038510029334,
                            1147539059770274511,
                            10504475757094058646,
                            298927325257596852,
                            1716575662989644286,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16648074241240248648,
                            18274684562189032639,
                            17360352817804970713,
                            1059627640875741436,
                            6897991225438707457,
                            629363636812793092,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15873473250236395643,
                            12977645007356703813,
                            8208924490408901447,
                            4599318807417937516,
                            483548642174731811,
                            222163419728575055,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6158796921437644583,
                            8370789166527684801,
                            5760511341802896891,
                            17945126048013647151,
                            6450997361693031811,
                            813115530765987639,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2273493884542555372,
                            14333200620786586253,
                            10559541673100513443,
                            7553625226323259795,
                            10777770590235906211,
                            978754887841112378,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8256227538920604942,
                            6111022756881818707,
                            13053122837052267120,
                            651897312859112357,
                            3016802473491248619,
                            1049407615558035226,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14364693709097481213,
                            12340868517265015944,
                            8736889476635081411,
                            7917880176286895781,
                            13370846970018036261,
                            1740088274742243246,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            524907353785450792,
                            15695601711423124186,
                            16911140830048785722,
                            11155824942809572925,
                            1770167768104118045,
                            777116731273387352,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10310937194804331620,
                            7637704793335670030,
                            7316173518283538746,
                            13366444768516630043,
                            1310491413285186700,
                            1337943973262041692,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14421048613253225665,
                            9797353975377286039,
                            7214508657673197879,
                            9146269721913933729,
                            4710560144112653236,
                            1673516520094669593,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1497622130892396390,
                            1685174150335999689,
                            17352312188836824522,
                            10139936009166615853,
                            17350898555052595477,
                            1419273397312173847,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16861046793701440639,
                            15250682641580774803,
                            555730900539281901,
                            12692740996580732515,
                            5395965899205941366,
                            1808528827748692474,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3098127189548887015,
                            15994738185042809244,
                            12411425234484117159,
                            6542382283625911324,
                            4654548441214649860,
                            431820555911675094,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2063071015836874970,
                            6840062203494022182,
                            4246906740621860236,
                            10347110875184985451,
                            3418282834817861917,
                            792403127842874800,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12411294235993556822,
                            6095942616613941634,
                            1264723007403367433,
                            17217693954228208161,
                            12837288654241068219,
                            1054814062074977847,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16345770028904744153,
                            635125758526069578,
                            13439582931103153126,
                            1020746358833587565,
                            904416854996290702,
                            1501061386345380706,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1757920279692427649,
                            18250564919363320036,
                            8788373708794817081,
                            2614680759282236424,
                            9267340562510926376,
                            1689983745832315252,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13117188184349176794,
                            9213746859351061973,
                            6206940721877177228,
                            10058174399688150253,
                            1490027588809854309,
                            111794144929683848,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3140692817466575112,
                            10106745724048169351,
                            2998507736300218301,
                            3725327402338198404,
                            9497342645347040977,
                            1659526413135649020,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16808624171886853667,
                            16823473022172203203,
                            2636185418996595896,
                            9682239813936974363,
                            9385139519878354739,
                            1439707913581902285,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10903121819971190926,
                            6301654292330854238,
                            622005511920086765,
                            86988489766571632,
                            7931113254436311922,
                            1840637847662929388,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5382989087246186575,
                            12611284546852156697,
                            5519007439087895825,
                            17267655992774560163,
                            15751083557506950874,
                            881687229034211373,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17470869782826080067,
                            14529183271117580885,
                            4753254732840383124,
                            4324084504937291015,
                            2552103603585105930,
                            1532636300247273429,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4303969820917004885,
                            16021322405070684967,
                            801514668775943850,
                            387090479516039467,
                            14861161134438853533,
                            1849811583389758609,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6098376282457688380,
                            2994986269629813457,
                            15989528436836051918,
                            8293271197016415590,
                            9632661798666043861,
                            1026559911101782200,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11986159923773006299,
                            1760375557396412509,
                            1096017653307098561,
                            3760674156213476675,
                            12067894833165901186,
                            1668810088621756976,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2042101157000966398,
                            7462055591349297228,
                            12104856671158231381,
                            9488827218597976755,
                            6136719543784469427,
                            436892495147116682,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13271770293435464535,
                            16197563408818897353,
                            9816930109073256953,
                            729214006287386557,
                            11927401393758584942,
                            49270738778169830,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7468886689336311320,
                            4759013954267918844,
                            2346172033566943771,
                            1192658728962029589,
                            14580388074944279118,
                            25590512253797962,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16400469989521551329,
                            14110364153711685630,
                            7517941015120787605,
                            7672599722420857808,
                            5185510060443190203,
                            942295098188018604,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13600955351968491944,
                            1176097789172245068,
                            12690409046523452044,
                            6398083700373400129,
                            5273897916138114699,
                            655719147052956322,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3963257258200804747,
                            2775797544556973959,
                            2940298404702965828,
                            2412744012885299363,
                            15545173526659350678,
                            388949182738911812,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3668785576722942204,
                            7249294278380608852,
                            13759030843631123525,
                            12215165387169092548,
                            5478351532888416875,
                            1547723398824710037,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4106699163198478467,
                            16839371468142614396,
                            16301215183118884666,
                            15169264832648796323,
                            15707777618212581667,
                            969960710948051225,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14598655048953364033,
                            8615324336260651737,
                            15970289678594653060,
                            2870734462625256187,
                            13647993669315812604,
                            1761232944315904675,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17585718292781595853,
                            14991205606377529575,
                            15074272135415351370,
                            13375869852001983627,
                            11054569396805776167,
                            927411424184104512,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7136560337966585341,
                            17745790229256045319,
                            5740144305982986561,
                            5417019123785848959,
                            18366577115079994571,
                            16685878888735315,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11495994509590059745,
                            5136784020063164036,
                            18078166988629502114,
                            16059086812426354324,
                            2209784122419294348,
                            1872818377118103742,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14437536084306211044,
                            2740119505754636304,
                            14887480180026324418,
                            6297788622344952083,
                            1014510258580108648,
                            491488379993155018,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8152577241533669276,
                            16536715003179716814,
                            5013364591910946032,
                            10476769608483189650,
                            16195222532199643034,
                            1774833168141260467,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13164266012407861771,
                            13889378600737292392,
                            13534801411086474702,
                            16493245232092274994,
                            18294328952236143749,
                            582973946380157112,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5998430215038144200,
                            13321010093839282367,
                            6355097140409156300,
                            17699122333059865088,
                            723381955488197603,
                            1752335823727171331,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8178378178964942425,
                            9690440548689074571,
                            1595239067887263702,
                            18246567274138096780,
                            12749575382483691112,
                            1117708454142771352,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6826943486004856546,
                            18252232290117612309,
                            16613874782269471735,
                            10995498202085839269,
                            9839203529012425954,
                            360662255776773701,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2511215022943472746,
                            2151498992163238866,
                            16874250069783055797,
                            8999429806181255120,
                            14973179934442845045,
                            1525815828660616547,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9317391341099404465,
                            16297310759480927000,
                            6062528274131147824,
                            3135883026756803893,
                            10550902392195492265,
                            1119024169417391583,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            311751782623208441,
                            15308013993573478419,
                            1447985556696587802,
                            8907269195813833246,
                            8926204583111173860,
                            385935389515190331,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7854360764642732589,
                            2197239552644963643,
                            6052113846242702709,
                            15731492239268983402,
                            13435933342835215032,
                            1355419882633272473,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17940706519477320747,
                            14604417898788785723,
                            83616261297132160,
                            5142722845448990123,
                            13157345363188253197,
                            987094135952052072,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            817576745842838727,
                            3752924033538816527,
                            11867010305307261464,
                            16239711530954178881,
                            12171047215837153488,
                            1439936650697082151,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1747255690104780089,
                            12331885010613715056,
                            18221767152818872114,
                            15942183720261926491,
                            12958660637375673564,
                            129036590065882848,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5969052953054689877,
                            16804734598565554010,
                            7346486627198660091,
                            13132376355489037157,
                            14621567297425687225,
                            1538165698240311584,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14045106345594092327,
                            2344720966773406875,
                            15704452785615335497,
                            9586786496211365777,
                            14301365767236171066,
                            1491370388304233653,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            112847569975435618,
                            2855011004390726126,
                            16666425836734398553,
                            7427401460247264422,
                            8930997958747968245,
                            941036243695178874,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6854176806441209952,
                            12452846921780398163,
                            10534590437174292092,
                            4157156765560989367,
                            12560340964386033937,
                            735499109515154957,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12868905878357315350,
                            5720457962818245394,
                            8499563938777709836,
                            1281605558793637176,
                            9052129869434703579,
                            932775640995920332,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15519245782387265988,
                            8965045589516494184,
                            14808629855723674572,
                            3477560561115016769,
                            6561027564782843847,
                            513536291561502076,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            358529453782366833,
                            8028322383692735528,
                            9267675569568673861,
                            13842451876326330593,
                            12434109116672330952,
                            995331262320207668,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2760256781738382532,
                            15543786468979870096,
                            1832074120111867574,
                            14871376433774997473,
                            5722239134705046853,
                            1184623726580659227,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2620758877365803179,
                            2146849051218906659,
                            16819755666756490750,
                            2671946394905233218,
                            1050142790719909726,
                            624074784988256019,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6566189454337295375,
                            12247294398119054834,
                            10722793470383301364,
                            5905519004925859975,
                            4517572920402335205,
                            509862736104351308,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15569853168920263456,
                            361197763499607806,
                            4003306921178996318,
                            17484618443118177221,
                            17462072783479530815,
                            420536359715603289,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17404024440860530247,
                            2559496277438558450,
                            15275341193758303572,
                            9916662702013170149,
                            14705928314281568400,
                            1502711175441676760,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18299144478966412892,
                            9262574033600178742,
                            3611043133597558110,
                            13896889307585154195,
                            7137001299317170158,
                            222940116695315308,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17636175901343449131,
                            10122383361345456211,
                            3523569282000241994,
                            10711521739350062468,
                            11571092932783985328,
                            991865777959814710,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9211934819990870596,
                            746245598164526808,
                            15713659156674722721,
                            7493013283684933399,
                            5973216279782849303,
                            1319690663858459127,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13207795863090796735,
                            8805612809295103223,
                            4784689056009220385,
                            4779394889098154256,
                            9440700709172463218,
                            1296452206193991108,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17715278236299607254,
                            6736439616463458006,
                            11038702887485942095,
                            15867804669036769526,
                            15634881396005817993,
                            489723018123298278,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1132162753583317856,
                            10277749087604186671,
                            2156841408148863095,
                            14046619671361840627,
                            14527320179355494350,
                            1521728716123160304,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14880215038721983766,
                            7387017523869429623,
                            17525999497280475373,
                            2380213343767944389,
                            9298862318316667707,
                            202047976778748515,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17495609747118830892,
                            10624484726923714660,
                            928001122155035043,
                            9180480136517984389,
                            2010016949815381476,
                            1809111527722906898,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1276934049014168426,
                            17276259311312628937,
                            4454076464824832889,
                            16301021201864034716,
                            8554975839098852766,
                            1458254812293818352,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7370589103868337910,
                            7875428885664578646,
                            17821887070850190125,
                            1726558993439482913,
                            17607731644779182843,
                            58735883930758391,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4547179141298766498,
                            7928969349288893386,
                            9404663890689026548,
                            11442273783165918777,
                            6241828040065135576,
                            1449357837746612826,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4830200267341305732,
                            13326111200092022226,
                            7450166445394390911,
                            11780673972623080478,
                            11446275300389411534,
                            827588453927130689,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13775940385084320299,
                            1972197458532403414,
                            11293923482485216782,
                            6460425606821847702,
                            1499788463996260506,
                            1620338708077809457,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6414268186586043316,
                            12478734243317707800,
                            2127837062602571476,
                            6251272720315557475,
                            6958058782770939414,
                            705146260919294060,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7387940570081342206,
                            15864631906126525410,
                            5237313169925674691,
                            10566319873514363291,
                            12887222303354613523,
                            819857065025035973,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15849157766345815947,
                            17004442624020788924,
                            15157123601131088152,
                            2424187017549975695,
                            6238805223338829593,
                            48107028212652075,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17990156117996354303,
                            7614302510487808932,
                            15352171224978273855,
                            3886432970865616940,
                            17288872436394837629,
                            370332122754624701,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1039109280292018695,
                            16583278523664793843,
                            9150267651887264280,
                            8663817771282245154,
                            4411283383737984247,
                            913945778067562110,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            305239749439892347,
                            800896027095572031,
                            6665037694304341,
                            11615696819982717509,
                            4338274716423768850,
                            1819962152045105013,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1806020103061386019,
                            7512847780014857209,
                            17094397007385037263,
                            316976600296578903,
                            401824747693126931,
                            548665527085722626,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3268685494497437767,
                            2080768970020944170,
                            2201735027910066163,
                            15458776692425830533,
                            2855112192250237611,
                            1513862753357530327,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5765969437519781038,
                            7920064697548272513,
                            11577488360486961158,
                            1839701258924740355,
                            8355871817728826171,
                            213266834882512443,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13476999144548679199,
                            9223629678314753198,
                            15433658711165222706,
                            3274602718410116298,
                            2599836751271234598,
                            1572096896368899673,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10423572297317691196,
                            2410573942084075013,
                            5950428235090737163,
                            16220464213449650592,
                            15317281349909137657,
                            1023319033027183011,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16540418349536687587,
                            4714793885263410257,
                            1183427607985861498,
                            3927432755476133183,
                            1162459966883173807,
                            352524813769969331,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4292274949408048322,
                            18192569304892911421,
                            9012356623930242566,
                            5739784363489062221,
                            4761260094004811729,
                            662718641348462616,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12086106591189901952,
                            11772063701557307576,
                            127860271682619651,
                            8895234449845459576,
                            12659121583485797654,
                            893236344384702902,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            999628549523765250,
                            17585768012291868471,
                            16179386747410357622,
                            10260780326409122862,
                            17342318601981071424,
                            879489197116237949,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17135650875019297117,
                            8973866350611335408,
                            2817772143296052760,
                            8311290227566691458,
                            5194443964302441476,
                            232054768591486061,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12197605605619767530,
                            9167861191842116778,
                            13289550063991233166,
                            7337565126402696897,
                            3996690786507181786,
                            147743495143234507,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6131990600103396770,
                            7537007034668572580,
                            3113705088722491092,
                            16217494464704995384,
                            4867344927662487314,
                            822308441637851790,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5562887427295293882,
                            8932777604121834338,
                            13206052999825282881,
                            2029397130855713889,
                            5266328360773418202,
                            1571041739963428279,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6766948076524794878,
                            5524163308829394687,
                            10947099391491609549,
                            11917569319004034198,
                            16323566958142703707,
                            1115135720175226146,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16146354649106487098,
                            256378285812368071,
                            9492767563206114645,
                            17249583047652680260,
                            17922820346193058995,
                            144282512688821677,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1810277271446360408,
                            11850796728294152714,
                            57851788112027534,
                            2109087389872636618,
                            9675898346022620635,
                            1084632926635979412,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12756063847267106679,
                            3321147515220005874,
                            15543113047199253995,
                            3344629895161813177,
                            16379494060978304053,
                            494857061855567884,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12034513295991798277,
                            13360782581322425245,
                            1725302584472980609,
                            5860344302874327127,
                            585907958187382909,
                            399850908026468668,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6365626450037446919,
                            13899131710773498567,
                            12460495842367859121,
                            16170929542485821506,
                            208255995547163999,
                            1788709529122513090,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11608572617590639088,
                            1490066462143332180,
                            18104854906992827701,
                            9920434731054710041,
                            211765713996340813,
                            1553339364874916386,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9672666264253487202,
                            10336654795350669889,
                            449862483373446854,
                            8801793815254199265,
                            1432193855307402981,
                            1150842027610094260,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16630684536941166445,
                            94594638205119849,
                            6999348924643195115,
                            18325706953800674390,
                            7488829312874030254,
                            1277679837592286485,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2043409913398544163,
                            8799985458975684188,
                            11810346102690400206,
                            7287894179620391849,
                            8310030298617803903,
                            72032388626634878,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1281687477320199555,
                            7563076564081146202,
                            12967015875403459476,
                            14624806766159224062,
                            17996965057478167606,
                            252697541808936833,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13793957312069841765,
                            9280360942893255165,
                            4205627493290145795,
                            17041753664664484143,
                            292481827112153331,
                            1625837563384118732,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9352821740625689875,
                            6308883394178894773,
                            3845557142717715027,
                            2629762210321359507,
                            17751017925156195727,
                            584390040153245303,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6686080398875968317,
                            3061215826479322264,
                            3497897865939501121,
                            18281203246973482576,
                            17927392922522140979,
                            210210068325921232,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16607078842599531378,
                            9504606871982910509,
                            3444663148800932338,
                            13100534280090774060,
                            2271611617452010488,
                            330988216624333302,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15201230330916224449,
                            17352678142014047801,
                            2904416965439394519,
                            13623439688205147735,
                            15906870593512811607,
                            1219691388678138207,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7840239581072302125,
                            2230979975670043222,
                            2803040505698332898,
                            12053299447726410370,
                            9133499313447891576,
                            1792952726533648441,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10887159154609486766,
                            3586570219945765498,
                            16987857331424166578,
                            17227117211665904563,
                            2641362075356050937,
                            1107000584903662835,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2913822763351979027,
                            16605086612048089786,
                            3073014559686911583,
                            3859287879108764710,
                            8979599193271739338,
                            717888751414334418,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10679990968047840039,
                            10565037045739067909,
                            3068293805934655617,
                            11345115244705867351,
                            819555464480452960,
                            1359092330283576500,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14986598304387577492,
                            1475342186073787009,
                            10690740891793407718,
                            16999563299462835243,
                            4555879779713993726,
                            1804953060349525147,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14718054382508842189,
                            3739789684741379050,
                            9034135208882203942,
                            1626888902243251070,
                            15015377529971837185,
                            1774026940931530558,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16283842137357890083,
                            17042287683087456568,
                            9283959198140871618,
                            5116811499550729674,
                            527971125133441524,
                            1195755169769435304,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2263768794863831072,
                            6053804060596607861,
                            9304430160953566875,
                            10800763975545038529,
                            4491057833324637918,
                            429009324512387952,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4474279464499229184,
                            12996303765150012187,
                            4641054898176728295,
                            6223252192051069941,
                            18203596112407286527,
                            375775551469839185,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            8672165050655637275,
                            11592407134014470035,
                            12429878132507516313,
                            7584016740753175759,
                            14004723765673647138,
                            652321248730214643,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6411316145341220134,
                            5097903652746372194,
                            3084321987603443066,
                            291263048266979464,
                            13948504323175578887,
                            745571255052464834,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13766498570618749733,
                            18086239932265263044,
                            6267759234237396905,
                            7524577637333901477,
                            18426300225099406723,
                            869027842015464503,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7915505991108860376,
                            14744184946864966602,
                            7841743679530089832,
                            6342118044023938028,
                            16924058188347940268,
                            921722487859844584,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18432711285445907477,
                            4799959705089732945,
                            388658094467292109,
                            13469196397244777115,
                            8027960956661936508,
                            1856146042798927224,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9412562462505690188,
                            8821023218336407305,
                            17692993600679959856,
                            16005771856109996480,
                            16508814953731959950,
                            156651092503265291,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3784443491987350905,
                            12821878762449544181,
                            15570828798018678235,
                            17369955329858634994,
                            3933669309037306365,
                            246493789403749385,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16488715180078991174,
                            15204639058814585608,
                            17282981194905586800,
                            15264446678800350625,
                            3984886682381078986,
                            1671127256012160488,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17590970556421112644,
                            17176240063847474694,
                            10088027845441536104,
                            6389168435284829065,
                            1591653901588710987,
                            1149030296320687731,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6079225404636231740,
                            11577093260786694347,
                            15113701608174770334,
                            2514789638500123591,
                            12818253947778157652,
                            973877333193733730,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            122455346643796234,
                            8815451192692205966,
                            11588754695599824954,
                            16375258168570638365,
                            10573408258322703739,
                            1309894564644849748,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15507485838998532052,
                            12273896981046538735,
                            9564646143178881986,
                            9799403820679217277,
                            18407515367424986612,
                            1458100371033925231,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6741837792526180340,
                            4741093931866041713,
                            11759041003213664145,
                            645125858661861983,
                            1040599062942109222,
                            1094091021380223034,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1997113849244717466,
                            629467846506294903,
                            8233285395657023714,
                            17967730458865455833,
                            5557919433109374072,
                            53265650382370398,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            111447671463676950,
                            4524009178586750554,
                            6241552241237694589,
                            15516227124308709619,
                            1634653178781246845,
                            308225627496460724,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11425290477616553189,
                            8987143399419740775,
                            7242024229149284738,
                            9341911953611946022,
                            13875120599558885654,
                            451265879496184204,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11256787455103266093,
                            1868940201283063058,
                            1806970943669599128,
                            11534684408870469118,
                            11050307924636247838,
                            1747625844835066119,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            55418155768619976,
                            2034658883971349892,
                            18274630924121538016,
                            1665817702737637819,
                            1836525310264334874,
                            1087170227886131971,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16334506042230835085,
                            3206237476817598184,
                            12402834128857567616,
                            8339689657317707135,
                            6029051804515072820,
                            1456846465263568157,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16766707407501672142,
                            1887530354787759486,
                            16790922829511246380,
                            11009431664827237384,
                            13153188622730201576,
                            1838374361829005667,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4910545072837641961,
                            13403460400440102710,
                            5273352206218781216,
                            676697953860982177,
                            10825617927840762982,
                            285154055105445443,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11746400297651516832,
                            10979763446858812125,
                            183692602325584858,
                            8498320789284697326,
                            462797629762387415,
                            576405248258865140,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            777911193742572296,
                            7647513902921402961,
                            6067446540972771385,
                            10790916516077108642,
                            17675182722691613012,
                            1751686398229996040,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6405068608950491031,
                            14140957542216444822,
                            2309070038411189322,
                            356329552979368518,
                            10765941306340344398,
                            116831036886342988,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9024698106098702623,
                            12296815156425022583,
                            9784564377917285395,
                            1404052146912132637,
                            14541191129419624393,
                            1226504296184078727,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9027381342836181758,
                            9138486834763091990,
                            222415840784199390,
                            14489635729577468753,
                            13929972455917488456,
                            1786529869997611434,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7403194024900695680,
                            5637255599482643285,
                            5375494383863040234,
                            224644731955307803,
                            2873214725430145840,
                            104229465812347909,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3439363775230345751,
                            15638073239631313515,
                            9492418863020281360,
                            8131969094489376602,
                            5720770235757025622,
                            483724292851919853,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16699187852082723843,
                            8693120683959256561,
                            16145520580880843176,
                            15514255596695428931,
                            7841666762005833637,
                            420652186032170947,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14335513727543274208,
                            16646213806585277802,
                            12925600038488807498,
                            2605225084328483855,
                            14665570212563299912,
                            754384477496729360,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6694419024724290957,
                            3871170988293794892,
                            15732304184831754055,
                            9738283805219832287,
                            7675180625074030238,
                            480847214481638638,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4529064992016325751,
                            15671417487523988332,
                            7508561555645614607,
                            18434413552791868384,
                            5809087897244316529,
                            1676080464407039490,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14253176987565140087,
                            8432973936757336868,
                            12536150827545251950,
                            15984093800740611917,
                            330194173108652513,
                            720863017847274140,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2504083676569153968,
                            15832083804081244200,
                            16877291253304412991,
                            3123117264403828128,
                            18348442583875952540,
                            10672200961417660,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5638240506016061059,
                            1583690574560387490,
                            17099322707903237119,
                            4602001013727840879,
                            11805945594538167489,
                            769932996100306776,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            13389054708145450145,
                            14824985038812446465,
                            17048423736687968055,
                            8814579705583440961,
                            5035209798269158033,
                            749603826618513244,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3328160815093979948,
                            6237421512227118148,
                            5454867442369564245,
                            8360606765311549714,
                            506884495890557126,
                            1337844504046619073,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11246208992146705102,
                            7491507820739201488,
                            8226502557152353642,
                            8242655725648707598,
                            2563614515138514716,
                            1276767182461265012,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4769926818852819626,
                            12326920208947329191,
                            7133440403278935560,
                            8217617849285520661,
                            16049604563650375359,
                            711475378941333023,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16849766076317370723,
                            10565333070263476703,
                            3115741580833081075,
                            8928575428429098701,
                            83956234334344128,
                            926347158376151918,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            10143750035226921107,
                            11175567661712278494,
                            3216221237041612947,
                            12620844773409697702,
                            10707220252741199768,
                            1623526040161124106,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14447879463473006980,
                            5619619168231237359,
                            17851493197815746321,
                            2472919125584316304,
                            12932681200314166169,
                            32006887797470998,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            6733468596582028491,
                            4938403331373994423,
                            2495886566350463086,
                            4866913292325304512,
                            17833122831271837090,
                            20747965259049769,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12123540221335456053,
                            17481962647007948085,
                            17714790377634962640,
                            14772495719257247834,
                            18145394591723123802,
                            743560198215921556,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1497908276767573553,
                            12116779752273264858,
                            2590346265392281813,
                            2332181715169792445,
                            17092360546369766522,
                            1785077242918424407,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7952464444820094318,
                            13994002917888001241,
                            15706352139461318104,
                            9628004532299569283,
                            8518954296988900559,
                            852566979184224097,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            7441466901095813604,
                            18132532258406101602,
                            18424073174602445664,
                            10981127994613099451,
                            8652506872459021220,
                            1468509449400294373,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5657596159808172880,
                            13526374509927555697,
                            9098455594626067101,
                            8158374417330316391,
                            17542823587561952014,
                            1304079557315137315,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            2947031150797965961,
                            7168775351093980616,
                            5402014775348167763,
                            15061123952343550529,
                            18424749141952968,
                            1371189299929536049,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            9281444850959927837,
                            1614617486747393747,
                            11241920734328704958,
                            2097073444320196800,
                            17161985834411206816,
                            1087148532604594683,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4447772820428775845,
                            12960992846973464060,
                            16293514322279992379,
                            12893219225638396788,
                            7781299197431263334,
                            449901882076709110,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14907262724371019648,
                            2800189046514750300,
                            1193812845074755789,
                            17554157846753588042,
                            11314840220597096532,
                            1498650060415121036,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14577735060881878484,
                            14619568849073463350,
                            1431448144111700444,
                            14528704181850128793,
                            14501293334336711131,
                            1612428065818573674,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14062135068179357635,
                            15712636924265402003,
                            14349265695217949070,
                            9810173535875805004,
                            4832166941298165361,
                            371959303338356286,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            529620776199412663,
                            11279048013297394977,
                            7438985769468435174,
                            8418833291556253638,
                            12638750430770189374,
                            782893910818976383,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3425819771147962005,
                            9716494799582968327,
                            16725193404816337991,
                            17445947387384886143,
                            10172343138072657791,
                            1282876810846454564,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            11164718869065048732,
                            17084654245035325868,
                            6328765895618868472,
                            4486022148862603580,
                            8280153517244863665,
                            888195886403406946,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            18324010328125972459,
                            10571572315927437740,
                            4014634975442021698,
                            11429030044824256915,
                            5271595227066846426,
                            1674840201622272053,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            16223747653705116452,
                            4719335504099331335,
                            12299381611509085527,
                            5371163119486524817,
                            15960986105789375268,
                            1057686279051569806,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            3231538934033646755,
                            5344125384418809110,
                            11846526592348732580,
                            2299306435200444082,
                            3692252847535720845,
                            575369555574054056,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4973247319762195912,
                            17368940014671360540,
                            14839003924064767409,
                            3245510716526580765,
                            3436906785389213579,
                            844931647848380362,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15165477183340963026,
                            16997843586210142388,
                            4445003245359517877,
                            10927166688015442507,
                            7520430330020049843,
                            1420012853112495986,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            17593407237472846452,
                            7139603939882108134,
                            3757323610542394666,
                            4472807864131808150,
                            3386962978897274091,
                            1383468498082191913,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            1444325385217431665,
                            10835112227600171299,
                            8404917474308882742,
                            6750769422842145945,
                            13441473813250589699,
                            924170415944226904,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            5548361823936492326,
                            14178292541188102100,
                            5559690584309841024,
                            991693879832300891,
                            8159539515805530943,
                            441534492458820411,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            12926075692359746707,
                            2076456620501910467,
                            11812451018956222528,
                            5858709688062104928,
                            15645714274888629921,
                            1083342478455859008,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            15181447811226672777,
                            15314942397772641642,
                            10472930132225485603,
                            1443381424108076475,
                            6075609957057421500,
                            1525570165854124564,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            14187469699259158108,
                            1543752628374803254,
                            5434623545917424877,
                            11549575584320809126,
                            5114744709675570251,
                            1705988908992398926,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff::fields::models::Fp(
                        crypto::BigInt([
                            4574256762420014476,
                            3109595645412534119,
                            18207041838432318466,
                            13843871776053619491,
                            13206492776194894486,
                            1386537781114910309,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
        ],
        infinity: false,
    };

// QuasiUART::new().write_fmt(format_args!("pub const PREPARED_G2_GENERATOR: <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared = crypto::bls12_381::curves::G2PreparedNoAlloc {{\n")).unwrap();
// QuasiUART::new().write_fmt(format_args!("    ell_coeffs: [\n")).unwrap();
// for i in 0..prepared_g2_generator.ell_coeffs.len() {
//     QuasiUART::new().write_fmt(format_args!("        (\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("            crypto::ark_ff::fields::models::Fp2 {{\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c0: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].0.c0.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c1: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].0.c1.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("            }},\n")).unwrap();
//
//     QuasiUART::new().write_fmt(format_args!("            crypto::ark_ff::fields::models::Fp2 {{\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c0: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].1.c0.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c1: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].1.c1.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("            }},\n")).unwrap();
//
//     QuasiUART::new().write_fmt(format_args!("            crypto::ark_ff::fields::models::Fp2 {{\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c0: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].2.c0.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                c1: crypto::ark_ff_delegation::Fp(\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    crypto::BigInt({:?}),\n", prepared_g2_generator.ell_coeffs[i].2.c1.0.0)).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                    core::marker::PhantomData\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("                ),\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("            }},\n")).unwrap();
//     QuasiUART::new().write_fmt(format_args!("        ),\n")).unwrap();
// }
// QuasiUART::new().write_fmt(format_args!("    ],\n")).unwrap();
// QuasiUART::new().write_fmt(format_args!("    infinity: {:?},\n", prepared_g2_generator.infinity)).unwrap();
// QuasiUART::new().write_fmt(format_args!("}};\n")).unwrap();
#[cfg(all(target_arch = "riscv32", feature = "proving"))]
pub const PREPARED_G2_GENERATOR:
    <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared =
    crypto::bls12_381::curves::G2PreparedNoAlloc {
        ell_coeffs: [
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3052494208186889246,
                            711327711884005890,
                            6566687770021815863,
                            5689367844038720698,
                            7839048598077007543,
                            463225536391688408,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18274513633322999197,
                            7165038535871059952,
                            5296893277260269221,
                            2421305131719586744,
                            13058838746739291576,
                            1129911328936591389,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1452074148275413176,
                            253517005792379887,
                            9889456999658790348,
                            2139576114717959168,
                            7439043430229261887,
                            1475740478566097248,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15661538434425882917,
                            971677572174623317,
                            5425596419450720164,
                            17937140564850191591,
                            12663984048921288312,
                            1351703496531388632,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7364203494592516438,
                            11468364815586117904,
                            14118104996076872486,
                            6001166368081071924,
                            13695759462986195410,
                            1300434096886380372,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15822649961347786769,
                            3580670598919003671,
                            7693182822216600534,
                            16277225726105488061,
                            11197118583593117420,
                            58015165862658380,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17790198068728695860,
                            7234458935014357,
                            6645308713270276386,
                            1108965528716371484,
                            12383267390965539511,
                            680934450938463541,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7108751515868695173,
                            13681906071608324747,
                            10306804698501353629,
                            12202873431168746666,
                            15133301835759971511,
                            776431533383261389,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11509412196563824554,
                            192340567597275869,
                            5624786804019609955,
                            6977812486287659700,
                            15635469719964212210,
                            166719666163808473,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12077570478074990842,
                            17711703566155220638,
                            939474816353470333,
                            18145514887238235953,
                            14203996091400231011,
                            1359844821074773525,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10311604063728649511,
                            1609651387857124885,
                            407701348737146113,
                            12028285977006890812,
                            11046164876796102767,
                            1797722042950904141,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1427879064635928480,
                            6656285964687876423,
                            13375145963470899141,
                            16710347741010854922,
                            12854807559074243563,
                            1750322764529718003,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            600216654275545583,
                            5414333347850592515,
                            13425270001663106572,
                            10061215446213342502,
                            10898293112961830784,
                            1581383153314888180,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7953911101668649384,
                            14502371485567639771,
                            4467223998604199670,
                            12953513131685627027,
                            9787222643454963879,
                            1689317564815265251,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17845090528666511814,
                            17218857764041981633,
                            434321486801145184,
                            7761449053373540029,
                            8320959589622445090,
                            398786852714584690,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4758378269505062716,
                            16351704614477589866,
                            12542090468608143805,
                            808605318425639446,
                            3503240832217460051,
                            1842073828575200413,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15734078063991052488,
                            14805981347834446115,
                            5358165178364389761,
                            7797436101080855272,
                            13897454754534204624,
                            1696458052295004902,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16649621874459670687,
                            10429050216862110273,
                            7460627473180391868,
                            11265217268035742568,
                            2735712989743781811,
                            882920157912079811,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3075135809246941167,
                            1089505841100397141,
                            10076243511248066921,
                            2394028537904061176,
                            14754095266737483210,
                            297320587567923809,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5363564499110814822,
                            11867016904495211986,
                            13941697407403803546,
                            7664856459874461452,
                            10933922705689004674,
                            338358933332895847,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10809564516168079915,
                            5737027856357256894,
                            12892628667917951165,
                            2213734124659505572,
                            13092779794933989135,
                            1323409123077895720,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11131448777240685080,
                            6925404385650247621,
                            13622446378718023885,
                            16721307281747360292,
                            8474077144991808591,
                            1340983569203099814,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1528672006511707373,
                            358890670182426684,
                            15883787751024872823,
                            7589192448887170302,
                            15339649564474758394,
                            178286491281225612,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18430688005093571951,
                            18162952206196859832,
                            3494563849312283207,
                            18436405466476566618,
                            7705418686106117642,
                            1295907200500658121,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9864917128818935680,
                            15031094304689192001,
                            4383772968751846037,
                            17381792424027520744,
                            16117299811085039800,
                            1220353500302315717,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7855118772709740126,
                            6540028954535862878,
                            15292103460338394486,
                            8060281849357684272,
                            2633944968963670217,
                            476348405468463137,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17765440231526072324,
                            9676826235115956546,
                            2705546691226375623,
                            2752247537313589314,
                            6877825397110780365,
                            1828782654102598044,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6867090617432332364,
                            3356641605717282471,
                            1705574496795018025,
                            12956011776884799676,
                            12701038320376644602,
                            1533226673826508571,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11842521177258981316,
                            17960613677823993081,
                            17452903803859729890,
                            2998747472929348727,
                            15666480210421280333,
                            940463773828130626,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11370869085942358320,
                            3523741466631425292,
                            12392458386958233900,
                            4512654000554617645,
                            383560161132585504,
                            688936409456362894,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3654900830952232285,
                            15816179799092201856,
                            9701466186991521955,
                            1204285215825073102,
                            7196014837569680243,
                            1807440671971120011,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17446673905646733460,
                            2968035069490327605,
                            1945422452553785097,
                            6405861100054680345,
                            2026663606117160557,
                            997594189683734898,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1827682781581716425,
                            1794243432359512807,
                            10605178223650739435,
                            10180306364887260735,
                            10579069175736778113,
                            484225598454989429,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13875111464225835617,
                            7691967656974318333,
                            1852918340622303378,
                            15864034991226998710,
                            4393910526263175436,
                            511665796839647158,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4783884378215718113,
                            4540621619551195186,
                            2850751512615260323,
                            11773064065889588953,
                            13178808825093108581,
                            326694971871283252,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16487667098820688560,
                            7710334756817029526,
                            11101831390421135721,
                            10347240302317568005,
                            13174135076480492483,
                            1323788570128371160,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15197103216391778603,
                            7110760572534454589,
                            5219725019100337327,
                            1512717616051337044,
                            9187078406523764602,
                            1764851362557002371,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12608100880885305675,
                            10953686429999876952,
                            12543112497099631400,
                            2213901796316081859,
                            15460166960038713006,
                            1256809923215280715,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16631344828141006385,
                            6254972786522134245,
                            10178119461507741560,
                            14045843476639986127,
                            7656771492118495314,
                            1458377839318352273,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16063330626818798199,
                            4650541829620065595,
                            6244614876404924905,
                            12395055914689021083,
                            3580369191028499350,
                            189068726599598796,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7912908214663280303,
                            1244539527153588649,
                            5217899939295510267,
                            11246602870937322544,
                            3496787014304545030,
                            22086255942919046,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16660790545462286755,
                            15160923557348826284,
                            9102870716234932206,
                            5629797427135841108,
                            4563311680313564885,
                            1739212864458319654,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9319999008871529158,
                            1124441984610024382,
                            15105338731927575319,
                            10340048231023281141,
                            3861773543980233727,
                            347834781517311721,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4369015729079879062,
                            3940604960755961225,
                            13659005392738124966,
                            14491388806605099967,
                            924775484483583890,
                            85755882404132494,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            374519509947936737,
                            17862280374400650099,
                            9668557401249605416,
                            8272969037950968500,
                            6717722522445555135,
                            1030247574245112396,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4961476162914894405,
                            3854848957015663757,
                            10189214470214276238,
                            14205229339709781387,
                            4417218397329879373,
                            1601596046767932803,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9191756317311960181,
                            9444864100088947659,
                            6855189798868853640,
                            1471448290965065107,
                            16738530168606230153,
                            361184433356738033,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12498591122706736175,
                            10563461173567731152,
                            2925870601692348119,
                            11739092067567402164,
                            3890774621640006655,
                            988568926197161883,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2851434790580815098,
                            14798034404149098704,
                            18334627029335818218,
                            1412439668022549114,
                            3330133603868618930,
                            482481860756062252,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11076480461501329533,
                            16425796211568415510,
                            5884360154509548442,
                            12741764688653511774,
                            5160897053009606772,
                            595985964684473617,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17481501968826625055,
                            6163383781568494616,
                            18140810454921260541,
                            7019729729609510306,
                            747657912668422666,
                            921218748400401184,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7910412861657422431,
                            14299206070802433860,
                            10904934023720001907,
                            13410482876149709820,
                            2384958978626215837,
                            1761350086503203196,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11286239120939214170,
                            10480930836527603110,
                            9760679216982824116,
                            17730897538364837727,
                            13233235315231713501,
                            667767427353596108,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7540281879100466063,
                            6000132550567242042,
                            10322251115203216968,
                            5574113681572046487,
                            9677027565739714424,
                            87224451523626278,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18135865025704023367,
                            16513046342324158514,
                            16215105130421488099,
                            8722284556001790773,
                            4993224110182347144,
                            1374636940574782372,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3396784971714920252,
                            3669075242798289615,
                            13180640745898139643,
                            14578171727536928280,
                            18223913146611729625,
                            1436390993491948246,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1131222950417859892,
                            18437492094497893295,
                            10228698798272953367,
                            15266850504068439050,
                            7440002742821659916,
                            897472557427209111,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5703884490550046660,
                            4276425947477962706,
                            5888204514887075484,
                            190362201139587780,
                            8825201290298479522,
                            1589658797046145042,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1693681404120296262,
                            8107007286509222730,
                            13880247749171669562,
                            1552200204120423577,
                            18307963471833595549,
                            800387677220048762,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8945314065479858363,
                            17536932884072547343,
                            17606452840230861237,
                            12445471600583414576,
                            3153945872818689304,
                            265371597851563234,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11807825517325396323,
                            10434780438827269225,
                            14921102236137645150,
                            16397929080195504057,
                            8941793830034260256,
                            1240671243615941116,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13149842342568918047,
                            8419575181823323159,
                            17926972879034340116,
                            16683577246991711257,
                            13248515901044517011,
                            804124383940895913,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6828471912622968418,
                            16852502461087490797,
                            4917095303035664428,
                            16584919885452493822,
                            13526296253423517608,
                            37068943455264989,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16429150428479559360,
                            6090358362583706423,
                            4337862606197229482,
                            17316819767602712848,
                            13851062445815223654,
                            1552858673892465146,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10476947128839422004,
                            6419637758485170305,
                            16788849621669907831,
                            7018744208196360573,
                            15149867409125287793,
                            1146745591388616820,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4218854713723296212,
                            5746676787536371490,
                            14664483290659652970,
                            3132817558113706909,
                            16066778071633692760,
                            1547179964279102902,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9604492757616667993,
                            2115876842065284597,
                            8700275535062844165,
                            15456997435423204979,
                            17942869472006406032,
                            1868140914031865148,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9601925843077376948,
                            12550574767746444407,
                            6308390619541534427,
                            12831025143367490374,
                            12739863380576549251,
                            1246186122007818689,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13357099077526248157,
                            10927982722792823798,
                            10355180012280029484,
                            15157294740713617558,
                            381448918254113853,
                            1100388803663847999,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8897621459645018997,
                            6607841315862345,
                            9313793248558125897,
                            15624010497408979186,
                            8671122233306691565,
                            1001600543736992533,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            233763342586661333,
                            18179496807331625382,
                            3454653555867713179,
                            9531316003525379528,
                            9805407905792566628,
                            619082838981481021,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7466111381324945590,
                            6613179136665616618,
                            16537610072355960083,
                            15321234996977351684,
                            4591215788778961810,
                            180912637343132148,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14908397204954739241,
                            17752955875838930311,
                            6508116482146656851,
                            10162180830810913835,
                            1238536751827215150,
                            442432529196498911,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8685430211725868320,
                            14366536341838877914,
                            13667556712476441866,
                            9593963582625335931,
                            9588752782420041864,
                            1510953500679319808,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5860083087492309854,
                            5354396228463740899,
                            1425301045007190601,
                            11427985354370956058,
                            17281790371390405961,
                            1738975714387803877,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13971116820166620450,
                            4037790649053401381,
                            14000789417085376619,
                            11786449676905252380,
                            11205693916923363934,
                            1145122980978999318,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2362525035475607372,
                            10790790573449293274,
                            14709620761576913869,
                            15561886936187009288,
                            17552799920143622317,
                            906524521413690162,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9148580777855278525,
                            9201619466325120746,
                            12222303873948797905,
                            15994685648620839506,
                            9012843103690271364,
                            349590262130508903,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12030596403131774691,
                            5400724023463431545,
                            2007520327483956508,
                            16501441191786797857,
                            10580150472454046540,
                            317051644950151745,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15738735580091264303,
                            10740382260027257510,
                            10061537908143741675,
                            11692763105448096918,
                            13832024148065089819,
                            797755810753465466,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11841580167713483080,
                            17433056336083848361,
                            17600141014603514811,
                            7565089146036459146,
                            1048358437156680825,
                            1451719748835728003,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9512987442857176271,
                            4602400208090369990,
                            14355585725269563137,
                            1458942752130196069,
                            8648734163309770138,
                            381272633757044507,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12230123046173387545,
                            10458544878303856304,
                            5008678336870036485,
                            15145867043167423229,
                            9722102766567012074,
                            846665048281601968,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5555457897525984402,
                            8970910994860824309,
                            13653925456085361596,
                            5701791226860105175,
                            4954555471822208440,
                            346474221626074939,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11268586486507521827,
                            6249786791175931661,
                            569118417960980033,
                            8473695511862241269,
                            2178494136187404976,
                            431432387993630182,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9767818281574084775,
                            1391450138935987945,
                            5174593879919979238,
                            17756379008181005275,
                            13001876029787805667,
                            1489785183613606589,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15585933096465434235,
                            2121643990888102447,
                            16273683044007784467,
                            10943999900411221160,
                            16688262079688955122,
                            1595420476499348304,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14530381411507886250,
                            2914368546624793584,
                            3110585286439612066,
                            3117909033431974203,
                            11628027506324341403,
                            1640143277118629030,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11061033095686422711,
                            14510911052289775026,
                            12034051224375857437,
                            7428201691196570824,
                            6105260842572334121,
                            1451798730407105563,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16348332367934394563,
                            2846386206927834309,
                            12710883081606547466,
                            10238716325422620291,
                            13479719958085072741,
                            1169039471489542700,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15724428694805818516,
                            1118188822170890733,
                            10020289466996559698,
                            13196527384266001734,
                            14229921070597705286,
                            453131383027160541,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1886492232467677427,
                            9155808694056011525,
                            3855631134365137616,
                            9746157117637807777,
                            14598754169850167743,
                            1256752014320216960,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16900211377735904598,
                            9541207940726013150,
                            16142430519856260861,
                            17448239548293786731,
                            12218177147651901902,
                            836840490789639547,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2886495440003846356,
                            14727056408252187117,
                            10767092398945987636,
                            5949399035132223748,
                            11110114335930746748,
                            266015121701179733,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8480143601674336423,
                            15912081855712111904,
                            10935439334081792176,
                            10240870886296076087,
                            7783194744692064149,
                            1131521111249516224,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2558463592596876794,
                            583958213346151011,
                            18014542345104299437,
                            14956299395006892573,
                            14864400237068714598,
                            1595584330988464848,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17446564942590282068,
                            15429666926326422399,
                            15694117050543498354,
                            17826505725541075824,
                            9405564886934799689,
                            443256447757001408,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18837462123398949,
                            6536924409262152549,
                            2675833911653007748,
                            3410516367434162110,
                            12625593918426696197,
                            1541303055058449767,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13750684621528149303,
                            12629231418344635697,
                            1857508212202154218,
                            1790093184673799050,
                            15121411427550266970,
                            275438851450726136,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12276493280963925407,
                            3270358995025284257,
                            12091953896214483072,
                            17910750599704837367,
                            14264475547154288821,
                            313143896161381714,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1332146746383296118,
                            17255736098587014279,
                            14816887651067907559,
                            6055520764886473228,
                            15317148872479131991,
                            871437953543889679,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18295607816066475325,
                            1067084375565372712,
                            13673520917124049324,
                            10695501982698404256,
                            12996018413156863904,
                            1325536126148830591,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13924501321009869428,
                            8634265851493182010,
                            4511029830559118103,
                            3568064270776750677,
                            7329112494530347235,
                            1532056137037072987,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1427023320611130676,
                            11960349193676796020,
                            12013030065653224620,
                            11872273267124159146,
                            5995878413815735116,
                            125875327340602563,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9049564332870174035,
                            10378124515149494580,
                            3324550182692375582,
                            2885173350661320828,
                            34625098653913189,
                            50536490812094212,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2139290107079644491,
                            3285366231758916967,
                            9776713007062133069,
                            15325239047839657685,
                            8074746099920232094,
                            769784250016793204,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7013078215331620411,
                            13516480407259643604,
                            8172543037750184157,
                            7102370402364549573,
                            17550140175781155590,
                            1820141719382774601,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16977843566776350591,
                            16597733624496371182,
                            482801885502878834,
                            422230165138351313,
                            12194242086364288008,
                            84168290893678564,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17746708234002147313,
                            13890082798757834104,
                            10486693427777579699,
                            16630505944025832898,
                            11209831055963795396,
                            285998332440715959,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17942077489794309785,
                            17965711880365933462,
                            2709080271199751297,
                            13524235603042559027,
                            11928075245847959425,
                            1582647677606540166,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5061566519242798271,
                            450345864977296986,
                            1926462603861529059,
                            3116689399115277034,
                            6411159212323990026,
                            455959799970799358,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10444940123659646236,
                            15112464026327313603,
                            2920813983989772818,
                            310434131318387232,
                            12881944387060054284,
                            1236393594208692022,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14901157661291898361,
                            940142789539996102,
                            12360162332234946476,
                            1504475837926657830,
                            2997137965923495864,
                            1263219474182695744,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            504969174943307686,
                            11128967441430027986,
                            1162156125259854108,
                            7755981445530474051,
                            2039238058185356728,
                            22586689550108379,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17961087863949889222,
                            6422430546967149933,
                            837008314542894338,
                            11276355777110081806,
                            3412551651541014448,
                            841337816078151637,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14872825222809788877,
                            2393451172414746646,
                            413538491764256539,
                            12933591748032011568,
                            3351581556648283258,
                            782596482577880205,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            504746258239733438,
                            12085326855307455916,
                            2280723197161719455,
                            18422591298722725984,
                            542390479264666327,
                            440058484034435178,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2708755935376602004,
                            9132803234669402130,
                            1977009408258888207,
                            2358367280359692710,
                            11294311373456056947,
                            1029617061087637256,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9481172343655531671,
                            13986220110339277816,
                            12677059319686112944,
                            15805358118076412931,
                            2487348979304608583,
                            180250936137238246,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17450045029473327362,
                            11526951174892935519,
                            15388435877947052910,
                            8472747805280844831,
                            651440017552360636,
                            1032884450055211391,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7365176830944236124,
                            1407264094595173047,
                            1707930433331235926,
                            7095441601662635015,
                            12966215563198344774,
                            216601671243634782,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5818602704773162449,
                            3263430439549258426,
                            12261106858523380353,
                            4532096764148239217,
                            10588692139859299584,
                            678860834220986584,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15886013249971217740,
                            15749120917128221094,
                            11391223627947230156,
                            9739960937417431216,
                            11930629035788909358,
                            1386042796129958645,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18009758323768426626,
                            15688526785026375885,
                            9052672553787857201,
                            17098084525049714177,
                            3429682098614426773,
                            1635586046779065050,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14480615821344381108,
                            5320916422073334579,
                            3874323348917314111,
                            14239419032624646267,
                            12170702702485978593,
                            62667813100750931,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16633532025534818497,
                            5105605526354159200,
                            116371485141499325,
                            3563043692527578234,
                            9743280782433467986,
                            191734893958064739,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17787579823268465968,
                            983147809418282055,
                            10436753112367891997,
                            12051421020351637881,
                            12808283427148587540,
                            946523263266965056,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7557753804677190402,
                            1868689416192226865,
                            2866793430929842363,
                            10579741929816299011,
                            4224179535915609218,
                            522118784290938149,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16049443653289505579,
                            6234045106681390982,
                            17112958157902276426,
                            15933239406921575515,
                            763747015067199370,
                            278448898884693470,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            597828249674798581,
                            15524987575546089282,
                            5821577246680231494,
                            16101698029334489436,
                            16425445765084053140,
                            154490486092510850,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7641175813644237396,
                            9242639387815443767,
                            16868651575364005258,
                            7632657546518119079,
                            17360613418510436855,
                            616010438738443515,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8908622966350013699,
                            5172610524429074474,
                            15319313558257893918,
                            14141228860593512325,
                            9125973363051074026,
                            1025929757195792118,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11079793026469425878,
                            5414435832040434577,
                            14077833142206707810,
                            8777731868979745596,
                            10500096573841787115,
                            1221427165298938941,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4870370613682753556,
                            164177066710720218,
                            4885021811586536205,
                            4281156270537918073,
                            2510281751820711505,
                            1595488243230804195,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7483942331952467359,
                            2818612421376491431,
                            9635005131040379430,
                            9236791352905058808,
                            7286334090988254649,
                            1847335700136917976,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15789678139086319610,
                            142097753968991594,
                            16388870877495489255,
                            9569490384952355680,
                            1031136515789373161,
                            1713253222209724456,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5785788737519094651,
                            3716212816220847352,
                            8604842845722864056,
                            16599114859176771243,
                            1145683043225518700,
                            60028473766776274,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3250238917270767807,
                            11464160087753305710,
                            599576521734199650,
                            6557388830862983125,
                            17062214447675692798,
                            877391496045873843,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3755319647652493377,
                            16083655437212405702,
                            2193672529072280075,
                            5996594255592331426,
                            9429139041287621803,
                            130205385792630692,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10383081724930109072,
                            12593861536066275986,
                            9092363334153095535,
                            15574659385312906131,
                            17215748711877627119,
                            1040145223288196417,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            643656109511062601,
                            13200658324373371178,
                            17206960108259696188,
                            11233738938246382685,
                            1055383647862133087,
                            1486396485995704042,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7634501443760909685,
                            834077210104955412,
                            9929619938206958029,
                            17890760045475262385,
                            2899753784719029241,
                            643667204327897670,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3174268054371490465,
                            10009757553528320724,
                            9159224366562372825,
                            4159108474050767996,
                            12382873007048194202,
                            86929289079880902,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            934716554789158677,
                            11794576519459193490,
                            1977297271684512703,
                            14651667789677145666,
                            9221423271519112098,
                            1017589536715774935,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8056025269452223405,
                            4079142411217425591,
                            15019148581930066391,
                            5618626673969278649,
                            1350278659055051921,
                            801622961883930833,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8846977164480037261,
                            4622977331006727655,
                            6185667009816791126,
                            4677603835163498196,
                            5955950476880706146,
                            1326492631786243205,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2967091482354529926,
                            11129988023377312234,
                            12676243656597383665,
                            1817413047939933819,
                            5644784580405798802,
                            556036227602466225,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            85845160107860753,
                            11247660396890151242,
                            4345592639492677279,
                            6734602623216239891,
                            501496018454030390,
                            1305232414753438920,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11148234907557086529,
                            14462342730806719817,
                            1785717931334717881,
                            7113502317257651687,
                            8809028124916226331,
                            180149914168708407,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17211462816625525513,
                            10067535613947661868,
                            10048693001446878550,
                            6426636549308521581,
                            7886927805009381956,
                            1210983796369315321,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16131450706276282561,
                            17975046937463708603,
                            4939893144681836925,
                            2534853622172135159,
                            18134417849241909481,
                            1039396520715138618,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15026916231730850029,
                            9811555623371877965,
                            10967470596638318856,
                            15048633004968624682,
                            12933825869646574721,
                            659583207878584315,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15244656217298296931,
                            9430831216212058572,
                            13952810780591350141,
                            5119385460660822351,
                            5772706368926564439,
                            599415181697207289,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2002581819720380730,
                            14785763027960546945,
                            4005577012622447306,
                            12289127541534828740,
                            14349838012616278704,
                            70983718740382502,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            476033296859622683,
                            13384601279375565993,
                            13068138190975066339,
                            13127375079578290872,
                            10764493554294759566,
                            1381054862658855899,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14401491584607695384,
                            16611457376406063811,
                            8533329089852311094,
                            13714113107816451663,
                            9385048128150139416,
                            265755515749398785,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1758081742691861576,
                            10029471554926535266,
                            5579104807043479358,
                            7426552216895414407,
                            10603561323263022992,
                            1828359084145060288,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            671201617396467918,
                            11270714995297395167,
                            17595200338417156857,
                            2997793723051414075,
                            5476069598428923542,
                            853505663081356732,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9475917557857477054,
                            6836565551475246463,
                            9636318994271348965,
                            9083220575923864558,
                            4655688152754256170,
                            1157663155294124729,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16726112399653613676,
                            17608677981924820831,
                            9403451788932647702,
                            17097169495393523745,
                            13916955309651939508,
                            229929295176615425,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1500654658363091166,
                            2462418948768557014,
                            2522355064324754832,
                            16948450337446527841,
                            9627814987936906615,
                            640898026263877898,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            247391394247950715,
                            18141432130890377810,
                            6517003376191310645,
                            17762250562540061071,
                            108722657843843712,
                            1445540051939526682,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5157899701727204085,
                            819875820322144099,
                            6268223895361764007,
                            17790643446513710876,
                            4856090573206057831,
                            120037224608867349,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6673596548810686211,
                            15652467327699337602,
                            16074380168166206423,
                            943423852215763048,
                            2206713156429161863,
                            1799845403716403460,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13708345154112971142,
                            16599240941408280833,
                            7472377757881781340,
                            7041185008155302587,
                            1149387447893298464,
                            199231073305616386,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10638864930094125391,
                            2964393293098342998,
                            16051437604211300379,
                            1823878552870796014,
                            12693670859951961957,
                            1323093107826232289,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12502883135788010058,
                            18231280322420514288,
                            13609500745263941821,
                            11262232747352697250,
                            2050276133421719704,
                            1773868523542181495,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6582546930739979992,
                            18064843125404450922,
                            16879886621970926628,
                            8304208433710264221,
                            8793142427366929350,
                            534269152404257089,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5381709330803505803,
                            13511745777493235733,
                            12518695074374605081,
                            3906594895289128788,
                            11980367190388171894,
                            87064433719931293,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5818419501413550715,
                            15350176950142879874,
                            7564850867650072943,
                            3837768924741808582,
                            16236069890090141234,
                            394603172328733518,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7281154305111416769,
                            4487193904832681241,
                            10482057769478292600,
                            14428836532944178231,
                            3818847610768323225,
                            1227705540953012991,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14469093430695529441,
                            14177409397091967112,
                            10737765202579359475,
                            5700512764417476765,
                            13705279009160092147,
                            48010097963797291,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18431134707601693210,
                            7943687374407101956,
                            15518754531551990431,
                            15393129583099572613,
                            1737533709702992939,
                            1064475304046392789,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13729905244641979546,
                            5257781694625282910,
                            6029432367105817497,
                            12303288450971948133,
                            15204568792935025486,
                            45481162493934815,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1079535808577504745,
                            3742438347222741760,
                            10687136529202576920,
                            10540734518387687769,
                            13332329698881989601,
                            1627108616201631915,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10072627604046774772,
                            11207552325089573686,
                            586741396402630293,
                            14934936009771664872,
                            15310008035487557227,
                            474720856878311591,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6678308864680023051,
                            7763453032544463201,
                            8128017577434783976,
                            17543744881486595866,
                            13844081870689233043,
                            687122574369410419,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15708340494683514223,
                            12914495024836956765,
                            6268353057240977462,
                            18431470635286417424,
                            13809535140037846890,
                            862571744357689151,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13511797885789807453,
                            7518003312699334915,
                            3162338503116824862,
                            6241710452729169149,
                            7094359365544231716,
                            1708378573776828963,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17449622493173814478,
                            15762920789993705289,
                            7026913244474081236,
                            349265112885782541,
                            4262810789085200878,
                            967636617801196363,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16070710155619574344,
                            2312512311948067641,
                            9500122922609539086,
                            14766935065840947946,
                            2865598743273119825,
                            1141847869534817919,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6344135760330774299,
                            20237546976340208,
                            7967698563967042030,
                            3564284165046232581,
                            11541660314872288090,
                            629427937028409768,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8345858857333226225,
                            4056862673494730895,
                            17652874393747873290,
                            1371276796011047660,
                            12325247121421736885,
                            1697494236265527758,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16058133463755938053,
                            15162109911118906467,
                            16875194522327383112,
                            10810057582694061779,
                            11120818845384684813,
                            924321402668763083,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1854362641616322716,
                            8408015594449886194,
                            3230316741017159264,
                            11824743867488752532,
                            8150634807842722352,
                            876312913867229724,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            197223997876536312,
                            16785199390686343123,
                            12260844578667807714,
                            13803799107428906903,
                            1555835140070862158,
                            767588343461276013,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14114696892650114167,
                            3226855278551472181,
                            11024518779862788351,
                            6440252271396310950,
                            13484813552917308041,
                            1254150784261742111,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6915198207777947371,
                            15825608317379830921,
                            10401290094720000760,
                            6874370291552641933,
                            1214404880067585053,
                            1742676671247371751,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            554681470704440006,
                            6305619944227277233,
                            3386900348683362144,
                            1949354595813508691,
                            6033074702430730414,
                            1698422090901230114,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15338222914148316010,
                            12377379407403092210,
                            18136960348538793649,
                            10043720685740566309,
                            1734708583794832641,
                            1146442558765030126,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3933736645647854909,
                            1944746901392429781,
                            1751059834539120114,
                            16421765145861123633,
                            2265600422991001998,
                            494344181283028521,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5118962626562096639,
                            2144270977124034316,
                            12024023598645748383,
                            16998128793992990438,
                            10999046712007634701,
                            1361754309230103645,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6267223636222920017,
                            8526101310420643506,
                            15459576729558696335,
                            436566652045809141,
                            2429938127522542926,
                            435970476903457622,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13685266480877902990,
                            15039697852170739250,
                            5999162652258661846,
                            6969890845429593175,
                            18017883970456866087,
                            1128093971018768292,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5616649208306484642,
                            13767263204138992060,
                            16484465342725903299,
                            12219549762887523625,
                            1625396102194408524,
                            270121101945181972,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18289040732904181887,
                            15787149131149471230,
                            14412806396884708696,
                            8449362377510943851,
                            1870032235507410041,
                            204814764962193692,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            280133190698556498,
                            3778818680279306934,
                            2045929580536952593,
                            4235331680543425849,
                            825164297310450770,
                            1847679367551055015,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12887714432874279068,
                            1993020441992937319,
                            16615123206253789332,
                            7994904475784341479,
                            5550825689012316165,
                            1379114606266609437,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16121913258908632883,
                            4614505332686681323,
                            17127857512475675520,
                            10135365154313638546,
                            17301246298875399676,
                            1245756524714845376,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13138865495269074727,
                            646099245207820161,
                            9912924189901660391,
                            12110822414326166570,
                            15515634970411191436,
                            1576043940298855074,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1869087875635714226,
                            12625678309312697025,
                            12615537621252666780,
                            4739326373587847475,
                            6123844338746918679,
                            1045711955779092085,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            882457720514772761,
                            10973943279125443435,
                            18063061459754070027,
                            10180593414612779714,
                            9830274087881604259,
                            583993293232020218,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5368352291482319937,
                            4996336871969496195,
                            2953096495367400804,
                            12329385541545832713,
                            14002200891566843591,
                            1389216765602956032,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14946012133818206715,
                            10141259852011384551,
                            33278170267745070,
                            3984795230858631798,
                            4027569870748205112,
                            587045876864920097,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17495715475323225013,
                            7670640758535971419,
                            7558014606165167629,
                            6139318875120393903,
                            6312105094319666476,
                            1776297276129350489,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8134551852788465503,
                            1689607357525963663,
                            8689043304186027430,
                            8969311778930272337,
                            12540459095085462107,
                            1014605753358060154,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4502095076386086548,
                            5050834343927355777,
                            4016625433877877139,
                            5563823549816048622,
                            6057653528621705281,
                            1867082322999344654,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5233686668589511042,
                            17141650195475450845,
                            166992622877609082,
                            15381605692146435359,
                            10095418594825252115,
                            1872305201853021815,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5715186471318322136,
                            8698492431710181663,
                            12370452617135377150,
                            2719476964473939638,
                            11981857684015211437,
                            617419608386868309,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1181634076498650045,
                            12893660024444163034,
                            3099190990095674577,
                            12979830602341511159,
                            14062503947707491385,
                            1011999948583898006,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2450070326687781037,
                            12982070751772574408,
                            1040643936794894602,
                            13146910064273945743,
                            9731588068563082863,
                            442570562915431899,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7425340590144792587,
                            172125985796190288,
                            15302516775559034981,
                            8374743388915249370,
                            10816207337592423964,
                            448611880336218988,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17309354681187641219,
                            17267001949163141710,
                            8640344944865267622,
                            17587614646529357222,
                            10574819041696652071,
                            787653493874810889,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10811892912936886699,
                            15356637348539678856,
                            5000545707805493727,
                            408553627986629669,
                            13648249379410911295,
                            869202766936012881,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4917153853665937694,
                            4591895061358218733,
                            2568874911293566849,
                            12364868639553041450,
                            16680842968024819102,
                            1749589204326980387,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2532926491184531262,
                            7660483422673682211,
                            16061217831913460207,
                            2068491925341409417,
                            13051244506870289604,
                            496576386106130939,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3472931335207165603,
                            16122098576363886020,
                            11909443469582056783,
                            14896728153095846109,
                            599116321172746805,
                            1091781388399734948,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            810552905219561937,
                            10610909402484210023,
                            3175059121281288667,
                            784191003879144897,
                            5320031841822975611,
                            1128532660309980813,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8090659203129451420,
                            5976755188618955052,
                            15883870818637195540,
                            14723522156689350004,
                            15897754911187980226,
                            465399857996247438,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1544337406725197852,
                            14659322697913457535,
                            9499355068095146482,
                            12838139958102278049,
                            10146207690633130990,
                            1586750250440934206,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4644888960680794565,
                            17435237999669924691,
                            13521018777083760103,
                            5761662352700362266,
                            14366743612928538283,
                            1214417005904731601,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9083676387730382050,
                            2862079173246843741,
                            11146235748606059077,
                            1963755046967401694,
                            12470009976570297794,
                            739726183813915258,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10326154043874153551,
                            11080934282307286683,
                            16614876120027577392,
                            11704097468561994654,
                            10710658641109872492,
                            176955610690441238,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12950157156629591466,
                            8660480216421198185,
                            2389621936687223798,
                            17369961952849224403,
                            17182729343216810128,
                            723302451967307399,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14654703356625934821,
                            10191519416613522847,
                            13248670481249646168,
                            12036411318356251275,
                            17036000999030271523,
                            1485671754685594446,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14558430125654880570,
                            16692834336527054842,
                            563619496781494127,
                            4827752127228013615,
                            1970133768231523606,
                            1181032790779799866,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13987404521987220051,
                            14023631837876838276,
                            14103300333705511159,
                            11986241301751934871,
                            14228400315885434953,
                            946358201980947736,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3399596786106383105,
                            1924944913898959053,
                            12173427276603060706,
                            5631059286124338214,
                            8425369771105643759,
                            996022080297139170,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1104222252884166189,
                            12655961089316906303,
                            16120603810325708425,
                            7576028799062140842,
                            8952396546508063030,
                            1304147764857767913,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8878921254315022052,
                            10132593357834713278,
                            5537418493258692541,
                            2410537735181312771,
                            1949335088203829731,
                            1203694364701300098,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5239905263861076123,
                            3262109898944242083,
                            14839072735826911675,
                            8466121870778428727,
                            11853974005987303107,
                            1063557961927081436,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16377504179782226581,
                            7506134599857010314,
                            10527855756935590536,
                            1378021226598719761,
                            16486038852734948486,
                            1555686612739665487,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5679708117390258412,
                            5688874111927024452,
                            6229501604523844561,
                            1750032931398730793,
                            14181329066398998493,
                            379977079285420356,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15662399217801968609,
                            6048035434448639239,
                            5630179038397128660,
                            11117163336615802509,
                            12302032663427302883,
                            1477946072907754827,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15389850345759258636,
                            16291578245368095955,
                            4294249060545283616,
                            8597665521542404519,
                            7313901830431988176,
                            1215682245601995143,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6783147800757807260,
                            15162148364859997143,
                            16804990876003077337,
                            5441803344932848224,
                            16345171841109060813,
                            202939260088691293,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5116237634567322794,
                            10872153309813384093,
                            9953534471995415091,
                            13284095475912227440,
                            2332382355572116571,
                            1053802884726596155,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17337143158450453261,
                            3199543596041476861,
                            14553693077906423482,
                            13806859823723296226,
                            11221934611867095322,
                            1136177245128119247,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10508604019832928464,
                            6462950433505833930,
                            11605023749576199104,
                            4667511931768510515,
                            11442959725857316700,
                            1245612020733295128,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16687496692825708414,
                            12947099845203553678,
                            4261864738020385556,
                            14757320465704151934,
                            13991391596297129432,
                            952323342034675720,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17619974543261149967,
                            4647545239967497899,
                            3049501863101711115,
                            16190933827568666077,
                            11377457396538403945,
                            477025103779147991,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2174601967961813440,
                            14003100917396634791,
                            10656961743598294368,
                            17896406508490072061,
                            9587941194287696360,
                            517632557552719026,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17821979251453980652,
                            18028442356806015602,
                            7747368644056824628,
                            9794935605082714011,
                            2339436192992715622,
                            1117816833841480708,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4033025756444186313,
                            18319079473076136551,
                            16392474719170793146,
                            17114807426477747565,
                            18041319656189892305,
                            252677953690973544,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1553304359603834276,
                            13265792566329450641,
                            16784701175056530299,
                            16488177676744129527,
                            8125213623975158405,
                            62007143183162309,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10289955547454482020,
                            9640838143798789489,
                            2999980315192642898,
                            7466946737569225331,
                            14839252287731116519,
                            1865180699185853661,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1706182033915909324,
                            4526674676922525067,
                            2839506378847713791,
                            1456928182645658316,
                            964257771392168070,
                            755546041724734435,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10894401738371107919,
                            3389903128073247541,
                            3725080878592153627,
                            5605618961428656938,
                            10787725952537216109,
                            1204778814229792763,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13306850128689404419,
                            14038949299257470399,
                            1989297041729415843,
                            3907666303327386774,
                            5702132837718461210,
                            1814614746844359257,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9706157989658202109,
                            6707904054533140422,
                            13620250788403079950,
                            3465688604745861615,
                            14961248428341176214,
                            1388946301442846477,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17799992878067095741,
                            573376315489957785,
                            1091970360817600977,
                            13968644181338953999,
                            7107568145266615816,
                            646632040378645023,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11169800552109351904,
                            7902171312406441467,
                            15372191805514302398,
                            4832506482253619462,
                            4931274726583789383,
                            93165863272683935,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14886573267805118598,
                            16638086051003208400,
                            12436853001115248191,
                            17568688696004970844,
                            17332495567050433522,
                            919970811992722053,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12716895416657346462,
                            10202131567267535830,
                            15724148786081578380,
                            399008555079259475,
                            426868679925159448,
                            1084371237692392493,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11058696917143630454,
                            3545171992888911575,
                            17135493043601304990,
                            17989230716983636447,
                            13840317626270824332,
                            263656297276193444,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9541473205655028866,
                            9896531692254145687,
                            1411096701430892324,
                            9762187333682248095,
                            1301790740859366654,
                            222370890005455295,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3102569695375517393,
                            2052738540110078042,
                            10444243642207034307,
                            6717290724466642155,
                            6227630021686407047,
                            538400932182638181,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4191270085385413910,
                            15227991169724101538,
                            445319281759940909,
                            3082807275698717015,
                            2717095455568957383,
                            512739157931507462,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5770967957937789875,
                            3961254766652045551,
                            16673602170065318409,
                            3607174403950524724,
                            2944473481537916196,
                            1067513558373527747,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5740032107131620152,
                            12893632206393966657,
                            6555070753610957236,
                            17959204762465581230,
                            6943612988404811823,
                            595560117979171235,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14299570474059847609,
                            7028323523143072122,
                            4668712100262978993,
                            1450111767940784733,
                            12979538070303962697,
                            1365570336812229287,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            831206279506977557,
                            13283680363190128402,
                            10377698720536998134,
                            10895635525449840219,
                            8309302110022056362,
                            1667845892873955178,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13975843247890773069,
                            18420282516668455299,
                            17399087878175449128,
                            9456184238449123000,
                            1497237983971240185,
                            1507155913531008364,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7443037524311178675,
                            3417331767331389580,
                            16927302365871702503,
                            6691250376723194521,
                            17741303966924258772,
                            1480426277012005327,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16770322242110146555,
                            11632430720779233335,
                            11038627769468000493,
                            2788169992228190127,
                            2226323320356754405,
                            722080852325323407,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1298998767233959620,
                            5290514810660762207,
                            4482775780116289434,
                            17228412927969163652,
                            5343077702405620895,
                            1473456344756120122,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1720835570524134814,
                            1745291019167735645,
                            4812783616205872702,
                            5524010722576429939,
                            10877960432186337810,
                            893779911068691026,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5866052511707159926,
                            13143728962510655465,
                            11472892078554024966,
                            13373311148430626284,
                            13363229430213086210,
                            647348237169516269,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4587724446897359466,
                            11380925469068653913,
                            7004170327038285373,
                            11782458483943300474,
                            1440862155898144256,
                            732648785928873071,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6171139749188624017,
                            15933502218596984660,
                            17692806511867579730,
                            1986753439343846307,
                            6377649773624493666,
                            864606161201179890,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11571788620836429328,
                            5633333826043179160,
                            10197772716288306808,
                            3875963196393589918,
                            12906385054848034456,
                            1432688216628257875,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12018641939311707406,
                            11764514397104875855,
                            14113939205071251815,
                            7629166027912223027,
                            469301746869792709,
                            1140631647919709988,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9213697485405249216,
                            8497135089351256435,
                            4401742128694891510,
                            247724360260801060,
                            6035234067123914157,
                            1083145856442677458,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18343475410444084725,
                            3408189879444328569,
                            13904044356360549491,
                            2035653532663781390,
                            2092316513159246651,
                            1720732503034824771,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            740750857330866674,
                            17290951286410561239,
                            7071045218522342113,
                            15226697893952202100,
                            7135034157406061180,
                            733043290060352831,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10090623957154040529,
                            5918519551393387902,
                            17497579547334651404,
                            12258249521410399805,
                            4542857732441116426,
                            844924392058993874,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13632662897960906186,
                            1601000296744622829,
                            4759799678484325711,
                            13763264553513831268,
                            16755187756631113771,
                            869034956005664530,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7323699265585338973,
                            3149312032235540148,
                            1747832248554333563,
                            3797107510894665585,
                            5705206586587868786,
                            750752865121091920,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15308728479039927346,
                            16490612234816450045,
                            15469686885053974309,
                            11805176566995366503,
                            1733492213710700629,
                            155194530600073965,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10732017327434726472,
                            18043780640350860686,
                            9765760222555480125,
                            11687047891105431744,
                            11161465730115202014,
                            1100651396542961404,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6337198146553155383,
                            9823025301002940388,
                            13537838126345124885,
                            285915648223734870,
                            6345676952136221137,
                            1709362093775295491,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16013725053069331564,
                            5229444761181339937,
                            10618127893806684797,
                            16174041854815994449,
                            18420828702542974073,
                            1310581757728597791,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1964699428304435358,
                            8939189509815834426,
                            3687778101678552475,
                            9051869195720646465,
                            433558938563592886,
                            721574528727003937,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8530926130700362544,
                            3828735059469979179,
                            12696364009149009752,
                            17795807224115963188,
                            1916560671183174501,
                            1052422011532865093,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14600147041566190559,
                            1727801246691269525,
                            6418444356305644183,
                            9353333811852003335,
                            6602066764657309853,
                            1317070028800716386,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12668660771442238403,
                            14454685551299431673,
                            12887173444079648569,
                            6803638423983720031,
                            621181536059627664,
                            123720793055541207,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2245004301357318803,
                            628532038599776835,
                            1037607080275848321,
                            4866779744497884033,
                            4047526254897212064,
                            1283131087529221391,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7794117941903613125,
                            2856640422536438237,
                            14848709550635340903,
                            9061711814102960782,
                            4708255016213260906,
                            216771163196879064,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13582964151230546745,
                            7189978924007035577,
                            5626326822630395237,
                            18280041587218873047,
                            305116474537157391,
                            1235541495814575566,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            360080346312154703,
                            10400880199350719394,
                            8891861254432451335,
                            2411209052337030312,
                            5871358203719919078,
                            1579757400811091711,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13772336095976337101,
                            2127501152214695497,
                            7675711191832635368,
                            8300379861736336063,
                            4310323794911974821,
                            35417139539433487,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4149641287768158033,
                            1644311270109681311,
                            7814007047413733613,
                            7055185217804921787,
                            10365746197993177764,
                            720211091670546894,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16011644299296884335,
                            17927357162931537064,
                            8551178068722706985,
                            6217737474676164861,
                            18249438757375621620,
                            1717870663231326547,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11600407766240395990,
                            10313992171230867853,
                            15267440847320484124,
                            8204580716784240906,
                            10335635778078763849,
                            1520847446055928994,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7195545655994186810,
                            6842147377752942677,
                            3536552057407761029,
                            16391033963145722585,
                            12802868576512704913,
                            1706025662522519427,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13347352306021642258,
                            10091062368568946266,
                            7136649578245068705,
                            15533726729381838623,
                            12005660840000554360,
                            1093529527686128005,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            390934607356732771,
                            10570200995386327898,
                            4607521007168062067,
                            11281332755743190684,
                            14183864238774593874,
                            890796023447492796,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12001788621263817051,
                            2675213454189652909,
                            9149661583835306584,
                            12408362141593423369,
                            8168663480116986529,
                            1505809389160516982,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10596501860955157998,
                            16537212101483787849,
                            13432555603114839051,
                            438813360296836813,
                            3709736952462363398,
                            1464824949626930683,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17552736113305781740,
                            15139708009560240832,
                            9824860443697786671,
                            6189543745044045081,
                            14557282622454709317,
                            1704897475937031640,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15194804300979606240,
                            14325578047290219075,
                            3956536252503852026,
                            16274775829583941256,
                            16993503073645129141,
                            216281155696429496,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12885358115006280680,
                            17350687892069611945,
                            11618461086314966492,
                            8970582011305599995,
                            13337183657339934879,
                            410761622130636357,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1236170347900458680,
                            10906772370928760757,
                            7618538440465017050,
                            11620346463806869321,
                            10896759169515394822,
                            1826909264385187955,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17686441083579255066,
                            12804186928589389919,
                            15864506579779477610,
                            17593751718362321389,
                            5529312760196023077,
                            471709952370801233,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13943163830587569377,
                            16785603236956910351,
                            15602815568012899905,
                            7552401409867314720,
                            1709433543086004397,
                            1627927926884534907,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9723531880103370888,
                            12967812698624705362,
                            9828822199972925561,
                            4046209601369077574,
                            2019593694756205364,
                            566437751600534029,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14725386758468925050,
                            3696968045287533666,
                            12520379808188130552,
                            7413368235903588394,
                            17179012390887330135,
                            917222220028200879,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9713035033791998458,
                            5668154360165243021,
                            10371987663735658021,
                            5769205667821534470,
                            8770710935730446003,
                            1467926411031949529,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16487105651632780600,
                            9729613534130450251,
                            5620813301494517028,
                            866227240634634154,
                            2916952374402250584,
                            1149960554498513683,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2366420970982188077,
                            17456076891084890758,
                            13778010790572053783,
                            8098857293575462025,
                            10716287581268090622,
                            389074389206961401,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17770673664316133010,
                            3594695075798046841,
                            10544801156282601238,
                            1775803355312734391,
                            6816934877486510384,
                            1638837635145417736,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14365438208449804129,
                            9855436038928458854,
                            8930004768911952417,
                            6588401514512916719,
                            18361097911461746696,
                            860727157252820021,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1269011045836919445,
                            9146575631699911421,
                            10608632424632590051,
                            1609320120008576913,
                            9085321674175103363,
                            357774541908665949,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7449219352680216943,
                            10383754896013123130,
                            13474564988721560401,
                            2178813769222568669,
                            13351514473718936308,
                            298706140821169170,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4425607281990497706,
                            12656287974383901770,
                            13849145564838880101,
                            16276108436526678179,
                            15319629416542465326,
                            136261654200738648,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18226641774049765760,
                            12346914742892917379,
                            4337620818074129202,
                            5176720764979717407,
                            1213401781652967084,
                            1361732581275641293,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5670784665769644409,
                            15388742199934761100,
                            8866831872991789809,
                            17196747838808197682,
                            8953423499142292712,
                            287408653766993981,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11371502373839199084,
                            7478706532295036925,
                            16501941539854242824,
                            14430851975329061060,
                            8762837357511141239,
                            700487374362670167,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17559118789399313420,
                            2276060953176040206,
                            8660115808144626924,
                            9416709014650533381,
                            12646907116364725990,
                            388251565012048217,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13074170043333871673,
                            7906883866062170353,
                            8813558480605921608,
                            13382947043739495937,
                            1037221952745221687,
                            1550804670085448390,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13015019525385996225,
                            5017260093973921914,
                            5679095462684575129,
                            1912781607085127184,
                            17912004075220891451,
                            755294764888800885,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8206910184382595575,
                            2449899488828457728,
                            11480296278975931077,
                            7083967460372814178,
                            13661536091773423416,
                            243676108830223347,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9124408032208404087,
                            16782047288035517331,
                            7083973178252296443,
                            2848822970103864663,
                            13517577417110915535,
                            1566686020515451066,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12777866874750969696,
                            6468493148848427722,
                            5237708949112526386,
                            6388349065023771768,
                            8393439894031623453,
                            721537119858626113,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2817627161538419071,
                            12631318860517558773,
                            17256707092034217667,
                            16544713831817882994,
                            10651702336891664947,
                            489930105649242450,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16450633171373654955,
                            11256413568056724125,
                            6013308701914064101,
                            13043621660238502920,
                            8278000531773111247,
                            1259016837333293999,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9636521360516157338,
                            14333871646860033548,
                            10746088978681814022,
                            9562232829090690277,
                            18434673607435680215,
                            1083093717453557552,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2753101063760769765,
                            534572029922189270,
                            14502932421676269419,
                            11822013956145212401,
                            6809473362153231906,
                            1119683640393865749,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            494560408213251629,
                            9931680604122969487,
                            8113262704019298644,
                            13897304758379397084,
                            16850180100477898196,
                            551670905511323284,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12092805204437300472,
                            5628123318859892181,
                            2958706208661759415,
                            13042788039249827219,
                            376428661580651445,
                            1673108164326024668,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11909068217808116255,
                            306255019290189317,
                            15988046665314334581,
                            8201711198029451047,
                            16969831353422249298,
                            1551997886933497465,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15600239978343565736,
                            4872547909284541518,
                            12818323168598072174,
                            7396623617427083699,
                            2570587016561729331,
                            554605634509636673,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8288598135674744138,
                            8728940646733991022,
                            11488070318822750914,
                            18225911695000067917,
                            17051242642254324327,
                            516673391970788043,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17625432568878298175,
                            16015515382830824272,
                            5112407783557734854,
                            5227378299617674814,
                            4170359583142399679,
                            478606780058589271,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1407222416826324392,
                            10158059606824348095,
                            7818044397847761824,
                            10517370192783656566,
                            18267373921571219303,
                            314760494423180358,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6035184501044068251,
                            12362139909554619545,
                            16173712496977684912,
                            8565212904749077604,
                            18282603375954080624,
                            53732736653086565,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13899620657622235939,
                            3646325537770532591,
                            7626056264211646629,
                            16527940030224389136,
                            16982778010583068091,
                            1052200409189742530,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3428061368705802179,
                            9172537495071915890,
                            13179176371284966449,
                            700751609655750664,
                            12690493996632458263,
                            52961215894976144,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9509913533673091268,
                            1975236828540636198,
                            16594874537000038965,
                            13576982874929068817,
                            10725185656497286950,
                            1238021259974286449,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12670536278745260827,
                            11199559527741711624,
                            1731347984091322519,
                            4985587212271287400,
                            16934547186916918627,
                            91796749095206282,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14574661712573087220,
                            6097049098691705262,
                            17877141646844674468,
                            15507493212424683885,
                            10657666408361467983,
                            1307076149042914396,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2388325403923589929,
                            12380567283038434604,
                            604905932484493791,
                            2658975434838153423,
                            17192494081819527871,
                            1168041427264484640,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11842194058626225407,
                            6693138488663942165,
                            9693479840472939808,
                            6557766515945332492,
                            17527675102839235765,
                            1683781909006149565,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6732548018962826686,
                            886556176451583387,
                            1074481832211224653,
                            1672131685357546020,
                            13441358290175210535,
                            982329272818999435,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10391688221486231074,
                            9045549196032969733,
                            9444276653435874426,
                            6225312622280940322,
                            541558567715002424,
                            745968211499641484,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12256806308523389866,
                            17531109129597277602,
                            15744046266434284801,
                            6358392850927869028,
                            8469414460933919507,
                            991351303461708033,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9883294061007381841,
                            2250779174019482262,
                            10628018332261470086,
                            5917624016444952832,
                            2049519592332484588,
                            379266076439362854,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9613078049267231442,
                            498002071805226876,
                            8894979186094872042,
                            3998139538429512555,
                            5678599008406874796,
                            1590878292733900636,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4950488905387863282,
                            9262554117179726455,
                            4921783018946894254,
                            17490603013569449815,
                            10209295587593749530,
                            1372763087957478961,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10788537566316853227,
                            6995845539765443609,
                            104664623115210233,
                            15907011503294258444,
                            8248858225433775710,
                            725566341458236355,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16944071049938325933,
                            4749728484611508220,
                            3405682516230518655,
                            6820475010781060602,
                            4269119022583474318,
                            1674821274156141564,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12871477738709567183,
                            18114200996132079755,
                            3592273059302378510,
                            4274385300990581274,
                            15706293305947464558,
                            501709999276534525,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12199343707705719479,
                            2067874046362595679,
                            11343781441327819959,
                            13168117941744802399,
                            18433469638266699705,
                            1106934153178093081,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2016184029140046031,
                            4745925294113021416,
                            14995511775427280633,
                            6576761812152300568,
                            12445804865351333693,
                            293585514246953069,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8240525097910977200,
                            3259925884454885677,
                            17004190596641004861,
                            6619601534433055667,
                            1303374522468985550,
                            374110802427833040,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13123795113832111202,
                            10903337672960158845,
                            6752723252270236171,
                            15385624631052190500,
                            17180877953006621744,
                            65287946125841389,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15009197299648262080,
                            15152532429797765178,
                            2815893965498385907,
                            15067959306656178812,
                            5232020304559539739,
                            785753815524395898,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17095723910618273309,
                            2744427301118095065,
                            14985660426165692345,
                            388267362491534868,
                            1951228479964912270,
                            1853839396735533641,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7835975411447361234,
                            15330174279654532250,
                            16579546370766982770,
                            4622870926255475903,
                            3073957253741719192,
                            1112641646961527620,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13860100220934755379,
                            13520324235152712038,
                            11883708575721294398,
                            9991568485814358385,
                            677725614634631872,
                            1126118962078021306,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11499776424769742665,
                            16585142277311604800,
                            14311893717630360190,
                            12499414477178173879,
                            1793876467309852771,
                            349624445597804114,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5330992631153768577,
                            9989914648078003742,
                            1089841577571042965,
                            8914014067818143551,
                            2909897287550795756,
                            785114038819398806,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5980218324405144602,
                            13199639146447309469,
                            3249734338140408388,
                            8521519871948674408,
                            16869863831438094466,
                            311451473261492764,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14193854149569035691,
                            15592218319700985734,
                            1118172703185492076,
                            13962837710719904303,
                            3481513995647488017,
                            5555461062698585,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17140359368830257541,
                            12219092160540932474,
                            10175966493170977471,
                            9319671628081490335,
                            3226672351599190705,
                            1535418975942490705,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2633683222881274269,
                            9888177501527202341,
                            2976223767762395931,
                            5295657603404850028,
                            16810526736426584311,
                            1405306708564948191,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7179403267861074296,
                            1156194225048593750,
                            8828349436066461292,
                            4987677177394640899,
                            2049695865359047480,
                            826989428879384857,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6926115536228234606,
                            14910577069091588075,
                            14981131966152447847,
                            275474284321153493,
                            15781296685970183236,
                            1789796523757160037,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12692276720975281900,
                            6133847828666720601,
                            12430285534558099330,
                            13984936875185584626,
                            13683051119454372201,
                            301048052841276500,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2263556948701160789,
                            6115207373767876401,
                            7302222749673621563,
                            12890416865650336760,
                            8038753364971042704,
                            586577839001341330,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3076375082100382272,
                            18381862358333263000,
                            3704717733053235629,
                            9794863899981739344,
                            1780017009724646864,
                            1307789440947569153,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10004152265876561984,
                            1458889045563305829,
                            14977673244935805061,
                            5079233733829455623,
                            11760788465618343111,
                            1325782974090110449,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15005162335548602425,
                            4075140056952092001,
                            890651084991501470,
                            18418375515674345474,
                            7673090901082564732,
                            174811740617162466,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            17005189149998607779,
                            2992805595006172062,
                            2376683054094833195,
                            4382688220844843590,
                            12186305317459798164,
                            1413426213444408571,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10504002673697582283,
                            9875146671074071328,
                            15206067664747058826,
                            17109141567575492682,
                            17592708120717401189,
                            589301336782756043,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7023210589813028872,
                            13191765606136660679,
                            11885709030959691635,
                            2586761658864977902,
                            6660360225713727182,
                            1203341243882119415,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13962806274771030849,
                            10645453697420098490,
                            16620666410056734066,
                            317993812922399689,
                            2740614137658037610,
                            1275878344426542241,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3800238653848732661,
                            15788066054738852840,
                            8681815587758963923,
                            12772471614953776278,
                            12617513865878340215,
                            400462204023618241,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12422688686876592562,
                            444617691100750151,
                            2560675910545817134,
                            13825073708261832206,
                            145023412912123543,
                            687751397148637545,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12385340442256480147,
                            18402759026464935950,
                            14410097002459109443,
                            13359793119432902303,
                            7729208089221831500,
                            421244296035777510,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            8976533507926277539,
                            5076835256084069957,
                            10199499832690286313,
                            12619024391917016216,
                            6688284471354212875,
                            1565897490196708033,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16944141547361876579,
                            17844652383138083124,
                            4590478891019257237,
                            6476374925301309128,
                            2314529645085556395,
                            1348927820634119780,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            4848188196378943624,
                            11641365785961733237,
                            3012098003207781304,
                            9220596303511391824,
                            3631445523137966486,
                            76993409067627931,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6207417271162366008,
                            4452663769493026490,
                            1611668112678898927,
                            8446145569155801737,
                            18190735618726711437,
                            1276869507744283329,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12778255112431922,
                            16176084500405599767,
                            2868643516108470289,
                            11443980812138394645,
                            2753026671852978348,
                            1351709508346272822,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            16851470738792520902,
                            12992578238356814599,
                            16820212661547977398,
                            10598900572360762276,
                            6117248566313151185,
                            1505001760047549398,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14493751740189060944,
                            15916689961859483202,
                            5592924189713318460,
                            12586159018748688366,
                            2888613619452935993,
                            1447254563222165336,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15431866192908345943,
                            1372783172961771817,
                            6674572231512474008,
                            9308213171561522250,
                            15427072822580302156,
                            1279930637327974235,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            3398908414373313910,
                            6216117207260679482,
                            11313592760390994038,
                            9661978777319070626,
                            5801425888080213318,
                            1089835299089482360,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1905119043038952242,
                            2897754190327815208,
                            1049929628414664782,
                            15031372479766987477,
                            17663711878911480482,
                            742585923929433618,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5989214689653167843,
                            12145031826513037828,
                            5321535885330764260,
                            5943661315612235342,
                            13080598233627902454,
                            1540434271538229804,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            122129220909060707,
                            5571769125170612177,
                            11547924214523723819,
                            11470421657066671811,
                            3640605496514731554,
                            867431070789556968,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            6872209918229270356,
                            14737383618417157836,
                            8904576422172451121,
                            1644798916007833818,
                            14303734679146464716,
                            1097516396118306872,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2644820217271059863,
                            8485819857955007523,
                            4025886232340677781,
                            7961205313348692225,
                            11621983444639442638,
                            986424711828919247,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            13757761388845863018,
                            3796983939469684253,
                            12090162670358861055,
                            5284444560707444163,
                            13872707122716148809,
                            1213998116539541653,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            2513980284374294945,
                            11059070606052829771,
                            1277658812578202074,
                            259774801948244512,
                            3367663047947356945,
                            1152141701221422368,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12243994789027294240,
                            14073019371948447402,
                            3882936601498092061,
                            16776529504017295476,
                            8027522066371639486,
                            690918294383465536,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            5418489366577403231,
                            3863060464893788931,
                            7263597997238078770,
                            11091099530908853018,
                            8021382689732109067,
                            2088942255581629,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            14786113624313481203,
                            17553901309180685759,
                            12718762238529784308,
                            3362507727287492114,
                            11919539286756310241,
                            310840627542802285,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            10978795069833424023,
                            15116409608641240593,
                            17114242115747884033,
                            11145374302586162971,
                            4050008846143743358,
                            948228268018750093,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            1328510643944488801,
                            3450517460629097645,
                            11202375602229541881,
                            5995800240058980143,
                            2654760623864469605,
                            1501856129732976449,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            11477544067061118285,
                            9340772296732937287,
                            9804854662207177080,
                            7230326152282267427,
                            14276192154178624067,
                            1588766550947596725,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
            ),
            (
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            18356635563801031232,
                            11959555258681758188,
                            14980917299752346569,
                            17603029786895496867,
                            3345670279167891227,
                            185718258393371739,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9445284682255788276,
                            2071563192205068850,
                            9799010855753346498,
                            7169307765271253967,
                            15293260213035514256,
                            1459071553085501236,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            7203929278814369378,
                            12875706433761421308,
                            13910112743568305943,
                            5764140797609047320,
                            5096864215259319589,
                            115451314101814237,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            12780125722711637927,
                            13519418790751874478,
                            13810382498987249678,
                            13589538434234429792,
                            18437054612875469721,
                            1474259165360360713,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                },
                crypto::ark_ff::fields::models::Fp2 {
                    c0: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            15289647063488254856,
                            14878021736967188543,
                            9379542578386465184,
                            14864943218776741037,
                            156384284512009681,
                            249471107516534100,
                            0,
                            0,
                        ]),
                        core::marker::PhantomData,
                    ),
                    c1: crypto::ark_ff_delegation::Fp(
                        crypto::BigInt([
                            9237531673089623465,
                            847556548915631356,
                            1076356927959502406,
                            6652421818521841385,
                            3816271507244105114,
                            1115283707760879599,
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
