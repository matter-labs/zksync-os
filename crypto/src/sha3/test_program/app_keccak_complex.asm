[0m[1m[33mwarning[0m[0m[1m: unused variable: `modulus`[0m
[0m   [0m[0m[1m[38;5;12m--> [0m[0m/home/mario/zksync-os/crypto/src/secp256k1/scalars/scalar32.rs:115:47[0m
[0m    [0m[0m[1m[38;5;12m|[0m
[0m[1m[38;5;12m115[0m[0m [0m[0m[1m[38;5;12m|[0m[0m [0m[0m    pub(super) fn eq_mod(&self, other: &Self, modulus: &Self) -> bool {[0m
[0m    [0m[0m[1m[38;5;12m|[0m[0m                                               [0m[0m[1m[33m^^^^^^^[0m[0m [0m[0m[1m[33mhelp: if this is intentional, prefix it with an underscore: `_modulus`[0m
[0m    [0m[0m[1m[38;5;12m|[0m
[0m    [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default[0m

[0m[1m[33mwarning[0m[0m[1m: associated function `from_be_bytes` is never used[0m
[0m  [0m[0m[1m[38;5;12m--> [0m[0m/home/mario/zksync-os/crypto/src/secp256k1/scalars/scalar32.rs:64:19[0m
[0m   [0m[0m[1m[38;5;12m|[0m
[0m[1m[38;5;12m25[0m[0m [0m[0m[1m[38;5;12m|[0m[0m [0m[0mimpl ScalarInner {[0m
[0m   [0m[0m[1m[38;5;12m|[0m[0m [0m[0m[1m[38;5;12m----------------[0m[0m [0m[0m[1m[38;5;12massociated function in this implementation[0m
[0m[1m[38;5;12m...[0m
[0m[1m[38;5;12m64[0m[0m [0m[0m[1m[38;5;12m|[0m[0m [0m[0m    pub(super) fn from_be_bytes(bytes: &[u8; 32]) -> Self {[0m
[0m   [0m[0m[1m[38;5;12m|[0m[0m                   [0m[0m[1m[33m^^^^^^^^^^^^^[0m
[0m   [0m[0m[1m[38;5;12m|[0m
[0m   [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default[0m

[0m[1m[33mwarning[0m[0m[1m: unstable feature specified for `-Ctarget-feature`: `unaligned-scalar-mem`[0m
[0m  [0m[0m[1m[38;5;12m|[0m
[0m  [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: this feature is not stably supported; its behavior can change in the future[0m

[0m[1m[33mwarning[0m[0m[1m: unstable feature specified for `-Ctarget-feature`: `relax`[0m
[0m  [0m[0m[1m[38;5;12m|[0m
[0m  [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: this feature is not stably supported; its behavior can change in the future[0m

[0m[1m[33mwarning[0m[0m[1m: unused variable: `heap_start`[0m
[0m  [0m[0m[1m[38;5;12m--> [0m[0msrc/main.rs:59:9[0m
[0m   [0m[0m[1m[38;5;12m|[0m
[0m[1m[38;5;12m59[0m[0m [0m[0m[1m[38;5;12m|[0m[0m [0m[0m    let heap_start = core::ptr::addr_of_mut!(_sheap);[0m
[0m   [0m[0m[1m[38;5;12m|[0m[0m         [0m[0m[1m[33m^^^^^^^^^^[0m[0m [0m[0m[1m[33mhelp: if this is intentional, prefix it with an underscore: `_heap_start`[0m
[0m   [0m[0m[1m[38;5;12m|[0m
[0m   [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default[0m

[0m[1m[33mwarning[0m[0m[1m: unused variable: `heap_end`[0m
[0m  [0m[0m[1m[38;5;12m--> [0m[0msrc/main.rs:60:9[0m
[0m   [0m[0m[1m[38;5;12m|[0m
[0m[1m[38;5;12m60[0m[0m [0m[0m[1m[38;5;12m|[0m[0m [0m[0m    let heap_end = core::ptr::addr_of_mut!(_eheap);[0m
[0m   [0m[0m[1m[38;5;12m|[0m[0m         [0m[0m[1m[33m^^^^^^^^[0m[0m [0m[0m[1m[33mhelp: if this is intentional, prefix it with an underscore: `_heap_end`[0m

[0m[1m[33mwarning[0m[0m[1m: unstable feature specified for `-Ctarget-feature`: `unaligned-scalar-mem`[0m
[0m  [0m[0m[1m[38;5;12m|[0m
[0m  [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: this feature is not stably supported; its behavior can change in the future[0m

[0m[1m[33mwarning[0m[0m[1m: unstable feature specified for `-Ctarget-feature`: `relax`[0m
[0m  [0m[0m[1m[38;5;12m|[0m
[0m  [0m[0m[1m[38;5;12m= [0m[0m[1mnote[0m[0m: this feature is not stably supported; its behavior can change in the future[0m


test_program:	file format elf32-littleriscv

Disassembly of section .text:

00000000 <_start>:
       0: 00000097     	auipc	ra, 0x0
       4: 00c08093     	addi	ra, ra, 0xc
       8: 00008067     	ret

0000000c <_abs_start>:
       c: 04201197     	auipc	gp, 0x4201
      10: 7f418193     	addi	gp, gp, 0x7f4

00000014 <.Lpcrel_hi2>:
      14: 04200117     	auipc	sp, 0x4200
      18: fec10113     	addi	sp, sp, -0x14
      1c: 00010433     	add	s0, sp, zero
      20: 0040006f     	j	0x24 <_start_rust>

00000024 <_start_rust>:
      24: ff010113     	addi	sp, sp, -0x10
      28: 00112623     	sw	ra, 0xc(sp)
      2c: 00812423     	sw	s0, 0x8(sp)
      30: 01010413     	addi	s0, sp, 0x10
      34: 004000ef     	jal	0x38 <test_program::main::h2f28104569e07ff9>

00000038 <test_program::main::h2f28104569e07ff9>:
      38: ff010113     	addi	sp, sp, -0x10
      3c: 00112623     	sw	ra, 0xc(sp)
      40: 00812423     	sw	s0, 0x8(sp)
      44: 01010413     	addi	s0, sp, 0x10
      48: 004000ef     	jal	0x4c <test_program::workload::h2774dc860186389d>

0000004c <test_program::workload::h2774dc860186389d>:
      4c: ff010113     	addi	sp, sp, -0x10
      50: 00112623     	sw	ra, 0xc(sp)
      54: 00812423     	sw	s0, 0x8(sp)
      58: 01010413     	addi	s0, sp, 0x10
      5c: 04200537     	lui	a0, 0x4200
      60: 00050513     	mv	a0, a0
      64: 04200637     	lui	a2, 0x4200
      68: 53060613     	addi	a2, a2, 0x530
      6c: 40a60633     	sub	a2, a2, a0
      70: 000085b7     	lui	a1, 0x8
      74: 81858593     	addi	a1, a1, -0x7e8
      78: 6f8070ef     	jal	0x7770 <memcpy>
      7c: 14c000ef     	jal	0x1c8 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53>
      80: 04200537     	lui	a0, 0x4200
      84: 02050513     	addi	a0, a0, 0x20
      88: 030000ef     	jal	0xb8 <riscv_common::zksync_os_finish_success::hb5b39dbba9e47e1e>

0000008c <_RNvCs6Gf8pSYpf6Z_7___rustc17rust_begin_unwind>:
      8c: ff010113     	addi	sp, sp, -0x10
      90: 00112623     	sw	ra, 0xc(sp)
      94: 00812423     	sw	s0, 0x8(sp)
      98: 01010413     	addi	s0, sp, 0x10
      9c: 118000ef     	jal	0x1b4 <rust_abort>

000000a0 <riscv_common::zksync_os_finish_error::h8db2dad1f6a026a3>:
      a0: ff010113     	addi	sp, sp, -0x10
      a4: 00112623     	sw	ra, 0xc(sp)
      a8: 00812423     	sw	s0, 0x8(sp)
      ac: 01010413     	addi	s0, sp, 0x10
      b0: c0001073     	unimp
      b4: c0001073     	unimp

000000b8 <riscv_common::zksync_os_finish_success::hb5b39dbba9e47e1e>:
      b8: fb010113     	addi	sp, sp, -0x50
      bc: 04112623     	sw	ra, 0x4c(sp)
      c0: 04812423     	sw	s0, 0x48(sp)
      c4: 05010413     	addi	s0, sp, 0x50
      c8: fe042423     	sw	zero, -0x18(s0)
      cc: fe042623     	sw	zero, -0x14(s0)
      d0: fe042823     	sw	zero, -0x10(s0)
      d4: fe042a23     	sw	zero, -0xc(s0)
      d8: fc042c23     	sw	zero, -0x28(s0)
      dc: fc042e23     	sw	zero, -0x24(s0)
      e0: fe042023     	sw	zero, -0x20(s0)
      e4: fe042223     	sw	zero, -0x1c(s0)
      e8: 00052583     	lw	a1, 0x0(a0)
      ec: 00452603     	lw	a2, 0x4(a0)
      f0: 00852683     	lw	a3, 0x8(a0)
      f4: 00c52703     	lw	a4, 0xc(a0)
      f8: fab42c23     	sw	a1, -0x48(s0)
      fc: fac42e23     	sw	a2, -0x44(s0)
     100: fcd42023     	sw	a3, -0x40(s0)
     104: fce42223     	sw	a4, -0x3c(s0)
     108: 01052583     	lw	a1, 0x10(a0)
     10c: 01452603     	lw	a2, 0x14(a0)
     110: 01852683     	lw	a3, 0x18(a0)
     114: 01c52503     	lw	a0, 0x1c(a0)
     118: fcb42423     	sw	a1, -0x38(s0)
     11c: fcc42623     	sw	a2, -0x34(s0)
     120: fcd42823     	sw	a3, -0x30(s0)
     124: fca42a23     	sw	a0, -0x2c(s0)
     128: fb840513     	addi	a0, s0, -0x48
     12c: 004000ef     	jal	0x130 <riscv_common::zksync_os_finish_success_extended::h341b033224353690>

00000130 <riscv_common::zksync_os_finish_success_extended::h341b033224353690>:
     130: fd010113     	addi	sp, sp, -0x30
     134: 02112623     	sw	ra, 0x2c(sp)
     138: 02812423     	sw	s0, 0x28(sp)
     13c: 03212223     	sw	s2, 0x24(sp)
     140: 03312023     	sw	s3, 0x20(sp)
     144: 01412e23     	sw	s4, 0x1c(sp)
     148: 01512c23     	sw	s5, 0x18(sp)
     14c: 01612a23     	sw	s6, 0x14(sp)
     150: 01712823     	sw	s7, 0x10(sp)
     154: 01812623     	sw	s8, 0xc(sp)
     158: 01912423     	sw	s9, 0x8(sp)
     15c: 01a12223     	sw	s10, 0x4(sp)
     160: 03010413     	addi	s0, sp, 0x30
     164: fca42823     	sw	a0, -0x30(s0)
     168: fd040513     	addi	a0, s0, -0x30
     16c: fd042d03     	lw	s10, -0x30(s0)
     170: 000d2503     	lw	a0, 0x0(s10)
     174: 004d2583     	lw	a1, 0x4(s10)
     178: 008d2603     	lw	a2, 0x8(s10)
     17c: 00cd2683     	lw	a3, 0xc(s10)
     180: 010d2703     	lw	a4, 0x10(s10)
     184: 014d2783     	lw	a5, 0x14(s10)
     188: 018d2803     	lw	a6, 0x18(s10)
     18c: 01cd2883     	lw	a7, 0x1c(s10)
     190: 020d2903     	lw	s2, 0x20(s10)
     194: 024d2983     	lw	s3, 0x24(s10)
     198: 028d2a03     	lw	s4, 0x28(s10)
     19c: 02cd2a83     	lw	s5, 0x2c(s10)
     1a0: 030d2b03     	lw	s6, 0x30(s10)
     1a4: 034d2b83     	lw	s7, 0x34(s10)
     1a8: 038d2c03     	lw	s8, 0x38(s10)
     1ac: 03cd2c83     	lw	s9, 0x3c(s10)
     1b0: 0000006f     	j	0x1b0 <riscv_common::zksync_os_finish_success_extended::h341b033224353690+0x80>

000001b4 <rust_abort>:
     1b4: ff010113     	addi	sp, sp, -0x10
     1b8: 00112623     	sw	ra, 0xc(sp)
     1bc: 00812423     	sw	s0, 0x8(sp)
     1c0: 01010413     	addi	s0, sp, 0x10
     1c4: eddff0ef     	jal	0xa0 <riscv_common::zksync_os_finish_error::h8db2dad1f6a026a3>

000001c8 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53>:
     1c8: 81010113     	addi	sp, sp, -0x7f0
     1cc: 7e112623     	sw	ra, 0x7ec(sp)
     1d0: 7e812423     	sw	s0, 0x7e8(sp)
     1d4: 7e912223     	sw	s1, 0x7e4(sp)
     1d8: 7f212023     	sw	s2, 0x7e0(sp)
     1dc: 7d312e23     	sw	s3, 0x7dc(sp)
     1e0: 7d412c23     	sw	s4, 0x7d8(sp)
     1e4: 7d512a23     	sw	s5, 0x7d4(sp)
     1e8: 7d612823     	sw	s6, 0x7d0(sp)
     1ec: 7d712623     	sw	s7, 0x7cc(sp)
     1f0: 7d812423     	sw	s8, 0x7c8(sp)
     1f4: 7d912223     	sw	s9, 0x7c4(sp)
     1f8: 7da12023     	sw	s10, 0x7c0(sp)
     1fc: 7bb12e23     	sw	s11, 0x7bc(sp)
     200: 7f010413     	addi	s0, sp, 0x7f0
     204: 9f010113     	addi	sp, sp, -0x610
     208: f0017113     	andi	sp, sp, -0x100
     20c: 00100493     	li	s1, 0x1
     210: 20810513     	addi	a0, sp, 0x208
     214: 028060ef     	jal	0x623c <ark_std::rand_helper::test_rng::h09418e87dd50358f>
     218: 7ff10513     	addi	a0, sp, 0x7ff
     21c: 43950513     	addi	a0, a0, 0x439
     220: 0c800613     	li	a2, 0xc8
     224: 00000593     	li	a1, 0x0
     228: 558070ef     	jal	0x7780 <memset>
     22c: 34010513     	addi	a0, sp, 0x340
     230: 0c800613     	li	a2, 0xc8
     234: 00000593     	li	a1, 0x0
     238: 548070ef     	jal	0x7780 <memset>
     23c: 41010513     	addi	a0, sp, 0x410
     240: 08900613     	li	a2, 0x89
     244: 00000593     	li	a1, 0x0
     248: 538070ef     	jal	0x7780 <memset>
     24c: 01800913     	li	s2, 0x18
     250: 41212423     	sw	s2, 0x408(sp)
     254: 4a010513     	addi	a0, sp, 0x4a0
     258: 0c800613     	li	a2, 0xc8
     25c: 00000593     	li	a1, 0x0
     260: 520070ef     	jal	0x7780 <memset>
     264: 57010513     	addi	a0, sp, 0x570
     268: 08900613     	li	a2, 0x89
     26c: 00000593     	li	a1, 0x0
     270: 510070ef     	jal	0x7780 <memset>
     274: 57212423     	sw	s2, 0x568(sp)
     278: 7ff10513     	addi	a0, sp, 0x7ff
     27c: 00150513     	addi	a0, a0, 0x1
     280: 0f800613     	li	a2, 0xf8
     284: 00000593     	li	a1, 0x0
     288: 4f8070ef     	jal	0x7780 <memset>
     28c: 60010513     	addi	a0, sp, 0x600
     290: 0f800613     	li	a2, 0xf8
     294: 00000593     	li	a1, 0x0
     298: 4e8070ef     	jal	0x7780 <memset>
     29c: 70012023     	sw	zero, 0x700(sp)
     2a0: 00001537     	lui	a0, 0x1
     2a4: 00a10533     	add	a0, sp, a0
     2a8: 90052023     	sw	zero, -0x700(a0)
     2ac: 7ff10513     	addi	a0, sp, 0x7ff
     2b0: 2fa50513     	addi	a0, a0, 0x2fa
     2b4: 0ff00613     	li	a2, 0xff
     2b8: 00000593     	li	a1, 0x0
     2bc: 4c4070ef     	jal	0x7780 <memset>
     2c0: 00000513     	li	a0, 0x0
     2c4: 08800b13     	li	s6, 0x88
     2c8: 00548913     	addi	s2, s1, 0x5
     2cc: 009935b3     	sltu	a1, s2, s1
     2d0: 10b12023     	sw	a1, 0x100(sp)
     2d4: 11212223     	sw	s2, 0x104(sp)
     2d8: 10a12423     	sw	a0, 0x108(sp)
     2dc: 20810513     	addi	a0, sp, 0x208
     2e0: 31c020ef     	jal	0x25fc <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7>
     2e4: 0ff57513     	zext.b	a0, a0
     2e8: 1ea12e23     	sw	a0, 0x1fc(sp)
     2ec: 00051463     	bnez	a0, 0x2f4 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x12c>
     2f0: 2110106f     	j	0x1d00 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1b38>
     2f4: 00000993     	li	s3, 0x0
     2f8: 0640006f     	j	0x35c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x194>
     2fc: 57010513     	addi	a0, sp, 0x570
     300: 00950533     	add	a0, a0, s1
     304: 7ff10593     	addi	a1, sp, 0x7ff
     308: 2fa58593     	addi	a1, a1, 0x2fa
     30c: 000b8613     	mv	a2, s7
     310: 460070ef     	jal	0x7770 <memcpy>
     314: 009b8a33     	add	s4, s7, s1
     318: 20012983     	lw	s3, 0x200(sp)
     31c: 00198993     	addi	s3, s3, 0x1
     320: 5f410c23     	sb	s4, 0x5f8(sp)
     324: 60010513     	addi	a0, sp, 0x600
     328: 7ff10593     	addi	a1, sp, 0x7ff
     32c: 2fa58593     	addi	a1, a1, 0x2fa
     330: 000b8613     	mv	a2, s7
     334: 271030ef     	jal	0x3da4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205>
     338: 7ff10513     	addi	a0, sp, 0x7ff
     33c: 00150513     	addi	a0, a0, 0x1
     340: 7ff10593     	addi	a1, sp, 0x7ff
     344: 2fa58593     	addi	a1, a1, 0x2fa
     348: 000b8613     	mv	a2, s7
     34c: 259030ef     	jal	0x3da4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205>
     350: 1fc12503     	lw	a0, 0x1fc(sp)
     354: 00a9e463     	bltu	s3, a0, 0x35c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x194>
     358: 1a90106f     	j	0x1d00 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1b38>
     35c: 20810513     	addi	a0, sp, 0x208
     360: 29c020ef     	jal	0x25fc <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7>
     364: 0ff57b93     	zext.b	s7, a0
     368: 020b8263     	beqz	s7, 0x38c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1c4>
     36c: 7ff10493     	addi	s1, sp, 0x7ff
     370: 2fa48493     	addi	s1, s1, 0x2fa
     374: 01748933     	add	s2, s1, s7
     378: 20810513     	addi	a0, sp, 0x208
     37c: 280020ef     	jal	0x25fc <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7>
     380: 00a48023     	sb	a0, 0x0(s1)
     384: 00148493     	addi	s1, s1, 0x1
     388: ff2498e3     	bne	s1, s2, 0x378 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1b0>
     38c: 49814483     	lbu	s1, 0x498(sp)
     390: 409b0633     	sub	a2, s6, s1
     394: 21312023     	sw	s3, 0x200(sp)
     398: 21712223     	sw	s7, 0x204(sp)
     39c: 02cbfa63     	bgeu	s7, a2, 0x3d0 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x208>
     3a0: 41010513     	addi	a0, sp, 0x410
     3a4: 00950533     	add	a0, a0, s1
     3a8: 7ff10593     	addi	a1, sp, 0x7ff
     3ac: 2fa58593     	addi	a1, a1, 0x2fa
     3b0: 000b8613     	mv	a2, s7
     3b4: 3bc070ef     	jal	0x7770 <memcpy>
     3b8: 009b8ab3     	add	s5, s7, s1
     3bc: 5f814483     	lbu	s1, 0x5f8(sp)
     3c0: 409b0633     	sub	a2, s6, s1
     3c4: 49510c23     	sb	s5, 0x498(sp)
     3c8: f2cbeae3     	bltu	s7, a2, 0x2fc <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x134>
     3cc: 4910006f     	j	0x105c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0xe94>
     3d0: 00049463     	bnez	s1, 0x3d8 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x210>
     3d4: 1050106f     	j	0x1cd8 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1b10>
     3d8: 41010513     	addi	a0, sp, 0x410
     3dc: 00950533     	add	a0, a0, s1
     3e0: 7ff10593     	addi	a1, sp, 0x7ff
     3e4: 2fa58593     	addi	a1, a1, 0x2fa
     3e8: 1ec12c23     	sw	a2, 0x1f8(sp)
     3ec: 384070ef     	jal	0x7770 <memcpy>
     3f0: 41012903     	lw	s2, 0x410(sp)
     3f4: 41412883     	lw	a7, 0x414(sp)
     3f8: 41812603     	lw	a2, 0x418(sp)
     3fc: 34012a83     	lw	s5, 0x340(sp)
     400: 34412283     	lw	t0, 0x344(sp)
     404: 34812783     	lw	a5, 0x348(sp)
     408: 34c12503     	lw	a0, 0x34c(sp)
     40c: 41c12803     	lw	a6, 0x41c(sp)
     410: 42012703     	lw	a4, 0x420(sp)
     414: 42412383     	lw	t2, 0x424(sp)
     418: 42812e03     	lw	t3, 0x428(sp)
     41c: 35012e83     	lw	t4, 0x350(sp)
     420: 35412f03     	lw	t5, 0x354(sp)
     424: 35812f83     	lw	t6, 0x358(sp)
     428: 35c12483     	lw	s1, 0x35c(sp)
     42c: 42c12b03     	lw	s6, 0x42c(sp)
     430: 43012303     	lw	t1, 0x430(sp)
     434: 43412983     	lw	s3, 0x434(sp)
     438: 43812a03     	lw	s4, 0x438(sp)
     43c: 36012683     	lw	a3, 0x360(sp)
     440: 36412b83     	lw	s7, 0x364(sp)
     444: 36812c03     	lw	s8, 0x368(sp)
     448: 36c12c83     	lw	s9, 0x36c(sp)
     44c: 0112c5b3     	xor	a1, t0, a7
     450: 1eb12a23     	sw	a1, 0x1f4(sp)
     454: 43c12883     	lw	a7, 0x43c(sp)
     458: 44012d83     	lw	s11, 0x440(sp)
     45c: 44412083     	lw	ra, 0x444(sp)
     460: 44812583     	lw	a1, 0x448(sp)
     464: 012ac2b3     	xor	t0, s5, s2
     468: 1e512223     	sw	t0, 0x1e4(sp)
     46c: 01054533     	xor	a0, a0, a6
     470: 1ea12823     	sw	a0, 0x1f0(sp)
     474: 00c7c633     	xor	a2, a5, a2
     478: 1ec12023     	sw	a2, 0x1e0(sp)
     47c: 007f4533     	xor	a0, t5, t2
     480: 1ea12623     	sw	a0, 0x1ec(sp)
     484: 37012603     	lw	a2, 0x370(sp)
     488: 37412f03     	lw	t5, 0x374(sp)
     48c: 37812503     	lw	a0, 0x378(sp)
     490: 37c12a83     	lw	s5, 0x37c(sp)
     494: 00eec733     	xor	a4, t4, a4
     498: 1ce12c23     	sw	a4, 0x1d8(sp)
     49c: 0164c733     	xor	a4, s1, s6
     4a0: 1ce12e23     	sw	a4, 0x1dc(sp)
     4a4: 01cfc733     	xor	a4, t6, t3
     4a8: 1ce12a23     	sw	a4, 0x1d4(sp)
     4ac: 013bc733     	xor	a4, s7, s3
     4b0: 1ee12423     	sw	a4, 0x1e8(sp)
     4b4: 44c12703     	lw	a4, 0x44c(sp)
     4b8: 45012983     	lw	s3, 0x450(sp)
     4bc: 45412b83     	lw	s7, 0x454(sp)
     4c0: 45812383     	lw	t2, 0x458(sp)
     4c4: 0066c4b3     	xor	s1, a3, t1
     4c8: 011cc6b3     	xor	a3, s9, a7
     4cc: 1cd12823     	sw	a3, 0x1d0(sp)
     4d0: 014c4e33     	xor	t3, s8, s4
     4d4: 01b64633     	xor	a2, a2, s11
     4d8: 1cc12623     	sw	a2, 0x1cc(sp)
     4dc: 38012603     	lw	a2, 0x380(sp)
     4e0: 38412a03     	lw	s4, 0x384(sp)
     4e4: 38812c03     	lw	s8, 0x388(sp)
     4e8: 38c12c83     	lw	s9, 0x38c(sp)
     4ec: 001f4fb3     	xor	t6, t5, ra
     4f0: 00eaceb3     	xor	t4, s5, a4
     4f4: 00b54333     	xor	t1, a0, a1
     4f8: 017a4f33     	xor	t5, s4, s7
     4fc: 45c12503     	lw	a0, 0x45c(sp)
     500: 46012583     	lw	a1, 0x460(sp)
     504: 46412703     	lw	a4, 0x464(sp)
     508: 46812d83     	lw	s11, 0x468(sp)
     50c: 01364bb3     	xor	s7, a2, s3
     510: 007c4a33     	xor	s4, s8, t2
     514: 00accab3     	xor	s5, s9, a0
     518: 39012503     	lw	a0, 0x390(sp)
     51c: 39412603     	lw	a2, 0x394(sp)
     520: 39812383     	lw	t2, 0x398(sp)
     524: 39c12983     	lw	s3, 0x39c(sp)
     528: 00b54cb3     	xor	s9, a0, a1
     52c: 00e642b3     	xor	t0, a2, a4
     530: 01b3cdb3     	xor	s11, t2, s11
     534: 46c12503     	lw	a0, 0x46c(sp)
     538: 47012583     	lw	a1, 0x470(sp)
     53c: 47412603     	lw	a2, 0x474(sp)
     540: 47812703     	lw	a4, 0x478(sp)
     544: 00a9c0b3     	xor	ra, s3, a0
     548: 3a012503     	lw	a0, 0x3a0(sp)
     54c: 3a412383     	lw	t2, 0x3a4(sp)
     550: 3a812983     	lw	s3, 0x3a8(sp)
     554: 3ac12683     	lw	a3, 0x3ac(sp)
     558: 00b54c33     	xor	s8, a0, a1
     55c: 00c3c8b3     	xor	a7, t2, a2
     560: 00e9c833     	xor	a6, s3, a4
     564: 47c12583     	lw	a1, 0x47c(sp)
     568: 48012703     	lw	a4, 0x480(sp)
     56c: 48412383     	lw	t2, 0x484(sp)
     570: 48812783     	lw	a5, 0x488(sp)
     574: 00b6c9b3     	xor	s3, a3, a1
     578: 3b012583     	lw	a1, 0x3b0(sp)
     57c: 3b412683     	lw	a3, 0x3b4(sp)
     580: 3b812b03     	lw	s6, 0x3b8(sp)
     584: 3bc12903     	lw	s2, 0x3bc(sp)
     588: 00e5c733     	xor	a4, a1, a4
     58c: 0076c6b3     	xor	a3, a3, t2
     590: 48c12603     	lw	a2, 0x48c(sp)
     594: 49012383     	lw	t2, 0x490(sp)
     598: 49412d03     	lw	s10, 0x494(sp)
     59c: 3c012503     	lw	a0, 0x3c0(sp)
     5a0: 3c412583     	lw	a1, 0x3c4(sp)
     5a4: 00fb47b3     	xor	a5, s6, a5
     5a8: 08800b13     	li	s6, 0x88
     5ac: 00c94633     	xor	a2, s2, a2
     5b0: 00754533     	xor	a0, a0, t2
     5b4: 01a5c3b3     	xor	t2, a1, s10
     5b8: 1e412583     	lw	a1, 0x1e4(sp)
     5bc: 34b12023     	sw	a1, 0x340(sp)
     5c0: 1f412583     	lw	a1, 0x1f4(sp)
     5c4: 34b12223     	sw	a1, 0x344(sp)
     5c8: 1e012583     	lw	a1, 0x1e0(sp)
     5cc: 34b12423     	sw	a1, 0x348(sp)
     5d0: 1f012583     	lw	a1, 0x1f0(sp)
     5d4: 34b12623     	sw	a1, 0x34c(sp)
     5d8: 1d812583     	lw	a1, 0x1d8(sp)
     5dc: 34b12823     	sw	a1, 0x350(sp)
     5e0: 1ec12583     	lw	a1, 0x1ec(sp)
     5e4: 34b12a23     	sw	a1, 0x354(sp)
     5e8: 1d412583     	lw	a1, 0x1d4(sp)
     5ec: 34b12c23     	sw	a1, 0x358(sp)
     5f0: 1dc12583     	lw	a1, 0x1dc(sp)
     5f4: 34b12e23     	sw	a1, 0x35c(sp)
     5f8: 36912023     	sw	s1, 0x360(sp)
     5fc: 40812583     	lw	a1, 0x408(sp)
     600: 1e812483     	lw	s1, 0x1e8(sp)
     604: 36912223     	sw	s1, 0x364(sp)
     608: 37c12423     	sw	t3, 0x368(sp)
     60c: 1d012e03     	lw	t3, 0x1d0(sp)
     610: 37c12623     	sw	t3, 0x36c(sp)
     614: 1cc12e03     	lw	t3, 0x1cc(sp)
     618: 37c12823     	sw	t3, 0x370(sp)
     61c: 37f12a23     	sw	t6, 0x374(sp)
     620: 36612c23     	sw	t1, 0x378(sp)
     624: 37d12e23     	sw	t4, 0x37c(sp)
     628: 39712023     	sw	s7, 0x380(sp)
     62c: 20412b83     	lw	s7, 0x204(sp)
     630: 39e12223     	sw	t5, 0x384(sp)
     634: 39412423     	sw	s4, 0x388(sp)
     638: 39512623     	sw	s5, 0x38c(sp)
     63c: 39912823     	sw	s9, 0x390(sp)
     640: 38512a23     	sw	t0, 0x394(sp)
     644: 39b12c23     	sw	s11, 0x398(sp)
     648: 38112e23     	sw	ra, 0x39c(sp)
     64c: 3b812023     	sw	s8, 0x3a0(sp)
     650: 3b112223     	sw	a7, 0x3a4(sp)
     654: 3b012423     	sw	a6, 0x3a8(sp)
     658: 3b312623     	sw	s3, 0x3ac(sp)
     65c: 3ae12823     	sw	a4, 0x3b0(sp)
     660: 3ad12a23     	sw	a3, 0x3b4(sp)
     664: 3af12c23     	sw	a5, 0x3b8(sp)
     668: 3ac12e23     	sw	a2, 0x3bc(sp)
     66c: 7ff10a13     	addi	s4, sp, 0x7ff
     670: 2faa0a13     	addi	s4, s4, 0x2fa
     674: 1f812603     	lw	a2, 0x1f8(sp)
     678: 00ca0a33     	add	s4, s4, a2
     67c: 3ca12023     	sw	a0, 0x3c0(sp)
     680: 3c712223     	sw	t2, 0x3c4(sp)
     684: 40cb84b3     	sub	s1, s7, a2
     688: 34010513     	addi	a0, sp, 0x340
     68c: 5c1040ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
     690: 1964ece3     	bltu	s1, s6, 0x1028 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0xe60>
     694: 000a4f83     	lbu	t6, 0x0(s4)
     698: 001a4c83     	lbu	s9, 0x1(s4)
     69c: 002a4583     	lbu	a1, 0x2(s4)
     6a0: 003a4603     	lbu	a2, 0x3(s4)
     6a4: 004a4503     	lbu	a0, 0x4(s4)
     6a8: 005a4683     	lbu	a3, 0x5(s4)
     6ac: 006a4283     	lbu	t0, 0x6(s4)
     6b0: 007a4e03     	lbu	t3, 0x7(s4)
     6b4: 1e912c23     	sw	s1, 0x1f8(sp)
     6b8: 008a4483     	lbu	s1, 0x8(s4)
     6bc: 009a4903     	lbu	s2, 0x9(s4)
     6c0: 00aa4803     	lbu	a6, 0xa(s4)
     6c4: 00ba4303     	lbu	t1, 0xb(s4)
     6c8: 00ca4703     	lbu	a4, 0xc(s4)
     6cc: 00da4e83     	lbu	t4, 0xd(s4)
     6d0: 00ea4a83     	lbu	s5, 0xe(s4)
     6d4: 00fa4b83     	lbu	s7, 0xf(s4)
     6d8: 010a4d03     	lbu	s10, 0x10(s4)
     6dc: 011a4083     	lbu	ra, 0x11(s4)
     6e0: 012a4883     	lbu	a7, 0x12(s4)
     6e4: 013a4383     	lbu	t2, 0x13(s4)
     6e8: 014a4783     	lbu	a5, 0x14(s4)
     6ec: 015a4f03     	lbu	t5, 0x15(s4)
     6f0: 016a4b03     	lbu	s6, 0x16(s4)
     6f4: 017a4c03     	lbu	s8, 0x17(s4)
     6f8: 008c9c93     	slli	s9, s9, 0x8
     6fc: 01fcefb3     	or	t6, s9, t6
     700: 1ff12a23     	sw	t6, 0x1f4(sp)
     704: 018a4d83     	lbu	s11, 0x18(s4)
     708: 019a4983     	lbu	s3, 0x19(s4)
     70c: 01aa4f83     	lbu	t6, 0x1a(s4)
     710: 01ba4c83     	lbu	s9, 0x1b(s4)
     714: 01059593     	slli	a1, a1, 0x10
     718: 01861613     	slli	a2, a2, 0x18
     71c: 00869693     	slli	a3, a3, 0x8
     720: 01029293     	slli	t0, t0, 0x10
     724: 018e1e13     	slli	t3, t3, 0x18
     728: 00891913     	slli	s2, s2, 0x8
     72c: 00b665b3     	or	a1, a2, a1
     730: 1eb12823     	sw	a1, 0x1f0(sp)
     734: 00a6e533     	or	a0, a3, a0
     738: 1ea12623     	sw	a0, 0x1ec(sp)
     73c: 005e6533     	or	a0, t3, t0
     740: 1ea12423     	sw	a0, 0x1e8(sp)
     744: 00996533     	or	a0, s2, s1
     748: 1ea12223     	sw	a0, 0x1e4(sp)
     74c: 01ca4503     	lbu	a0, 0x1c(s4)
     750: 01da4583     	lbu	a1, 0x1d(s4)
     754: 01ea4603     	lbu	a2, 0x1e(s4)
     758: 01fa4683     	lbu	a3, 0x1f(s4)
     75c: 01081813     	slli	a6, a6, 0x10
     760: 01831313     	slli	t1, t1, 0x18
     764: 008e9e93     	slli	t4, t4, 0x8
     768: 010a9a93     	slli	s5, s5, 0x10
     76c: 018b9b93     	slli	s7, s7, 0x18
     770: 00809093     	slli	ra, ra, 0x8
     774: 01036833     	or	a6, t1, a6
     778: 1f012023     	sw	a6, 0x1e0(sp)
     77c: 00eee733     	or	a4, t4, a4
     780: 1ce12e23     	sw	a4, 0x1dc(sp)
     784: 015beab3     	or	s5, s7, s5
     788: 01a0ebb3     	or	s7, ra, s10
     78c: 020a4703     	lbu	a4, 0x20(s4)
     790: 021a4803     	lbu	a6, 0x21(s4)
     794: 022a4283     	lbu	t0, 0x22(s4)
     798: 023a4303     	lbu	t1, 0x23(s4)
     79c: 01089893     	slli	a7, a7, 0x10
     7a0: 01839393     	slli	t2, t2, 0x18
     7a4: 008f1f13     	slli	t5, t5, 0x8
     7a8: 010b1b13     	slli	s6, s6, 0x10
     7ac: 018c1c13     	slli	s8, s8, 0x18
     7b0: 00899993     	slli	s3, s3, 0x8
     7b4: 0113ed33     	or	s10, t2, a7
     7b8: 00ff67b3     	or	a5, t5, a5
     7bc: 1cf12c23     	sw	a5, 0x1d8(sp)
     7c0: 016c67b3     	or	a5, s8, s6
     7c4: 1cf12a23     	sw	a5, 0x1d4(sp)
     7c8: 01b9e7b3     	or	a5, s3, s11
     7cc: 1cf12823     	sw	a5, 0x1d0(sp)
     7d0: 024a4783     	lbu	a5, 0x24(s4)
     7d4: 025a4883     	lbu	a7, 0x25(s4)
     7d8: 026a4383     	lbu	t2, 0x26(s4)
     7dc: 027a4e03     	lbu	t3, 0x27(s4)
     7e0: 010f9f93     	slli	t6, t6, 0x10
     7e4: 018c9c93     	slli	s9, s9, 0x18
     7e8: 00859593     	slli	a1, a1, 0x8
     7ec: 01061613     	slli	a2, a2, 0x10
     7f0: 01869693     	slli	a3, a3, 0x18
     7f4: 00881813     	slli	a6, a6, 0x8
     7f8: 01fcefb3     	or	t6, s9, t6
     7fc: 00a5ecb3     	or	s9, a1, a0
     800: 00c6e633     	or	a2, a3, a2
     804: 1cc12623     	sw	a2, 0x1cc(sp)
     808: 00e86533     	or	a0, a6, a4
     80c: 1ca12423     	sw	a0, 0x1c8(sp)
     810: 028a4503     	lbu	a0, 0x28(s4)
     814: 029a4583     	lbu	a1, 0x29(s4)
     818: 02aa4603     	lbu	a2, 0x2a(s4)
     81c: 02ba4683     	lbu	a3, 0x2b(s4)
     820: 01029293     	slli	t0, t0, 0x10
     824: 01831313     	slli	t1, t1, 0x18
     828: 00889893     	slli	a7, a7, 0x8
     82c: 01039393     	slli	t2, t2, 0x10
     830: 018e1e13     	slli	t3, t3, 0x18
     834: 00859593     	slli	a1, a1, 0x8
     838: 00536733     	or	a4, t1, t0
     83c: 1ce12223     	sw	a4, 0x1c4(sp)
     840: 00f8e733     	or	a4, a7, a5
     844: 1ce12023     	sw	a4, 0x1c0(sp)
     848: 007e6733     	or	a4, t3, t2
     84c: 1ae12e23     	sw	a4, 0x1bc(sp)
     850: 00a5e533     	or	a0, a1, a0
     854: 1aa12c23     	sw	a0, 0x1b8(sp)
     858: 02ca4503     	lbu	a0, 0x2c(s4)
     85c: 02da4583     	lbu	a1, 0x2d(s4)
     860: 02ea4703     	lbu	a4, 0x2e(s4)
     864: 02fa4783     	lbu	a5, 0x2f(s4)
     868: 01061613     	slli	a2, a2, 0x10
     86c: 01869693     	slli	a3, a3, 0x18
     870: 00859593     	slli	a1, a1, 0x8
     874: 01071713     	slli	a4, a4, 0x10
     878: 01879793     	slli	a5, a5, 0x18
     87c: 00c6e633     	or	a2, a3, a2
     880: 1ac12a23     	sw	a2, 0x1b4(sp)
     884: 00a5e533     	or	a0, a1, a0
     888: 1aa12823     	sw	a0, 0x1b0(sp)
     88c: 00e7e733     	or	a4, a5, a4
     890: 1ae12623     	sw	a4, 0x1ac(sp)
     894: 035a4503     	lbu	a0, 0x35(s4)
     898: 034a4583     	lbu	a1, 0x34(s4)
     89c: 036a4603     	lbu	a2, 0x36(s4)
     8a0: 037a4683     	lbu	a3, 0x37(s4)
     8a4: 00851513     	slli	a0, a0, 0x8
     8a8: 00b56533     	or	a0, a0, a1
     8ac: 1aa12423     	sw	a0, 0x1a8(sp)
     8b0: 01061613     	slli	a2, a2, 0x10
     8b4: 01869693     	slli	a3, a3, 0x18
     8b8: 00c6e633     	or	a2, a3, a2
     8bc: 1ac12223     	sw	a2, 0x1a4(sp)
     8c0: 031a4503     	lbu	a0, 0x31(s4)
     8c4: 030a4583     	lbu	a1, 0x30(s4)
     8c8: 032a4603     	lbu	a2, 0x32(s4)
     8cc: 033a4683     	lbu	a3, 0x33(s4)
     8d0: 00851513     	slli	a0, a0, 0x8
     8d4: 00b56533     	or	a0, a0, a1
     8d8: 1aa12023     	sw	a0, 0x1a0(sp)
     8dc: 01061613     	slli	a2, a2, 0x10
     8e0: 01869693     	slli	a3, a3, 0x18
     8e4: 00c6e633     	or	a2, a3, a2
     8e8: 18c12e23     	sw	a2, 0x19c(sp)
     8ec: 039a4503     	lbu	a0, 0x39(s4)
     8f0: 038a4583     	lbu	a1, 0x38(s4)
     8f4: 03aa4603     	lbu	a2, 0x3a(s4)
     8f8: 03ba4683     	lbu	a3, 0x3b(s4)
     8fc: 00851513     	slli	a0, a0, 0x8
     900: 00b56533     	or	a0, a0, a1
     904: 18a12c23     	sw	a0, 0x198(sp)
     908: 01061613     	slli	a2, a2, 0x10
     90c: 01869693     	slli	a3, a3, 0x18
     910: 00c6e633     	or	a2, a3, a2
     914: 18c12a23     	sw	a2, 0x194(sp)
     918: 03da4503     	lbu	a0, 0x3d(s4)
     91c: 03ca4583     	lbu	a1, 0x3c(s4)
     920: 03ea4603     	lbu	a2, 0x3e(s4)
     924: 03fa4683     	lbu	a3, 0x3f(s4)
     928: 00851513     	slli	a0, a0, 0x8
     92c: 00b56533     	or	a0, a0, a1
     930: 18a12823     	sw	a0, 0x190(sp)
     934: 01061613     	slli	a2, a2, 0x10
     938: 01869693     	slli	a3, a3, 0x18
     93c: 00c6e633     	or	a2, a3, a2
     940: 18c12623     	sw	a2, 0x18c(sp)
     944: 041a4503     	lbu	a0, 0x41(s4)
     948: 040a4583     	lbu	a1, 0x40(s4)
     94c: 042a4603     	lbu	a2, 0x42(s4)
     950: 043a4683     	lbu	a3, 0x43(s4)
     954: 00851513     	slli	a0, a0, 0x8
     958: 00b56533     	or	a0, a0, a1
     95c: 18a12423     	sw	a0, 0x188(sp)
     960: 01061613     	slli	a2, a2, 0x10
     964: 01869693     	slli	a3, a3, 0x18
     968: 00c6e633     	or	a2, a3, a2
     96c: 18c12223     	sw	a2, 0x184(sp)
     970: 045a4503     	lbu	a0, 0x45(s4)
     974: 044a4583     	lbu	a1, 0x44(s4)
     978: 046a4603     	lbu	a2, 0x46(s4)
     97c: 047a4683     	lbu	a3, 0x47(s4)
     980: 00851513     	slli	a0, a0, 0x8
     984: 00b56533     	or	a0, a0, a1
     988: 18a12023     	sw	a0, 0x180(sp)
     98c: 01061613     	slli	a2, a2, 0x10
     990: 01869693     	slli	a3, a3, 0x18
     994: 00c6e633     	or	a2, a3, a2
     998: 16c12e23     	sw	a2, 0x17c(sp)
     99c: 04da4503     	lbu	a0, 0x4d(s4)
     9a0: 04ca4583     	lbu	a1, 0x4c(s4)
     9a4: 04ea4603     	lbu	a2, 0x4e(s4)
     9a8: 04fa4683     	lbu	a3, 0x4f(s4)
     9ac: 00851513     	slli	a0, a0, 0x8
     9b0: 00b56533     	or	a0, a0, a1
     9b4: 16a12c23     	sw	a0, 0x178(sp)
     9b8: 01061613     	slli	a2, a2, 0x10
     9bc: 01869693     	slli	a3, a3, 0x18
     9c0: 00c6e633     	or	a2, a3, a2
     9c4: 16c12a23     	sw	a2, 0x174(sp)
     9c8: 049a4503     	lbu	a0, 0x49(s4)
     9cc: 048a4583     	lbu	a1, 0x48(s4)
     9d0: 04aa4603     	lbu	a2, 0x4a(s4)
     9d4: 04ba4683     	lbu	a3, 0x4b(s4)
     9d8: 00851513     	slli	a0, a0, 0x8
     9dc: 00b56533     	or	a0, a0, a1
     9e0: 16a12823     	sw	a0, 0x170(sp)
     9e4: 01061613     	slli	a2, a2, 0x10
     9e8: 01869693     	slli	a3, a3, 0x18
     9ec: 00c6e633     	or	a2, a3, a2
     9f0: 16c12623     	sw	a2, 0x16c(sp)
     9f4: 055a4503     	lbu	a0, 0x55(s4)
     9f8: 054a4583     	lbu	a1, 0x54(s4)
     9fc: 056a4603     	lbu	a2, 0x56(s4)
     a00: 057a4683     	lbu	a3, 0x57(s4)
     a04: 00851513     	slli	a0, a0, 0x8
     a08: 00b56533     	or	a0, a0, a1
     a0c: 16a12423     	sw	a0, 0x168(sp)
     a10: 01061613     	slli	a2, a2, 0x10
     a14: 01869693     	slli	a3, a3, 0x18
     a18: 00c6e633     	or	a2, a3, a2
     a1c: 16c12223     	sw	a2, 0x164(sp)
     a20: 051a4503     	lbu	a0, 0x51(s4)
     a24: 050a4583     	lbu	a1, 0x50(s4)
     a28: 052a4603     	lbu	a2, 0x52(s4)
     a2c: 053a4683     	lbu	a3, 0x53(s4)
     a30: 00851513     	slli	a0, a0, 0x8
     a34: 00b56533     	or	a0, a0, a1
     a38: 16a12023     	sw	a0, 0x160(sp)
     a3c: 01061613     	slli	a2, a2, 0x10
     a40: 01869693     	slli	a3, a3, 0x18
     a44: 00c6e633     	or	a2, a3, a2
     a48: 14c12e23     	sw	a2, 0x15c(sp)
     a4c: 05da4503     	lbu	a0, 0x5d(s4)
     a50: 05ca4583     	lbu	a1, 0x5c(s4)
     a54: 05ea4703     	lbu	a4, 0x5e(s4)
     a58: 05fa4783     	lbu	a5, 0x5f(s4)
     a5c: 00851513     	slli	a0, a0, 0x8
     a60: 00b56533     	or	a0, a0, a1
     a64: 14a12c23     	sw	a0, 0x158(sp)
     a68: 01071713     	slli	a4, a4, 0x10
     a6c: 01879793     	slli	a5, a5, 0x18
     a70: 00e7e733     	or	a4, a5, a4
     a74: 14e12a23     	sw	a4, 0x154(sp)
     a78: 059a4503     	lbu	a0, 0x59(s4)
     a7c: 058a4583     	lbu	a1, 0x58(s4)
     a80: 05aa4703     	lbu	a4, 0x5a(s4)
     a84: 05ba4783     	lbu	a5, 0x5b(s4)
     a88: 00851513     	slli	a0, a0, 0x8
     a8c: 00b56533     	or	a0, a0, a1
     a90: 14a12823     	sw	a0, 0x150(sp)
     a94: 01071713     	slli	a4, a4, 0x10
     a98: 01879793     	slli	a5, a5, 0x18
     a9c: 00e7e733     	or	a4, a5, a4
     aa0: 14e12623     	sw	a4, 0x14c(sp)
     aa4: 065a4503     	lbu	a0, 0x65(s4)
     aa8: 064a4583     	lbu	a1, 0x64(s4)
     aac: 066a4703     	lbu	a4, 0x66(s4)
     ab0: 067a4783     	lbu	a5, 0x67(s4)
     ab4: 00851513     	slli	a0, a0, 0x8
     ab8: 00b56533     	or	a0, a0, a1
     abc: 14a12423     	sw	a0, 0x148(sp)
     ac0: 01071713     	slli	a4, a4, 0x10
     ac4: 01879793     	slli	a5, a5, 0x18
     ac8: 00e7e733     	or	a4, a5, a4
     acc: 14e12223     	sw	a4, 0x144(sp)
     ad0: 061a4503     	lbu	a0, 0x61(s4)
     ad4: 060a4583     	lbu	a1, 0x60(s4)
     ad8: 062a4703     	lbu	a4, 0x62(s4)
     adc: 063a4783     	lbu	a5, 0x63(s4)
     ae0: 00851513     	slli	a0, a0, 0x8
     ae4: 00b56533     	or	a0, a0, a1
     ae8: 14a12023     	sw	a0, 0x140(sp)
     aec: 01071713     	slli	a4, a4, 0x10
     af0: 01879793     	slli	a5, a5, 0x18
     af4: 00e7e733     	or	a4, a5, a4
     af8: 12e12e23     	sw	a4, 0x13c(sp)
     afc: 06da4503     	lbu	a0, 0x6d(s4)
     b00: 06ca4583     	lbu	a1, 0x6c(s4)
     b04: 06ea4e83     	lbu	t4, 0x6e(s4)
     b08: 06fa4f03     	lbu	t5, 0x6f(s4)
     b0c: 00851513     	slli	a0, a0, 0x8
     b10: 00b56533     	or	a0, a0, a1
     b14: 12a12c23     	sw	a0, 0x138(sp)
     b18: 010e9e93     	slli	t4, t4, 0x10
     b1c: 018f1f13     	slli	t5, t5, 0x18
     b20: 01df6533     	or	a0, t5, t4
     b24: 12a12a23     	sw	a0, 0x134(sp)
     b28: 069a4503     	lbu	a0, 0x69(s4)
     b2c: 068a4583     	lbu	a1, 0x68(s4)
     b30: 06aa4483     	lbu	s1, 0x6a(s4)
     b34: 06ba4903     	lbu	s2, 0x6b(s4)
     b38: 00851513     	slli	a0, a0, 0x8
     b3c: 00b56533     	or	a0, a0, a1
     b40: 12a12823     	sw	a0, 0x130(sp)
     b44: 01049493     	slli	s1, s1, 0x10
     b48: 01891913     	slli	s2, s2, 0x18
     b4c: 00996533     	or	a0, s2, s1
     b50: 12a12623     	sw	a0, 0x12c(sp)
     b54: 075a4503     	lbu	a0, 0x75(s4)
     b58: 074a4583     	lbu	a1, 0x74(s4)
     b5c: 076a4903     	lbu	s2, 0x76(s4)
     b60: 077a4b03     	lbu	s6, 0x77(s4)
     b64: 00851513     	slli	a0, a0, 0x8
     b68: 00b56533     	or	a0, a0, a1
     b6c: 12a12423     	sw	a0, 0x128(sp)
     b70: 01091913     	slli	s2, s2, 0x10
     b74: 018b1b13     	slli	s6, s6, 0x18
     b78: 012b6533     	or	a0, s6, s2
     b7c: 12a12223     	sw	a0, 0x124(sp)
     b80: 071a4503     	lbu	a0, 0x71(s4)
     b84: 070a4583     	lbu	a1, 0x70(s4)
     b88: 072a4903     	lbu	s2, 0x72(s4)
     b8c: 073a4b03     	lbu	s6, 0x73(s4)
     b90: 00851513     	slli	a0, a0, 0x8
     b94: 00b56533     	or	a0, a0, a1
     b98: 12a12023     	sw	a0, 0x120(sp)
     b9c: 01091913     	slli	s2, s2, 0x10
     ba0: 018b1b13     	slli	s6, s6, 0x18
     ba4: 012b6533     	or	a0, s6, s2
     ba8: 10a12e23     	sw	a0, 0x11c(sp)
     bac: 07da4503     	lbu	a0, 0x7d(s4)
     bb0: 07ca4583     	lbu	a1, 0x7c(s4)
     bb4: 07ea4903     	lbu	s2, 0x7e(s4)
     bb8: 07fa4b03     	lbu	s6, 0x7f(s4)
     bbc: 00851513     	slli	a0, a0, 0x8
     bc0: 00b56533     	or	a0, a0, a1
     bc4: 10a12c23     	sw	a0, 0x118(sp)
     bc8: 01091913     	slli	s2, s2, 0x10
     bcc: 018b1b13     	slli	s6, s6, 0x18
     bd0: 012b6533     	or	a0, s6, s2
     bd4: 10a12a23     	sw	a0, 0x114(sp)
     bd8: 079a4c03     	lbu	s8, 0x79(s4)
     bdc: 078a4583     	lbu	a1, 0x78(s4)
     be0: 07aa4903     	lbu	s2, 0x7a(s4)
     be4: 07ba4503     	lbu	a0, 0x7b(s4)
     be8: 008c1c13     	slli	s8, s8, 0x8
     bec: 00bc65b3     	or	a1, s8, a1
     bf0: 10b12823     	sw	a1, 0x110(sp)
     bf4: 01091913     	slli	s2, s2, 0x10
     bf8: 01851513     	slli	a0, a0, 0x18
     bfc: 01256533     	or	a0, a0, s2
     c00: 10a12623     	sw	a0, 0x10c(sp)
     c04: 085a4603     	lbu	a2, 0x85(s4)
     c08: 084a4683     	lbu	a3, 0x84(s4)
     c0c: 086a4583     	lbu	a1, 0x86(s4)
     c10: 087a4503     	lbu	a0, 0x87(s4)
     c14: 00861613     	slli	a2, a2, 0x8
     c18: 00d669b3     	or	s3, a2, a3
     c1c: 01059593     	slli	a1, a1, 0x10
     c20: 01851513     	slli	a0, a0, 0x18
     c24: 00b56633     	or	a2, a0, a1
     c28: 081a4583     	lbu	a1, 0x81(s4)
     c2c: 080a4683     	lbu	a3, 0x80(s4)
     c30: 082a4703     	lbu	a4, 0x82(s4)
     c34: 083a4783     	lbu	a5, 0x83(s4)
     c38: 00859593     	slli	a1, a1, 0x8
     c3c: 00d5e5b3     	or	a1, a1, a3
     c40: 01071713     	slli	a4, a4, 0x10
     c44: 01879793     	slli	a5, a5, 0x18
     c48: 00e7e733     	or	a4, a5, a4
     c4c: 1f412503     	lw	a0, 0x1f4(sp)
     c50: 1f012783     	lw	a5, 0x1f0(sp)
     c54: 00a7e7b3     	or	a5, a5, a0
     c58: 1ec12503     	lw	a0, 0x1ec(sp)
     c5c: 1e812683     	lw	a3, 0x1e8(sp)
     c60: 00a6e533     	or	a0, a3, a0
     c64: 1e412683     	lw	a3, 0x1e4(sp)
     c68: 1e012803     	lw	a6, 0x1e0(sp)
     c6c: 00d86833     	or	a6, a6, a3
     c70: 1dc12683     	lw	a3, 0x1dc(sp)
     c74: 00dae6b3     	or	a3, s5, a3
     c78: 017d6333     	or	t1, s10, s7
     c7c: 1d812883     	lw	a7, 0x1d8(sp)
     c80: 1d412283     	lw	t0, 0x1d4(sp)
     c84: 0112e2b3     	or	t0, t0, a7
     c88: 1d012883     	lw	a7, 0x1d0(sp)
     c8c: 011fe3b3     	or	t2, t6, a7
     c90: 1cc12883     	lw	a7, 0x1cc(sp)
     c94: 0198e8b3     	or	a7, a7, s9
     c98: 1c812e03     	lw	t3, 0x1c8(sp)
     c9c: 1c412e83     	lw	t4, 0x1c4(sp)
     ca0: 01ceefb3     	or	t6, t4, t3
     ca4: 1c012e03     	lw	t3, 0x1c0(sp)
     ca8: 1bc12e83     	lw	t4, 0x1bc(sp)
     cac: 01ceeeb3     	or	t4, t4, t3
     cb0: 1b812e03     	lw	t3, 0x1b8(sp)
     cb4: 1b412f03     	lw	t5, 0x1b4(sp)
     cb8: 01cf6f33     	or	t5, t5, t3
     cbc: 1b012e03     	lw	t3, 0x1b0(sp)
     cc0: 1ac12483     	lw	s1, 0x1ac(sp)
     cc4: 01c4ee33     	or	t3, s1, t3
     cc8: 1a812483     	lw	s1, 0x1a8(sp)
     ccc: 1a412903     	lw	s2, 0x1a4(sp)
     cd0: 00996b33     	or	s6, s2, s1
     cd4: 1a012483     	lw	s1, 0x1a0(sp)
     cd8: 19c12903     	lw	s2, 0x19c(sp)
     cdc: 009964b3     	or	s1, s2, s1
     ce0: 19812903     	lw	s2, 0x198(sp)
     ce4: 19412a83     	lw	s5, 0x194(sp)
     ce8: 012aeab3     	or	s5, s5, s2
     cec: 19012903     	lw	s2, 0x190(sp)
     cf0: 18c12b83     	lw	s7, 0x18c(sp)
     cf4: 012be933     	or	s2, s7, s2
     cf8: 18812b83     	lw	s7, 0x188(sp)
     cfc: 18412c03     	lw	s8, 0x184(sp)
     d00: 017c6cb3     	or	s9, s8, s7
     d04: 18012b83     	lw	s7, 0x180(sp)
     d08: 17c12c03     	lw	s8, 0x17c(sp)
     d0c: 017c6bb3     	or	s7, s8, s7
     d10: 17812c03     	lw	s8, 0x178(sp)
     d14: 17412d03     	lw	s10, 0x174(sp)
     d18: 018d6d33     	or	s10, s10, s8
     d1c: 17012c03     	lw	s8, 0x170(sp)
     d20: 16c12083     	lw	ra, 0x16c(sp)
     d24: 0180ec33     	or	s8, ra, s8
     d28: 16812083     	lw	ra, 0x168(sp)
     d2c: 16412d83     	lw	s11, 0x164(sp)
     d30: 001dedb3     	or	s11, s11, ra
     d34: 1bb12623     	sw	s11, 0x1ac(sp)
     d38: 16012d83     	lw	s11, 0x160(sp)
     d3c: 15c12083     	lw	ra, 0x15c(sp)
     d40: 01b0edb3     	or	s11, ra, s11
     d44: 1bb12023     	sw	s11, 0x1a0(sp)
     d48: 15812d83     	lw	s11, 0x158(sp)
     d4c: 15412083     	lw	ra, 0x154(sp)
     d50: 01b0edb3     	or	s11, ra, s11
     d54: 1bb12423     	sw	s11, 0x1a8(sp)
     d58: 15012d83     	lw	s11, 0x150(sp)
     d5c: 14c12083     	lw	ra, 0x14c(sp)
     d60: 01b0edb3     	or	s11, ra, s11
     d64: 19b12e23     	sw	s11, 0x19c(sp)
     d68: 14812d83     	lw	s11, 0x148(sp)
     d6c: 14412083     	lw	ra, 0x144(sp)
     d70: 01b0edb3     	or	s11, ra, s11
     d74: 1db12623     	sw	s11, 0x1cc(sp)
     d78: 14012d83     	lw	s11, 0x140(sp)
     d7c: 13c12083     	lw	ra, 0x13c(sp)
     d80: 01b0edb3     	or	s11, ra, s11
     d84: 1bb12e23     	sw	s11, 0x1bc(sp)
     d88: 13812d83     	lw	s11, 0x138(sp)
     d8c: 13412083     	lw	ra, 0x134(sp)
     d90: 01b0edb3     	or	s11, ra, s11
     d94: 1db12423     	sw	s11, 0x1c8(sp)
     d98: 13012d83     	lw	s11, 0x130(sp)
     d9c: 12c12083     	lw	ra, 0x12c(sp)
     da0: 01b0edb3     	or	s11, ra, s11
     da4: 1bb12c23     	sw	s11, 0x1b8(sp)
     da8: 12812d83     	lw	s11, 0x128(sp)
     dac: 12412083     	lw	ra, 0x124(sp)
     db0: 01b0edb3     	or	s11, ra, s11
     db4: 1fb12223     	sw	s11, 0x1e4(sp)
     db8: 12012d83     	lw	s11, 0x120(sp)
     dbc: 11c12083     	lw	ra, 0x11c(sp)
     dc0: 01b0edb3     	or	s11, ra, s11
     dc4: 1db12c23     	sw	s11, 0x1d8(sp)
     dc8: 11812d83     	lw	s11, 0x118(sp)
     dcc: 11412083     	lw	ra, 0x114(sp)
     dd0: 01b0edb3     	or	s11, ra, s11
     dd4: 1fb12a23     	sw	s11, 0x1f4(sp)
     dd8: 11012d83     	lw	s11, 0x110(sp)
     ddc: 10c12083     	lw	ra, 0x10c(sp)
     de0: 01b0edb3     	or	s11, ra, s11
     de4: 1fb12623     	sw	s11, 0x1ec(sp)
     de8: 01366633     	or	a2, a2, s3
     dec: 1ec12823     	sw	a2, 0x1f0(sp)
     df0: 00b765b3     	or	a1, a4, a1
     df4: 1eb12423     	sw	a1, 0x1e8(sp)
     df8: 34412583     	lw	a1, 0x344(sp)
     dfc: 34012983     	lw	s3, 0x340(sp)
     e00: 34c12d83     	lw	s11, 0x34c(sp)
     e04: 34812603     	lw	a2, 0x348(sp)
     e08: 00a5c533     	xor	a0, a1, a0
     e0c: 1ea12023     	sw	a0, 0x1e0(sp)
     e10: 00f9c533     	xor	a0, s3, a5
     e14: 1ca12a23     	sw	a0, 0x1d4(sp)
     e18: 00ddc533     	xor	a0, s11, a3
     e1c: 1ca12e23     	sw	a0, 0x1dc(sp)
     e20: 01064533     	xor	a0, a2, a6
     e24: 1ca12823     	sw	a0, 0x1d0(sp)
     e28: 35412683     	lw	a3, 0x354(sp)
     e2c: 35012783     	lw	a5, 0x350(sp)
     e30: 35c12983     	lw	s3, 0x35c(sp)
     e34: 35812d83     	lw	s11, 0x358(sp)
     e38: 0056c533     	xor	a0, a3, t0
     e3c: 1ca12223     	sw	a0, 0x1c4(sp)
     e40: 0067c533     	xor	a0, a5, t1
     e44: 1aa12a23     	sw	a0, 0x1b4(sp)
     e48: 0119c533     	xor	a0, s3, a7
     e4c: 1ca12023     	sw	a0, 0x1c0(sp)
     e50: 007dc533     	xor	a0, s11, t2
     e54: 1aa12823     	sw	a0, 0x1b0(sp)
     e58: 36412283     	lw	t0, 0x364(sp)
     e5c: 36012303     	lw	t1, 0x360(sp)
     e60: 36c12983     	lw	s3, 0x36c(sp)
     e64: 36812d83     	lw	s11, 0x368(sp)
     e68: 01d2c533     	xor	a0, t0, t4
     e6c: 1aa12223     	sw	a0, 0x1a4(sp)
     e70: 01f343b3     	xor	t2, t1, t6
     e74: 01c9c333     	xor	t1, s3, t3
     e78: 01edcdb3     	xor	s11, s11, t5
     e7c: 37012e03     	lw	t3, 0x370(sp)
     e80: 37412e83     	lw	t4, 0x374(sp)
     e84: 37c12f03     	lw	t5, 0x37c(sp)
     e88: 37812f83     	lw	t6, 0x378(sp)
     e8c: 009e4e33     	xor	t3, t3, s1
     e90: 016eceb3     	xor	t4, t4, s6
     e94: 012f4f33     	xor	t5, t5, s2
     e98: 015fcfb3     	xor	t6, t6, s5
     e9c: 38412483     	lw	s1, 0x384(sp)
     ea0: 38012903     	lw	s2, 0x380(sp)
     ea4: 38812983     	lw	s3, 0x388(sp)
     ea8: 38c12a83     	lw	s5, 0x38c(sp)
     eac: 0174c4b3     	xor	s1, s1, s7
     eb0: 01994933     	xor	s2, s2, s9
     eb4: 0189c9b3     	xor	s3, s3, s8
     eb8: 01aacab3     	xor	s5, s5, s10
     ebc: 39012b03     	lw	s6, 0x390(sp)
     ec0: 39412b83     	lw	s7, 0x394(sp)
     ec4: 39812c03     	lw	s8, 0x398(sp)
     ec8: 39c12c83     	lw	s9, 0x39c(sp)
     ecc: 1a012503     	lw	a0, 0x1a0(sp)
     ed0: 00ab4b33     	xor	s6, s6, a0
     ed4: 1ac12503     	lw	a0, 0x1ac(sp)
     ed8: 00abcbb3     	xor	s7, s7, a0
     edc: 19c12503     	lw	a0, 0x19c(sp)
     ee0: 00ac4c33     	xor	s8, s8, a0
     ee4: 1a812503     	lw	a0, 0x1a8(sp)
     ee8: 00acccb3     	xor	s9, s9, a0
     eec: 3a012d03     	lw	s10, 0x3a0(sp)
     ef0: 3a412083     	lw	ra, 0x3a4(sp)
     ef4: 3a812503     	lw	a0, 0x3a8(sp)
     ef8: 3ac12583     	lw	a1, 0x3ac(sp)
     efc: 1bc12603     	lw	a2, 0x1bc(sp)
     f00: 00cd4d33     	xor	s10, s10, a2
     f04: 1cc12603     	lw	a2, 0x1cc(sp)
     f08: 00c0c0b3     	xor	ra, ra, a2
     f0c: 1b812603     	lw	a2, 0x1b8(sp)
     f10: 00c542b3     	xor	t0, a0, a2
     f14: 1c812503     	lw	a0, 0x1c8(sp)
     f18: 00a5c8b3     	xor	a7, a1, a0
     f1c: 3b012503     	lw	a0, 0x3b0(sp)
     f20: 3b412583     	lw	a1, 0x3b4(sp)
     f24: 3b812603     	lw	a2, 0x3b8(sp)
     f28: 3bc12683     	lw	a3, 0x3bc(sp)
     f2c: 1d812783     	lw	a5, 0x1d8(sp)
     f30: 00f547b3     	xor	a5, a0, a5
     f34: 1e412503     	lw	a0, 0x1e4(sp)
     f38: 00a5c833     	xor	a6, a1, a0
     f3c: 3c012583     	lw	a1, 0x3c0(sp)
     f40: 3c412503     	lw	a0, 0x3c4(sp)
     f44: 1ec12703     	lw	a4, 0x1ec(sp)
     f48: 00e64633     	xor	a2, a2, a4
     f4c: 1f412703     	lw	a4, 0x1f4(sp)
     f50: 00e6c6b3     	xor	a3, a3, a4
     f54: 1e812703     	lw	a4, 0x1e8(sp)
     f58: 00e5c733     	xor	a4, a1, a4
     f5c: 1f012583     	lw	a1, 0x1f0(sp)
     f60: 00b54533     	xor	a0, a0, a1
     f64: 1d412583     	lw	a1, 0x1d4(sp)
     f68: 34b12023     	sw	a1, 0x340(sp)
     f6c: 1e012583     	lw	a1, 0x1e0(sp)
     f70: 34b12223     	sw	a1, 0x344(sp)
     f74: 1d012583     	lw	a1, 0x1d0(sp)
     f78: 34b12423     	sw	a1, 0x348(sp)
     f7c: 1dc12583     	lw	a1, 0x1dc(sp)
     f80: 34b12623     	sw	a1, 0x34c(sp)
     f84: 1b412583     	lw	a1, 0x1b4(sp)
     f88: 34b12823     	sw	a1, 0x350(sp)
     f8c: 1c412583     	lw	a1, 0x1c4(sp)
     f90: 34b12a23     	sw	a1, 0x354(sp)
     f94: 1b012583     	lw	a1, 0x1b0(sp)
     f98: 34b12c23     	sw	a1, 0x358(sp)
     f9c: 1c012583     	lw	a1, 0x1c0(sp)
     fa0: 34b12e23     	sw	a1, 0x35c(sp)
     fa4: 36712023     	sw	t2, 0x360(sp)
     fa8: 1a412583     	lw	a1, 0x1a4(sp)
     fac: 36b12223     	sw	a1, 0x364(sp)
     fb0: 37b12423     	sw	s11, 0x368(sp)
     fb4: 36612623     	sw	t1, 0x36c(sp)
     fb8: 37c12823     	sw	t3, 0x370(sp)
     fbc: 37d12a23     	sw	t4, 0x374(sp)
     fc0: 37f12c23     	sw	t6, 0x378(sp)
     fc4: 37e12e23     	sw	t5, 0x37c(sp)
     fc8: 39212023     	sw	s2, 0x380(sp)
     fcc: 38912223     	sw	s1, 0x384(sp)
     fd0: 1f812483     	lw	s1, 0x1f8(sp)
     fd4: 39312423     	sw	s3, 0x388(sp)
     fd8: 39512623     	sw	s5, 0x38c(sp)
     fdc: 39612823     	sw	s6, 0x390(sp)
     fe0: 08800b13     	li	s6, 0x88
     fe4: 39712a23     	sw	s7, 0x394(sp)
     fe8: 20412b83     	lw	s7, 0x204(sp)
     fec: 39812c23     	sw	s8, 0x398(sp)
     ff0: 39912e23     	sw	s9, 0x39c(sp)
     ff4: 3ba12023     	sw	s10, 0x3a0(sp)
     ff8: 3a112223     	sw	ra, 0x3a4(sp)
     ffc: 3a512423     	sw	t0, 0x3a8(sp)
    1000: 3b112623     	sw	a7, 0x3ac(sp)
    1004: 40812583     	lw	a1, 0x408(sp)
    1008: 3af12823     	sw	a5, 0x3b0(sp)
    100c: 3b012a23     	sw	a6, 0x3b4(sp)
    1010: 3ac12c23     	sw	a2, 0x3b8(sp)
    1014: 3ad12e23     	sw	a3, 0x3bc(sp)
    1018: 3ce12023     	sw	a4, 0x3c0(sp)
    101c: 3ca12223     	sw	a0, 0x3c4(sp)
    1020: 34010513     	addi	a0, sp, 0x340
    1024: 428040ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
    1028: 0884b513     	sltiu	a0, s1, 0x88
    102c: fff50513     	addi	a0, a0, -0x1
    1030: f7857a93     	andi	s5, a0, -0x88
    1034: 01548ab3     	add	s5, s1, s5
    1038: 009a0a33     	add	s4, s4, s1
    103c: 415a05b3     	sub	a1, s4, s5
    1040: 41010513     	addi	a0, sp, 0x410
    1044: 000a8613     	mv	a2, s5
    1048: 728060ef     	jal	0x7770 <memcpy>
    104c: 5f814483     	lbu	s1, 0x5f8(sp)
    1050: 409b0633     	sub	a2, s6, s1
    1054: 49510c23     	sb	s5, 0x498(sp)
    1058: aacbe263     	bltu	s7, a2, 0x2fc <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x134>
    105c: 480488e3     	beqz	s1, 0x1cec <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1b24>
    1060: 57010513     	addi	a0, sp, 0x570
    1064: 00950533     	add	a0, a0, s1
    1068: 7ff10593     	addi	a1, sp, 0x7ff
    106c: 2fa58593     	addi	a1, a1, 0x2fa
    1070: 1ec12c23     	sw	a2, 0x1f8(sp)
    1074: 6fc060ef     	jal	0x7770 <memcpy>
    1078: 57012903     	lw	s2, 0x570(sp)
    107c: 57412883     	lw	a7, 0x574(sp)
    1080: 57812603     	lw	a2, 0x578(sp)
    1084: 4a012a03     	lw	s4, 0x4a0(sp)
    1088: 4a412283     	lw	t0, 0x4a4(sp)
    108c: 4a812783     	lw	a5, 0x4a8(sp)
    1090: 4ac12503     	lw	a0, 0x4ac(sp)
    1094: 57c12803     	lw	a6, 0x57c(sp)
    1098: 58012703     	lw	a4, 0x580(sp)
    109c: 58412383     	lw	t2, 0x584(sp)
    10a0: 58812e03     	lw	t3, 0x588(sp)
    10a4: 4b012e83     	lw	t4, 0x4b0(sp)
    10a8: 4b412f03     	lw	t5, 0x4b4(sp)
    10ac: 4b812f83     	lw	t6, 0x4b8(sp)
    10b0: 4bc12483     	lw	s1, 0x4bc(sp)
    10b4: 58c12b03     	lw	s6, 0x58c(sp)
    10b8: 59012303     	lw	t1, 0x590(sp)
    10bc: 59412983     	lw	s3, 0x594(sp)
    10c0: 59812a83     	lw	s5, 0x598(sp)
    10c4: 4c012683     	lw	a3, 0x4c0(sp)
    10c8: 4c412b83     	lw	s7, 0x4c4(sp)
    10cc: 4c812c03     	lw	s8, 0x4c8(sp)
    10d0: 4cc12c83     	lw	s9, 0x4cc(sp)
    10d4: 0112c5b3     	xor	a1, t0, a7
    10d8: 1eb12a23     	sw	a1, 0x1f4(sp)
    10dc: 59c12883     	lw	a7, 0x59c(sp)
    10e0: 5a012d83     	lw	s11, 0x5a0(sp)
    10e4: 5a412083     	lw	ra, 0x5a4(sp)
    10e8: 5a812583     	lw	a1, 0x5a8(sp)
    10ec: 012a42b3     	xor	t0, s4, s2
    10f0: 1e512223     	sw	t0, 0x1e4(sp)
    10f4: 01054533     	xor	a0, a0, a6
    10f8: 1ea12823     	sw	a0, 0x1f0(sp)
    10fc: 00c7c633     	xor	a2, a5, a2
    1100: 1ec12023     	sw	a2, 0x1e0(sp)
    1104: 007f4533     	xor	a0, t5, t2
    1108: 1ea12623     	sw	a0, 0x1ec(sp)
    110c: 4d012603     	lw	a2, 0x4d0(sp)
    1110: 4d412f03     	lw	t5, 0x4d4(sp)
    1114: 4d812503     	lw	a0, 0x4d8(sp)
    1118: 4dc12a03     	lw	s4, 0x4dc(sp)
    111c: 00eec733     	xor	a4, t4, a4
    1120: 1ce12c23     	sw	a4, 0x1d8(sp)
    1124: 0164c733     	xor	a4, s1, s6
    1128: 1ce12e23     	sw	a4, 0x1dc(sp)
    112c: 01cfc733     	xor	a4, t6, t3
    1130: 1ce12a23     	sw	a4, 0x1d4(sp)
    1134: 013bc733     	xor	a4, s7, s3
    1138: 1ee12423     	sw	a4, 0x1e8(sp)
    113c: 5ac12703     	lw	a4, 0x5ac(sp)
    1140: 5b012983     	lw	s3, 0x5b0(sp)
    1144: 5b412b83     	lw	s7, 0x5b4(sp)
    1148: 5b812383     	lw	t2, 0x5b8(sp)
    114c: 0066c4b3     	xor	s1, a3, t1
    1150: 011cc6b3     	xor	a3, s9, a7
    1154: 1cd12823     	sw	a3, 0x1d0(sp)
    1158: 015c4e33     	xor	t3, s8, s5
    115c: 01b64633     	xor	a2, a2, s11
    1160: 1cc12623     	sw	a2, 0x1cc(sp)
    1164: 4e012603     	lw	a2, 0x4e0(sp)
    1168: 4e412a83     	lw	s5, 0x4e4(sp)
    116c: 4e812c03     	lw	s8, 0x4e8(sp)
    1170: 4ec12c83     	lw	s9, 0x4ec(sp)
    1174: 001f4fb3     	xor	t6, t5, ra
    1178: 00ea4eb3     	xor	t4, s4, a4
    117c: 00b54333     	xor	t1, a0, a1
    1180: 017acf33     	xor	t5, s5, s7
    1184: 5bc12503     	lw	a0, 0x5bc(sp)
    1188: 5c012583     	lw	a1, 0x5c0(sp)
    118c: 5c412703     	lw	a4, 0x5c4(sp)
    1190: 5c812d83     	lw	s11, 0x5c8(sp)
    1194: 01364bb3     	xor	s7, a2, s3
    1198: 007c4ab3     	xor	s5, s8, t2
    119c: 00acca33     	xor	s4, s9, a0
    11a0: 4f012503     	lw	a0, 0x4f0(sp)
    11a4: 4f412603     	lw	a2, 0x4f4(sp)
    11a8: 4f812383     	lw	t2, 0x4f8(sp)
    11ac: 4fc12983     	lw	s3, 0x4fc(sp)
    11b0: 00b54cb3     	xor	s9, a0, a1
    11b4: 00e642b3     	xor	t0, a2, a4
    11b8: 01b3cdb3     	xor	s11, t2, s11
    11bc: 5cc12503     	lw	a0, 0x5cc(sp)
    11c0: 5d012583     	lw	a1, 0x5d0(sp)
    11c4: 5d412603     	lw	a2, 0x5d4(sp)
    11c8: 5d812703     	lw	a4, 0x5d8(sp)
    11cc: 00a9c0b3     	xor	ra, s3, a0
    11d0: 50012503     	lw	a0, 0x500(sp)
    11d4: 50412383     	lw	t2, 0x504(sp)
    11d8: 50812983     	lw	s3, 0x508(sp)
    11dc: 50c12683     	lw	a3, 0x50c(sp)
    11e0: 00b54c33     	xor	s8, a0, a1
    11e4: 00c3c8b3     	xor	a7, t2, a2
    11e8: 00e9c833     	xor	a6, s3, a4
    11ec: 5dc12583     	lw	a1, 0x5dc(sp)
    11f0: 5e012703     	lw	a4, 0x5e0(sp)
    11f4: 5e412383     	lw	t2, 0x5e4(sp)
    11f8: 5e812783     	lw	a5, 0x5e8(sp)
    11fc: 00b6c9b3     	xor	s3, a3, a1
    1200: 51012583     	lw	a1, 0x510(sp)
    1204: 51412683     	lw	a3, 0x514(sp)
    1208: 51812b03     	lw	s6, 0x518(sp)
    120c: 51c12903     	lw	s2, 0x51c(sp)
    1210: 00e5c733     	xor	a4, a1, a4
    1214: 0076c6b3     	xor	a3, a3, t2
    1218: 5ec12603     	lw	a2, 0x5ec(sp)
    121c: 5f012383     	lw	t2, 0x5f0(sp)
    1220: 5f412d03     	lw	s10, 0x5f4(sp)
    1224: 52012503     	lw	a0, 0x520(sp)
    1228: 52412583     	lw	a1, 0x524(sp)
    122c: 00fb47b3     	xor	a5, s6, a5
    1230: 08800b13     	li	s6, 0x88
    1234: 00c94633     	xor	a2, s2, a2
    1238: 00754533     	xor	a0, a0, t2
    123c: 01a5c3b3     	xor	t2, a1, s10
    1240: 1e412583     	lw	a1, 0x1e4(sp)
    1244: 4ab12023     	sw	a1, 0x4a0(sp)
    1248: 1f412583     	lw	a1, 0x1f4(sp)
    124c: 4ab12223     	sw	a1, 0x4a4(sp)
    1250: 1e012583     	lw	a1, 0x1e0(sp)
    1254: 4ab12423     	sw	a1, 0x4a8(sp)
    1258: 1f012583     	lw	a1, 0x1f0(sp)
    125c: 4ab12623     	sw	a1, 0x4ac(sp)
    1260: 1d812583     	lw	a1, 0x1d8(sp)
    1264: 4ab12823     	sw	a1, 0x4b0(sp)
    1268: 1ec12583     	lw	a1, 0x1ec(sp)
    126c: 4ab12a23     	sw	a1, 0x4b4(sp)
    1270: 1d412583     	lw	a1, 0x1d4(sp)
    1274: 4ab12c23     	sw	a1, 0x4b8(sp)
    1278: 1dc12583     	lw	a1, 0x1dc(sp)
    127c: 4ab12e23     	sw	a1, 0x4bc(sp)
    1280: 4c912023     	sw	s1, 0x4c0(sp)
    1284: 56812583     	lw	a1, 0x568(sp)
    1288: 1e812483     	lw	s1, 0x1e8(sp)
    128c: 4c912223     	sw	s1, 0x4c4(sp)
    1290: 4dc12423     	sw	t3, 0x4c8(sp)
    1294: 1d012e03     	lw	t3, 0x1d0(sp)
    1298: 4dc12623     	sw	t3, 0x4cc(sp)
    129c: 1cc12e03     	lw	t3, 0x1cc(sp)
    12a0: 4dc12823     	sw	t3, 0x4d0(sp)
    12a4: 4df12a23     	sw	t6, 0x4d4(sp)
    12a8: 4c612c23     	sw	t1, 0x4d8(sp)
    12ac: 4dd12e23     	sw	t4, 0x4dc(sp)
    12b0: 4f712023     	sw	s7, 0x4e0(sp)
    12b4: 20412b83     	lw	s7, 0x204(sp)
    12b8: 4fe12223     	sw	t5, 0x4e4(sp)
    12bc: 4f512423     	sw	s5, 0x4e8(sp)
    12c0: 4f412623     	sw	s4, 0x4ec(sp)
    12c4: 4f912823     	sw	s9, 0x4f0(sp)
    12c8: 4e512a23     	sw	t0, 0x4f4(sp)
    12cc: 4fb12c23     	sw	s11, 0x4f8(sp)
    12d0: 4e112e23     	sw	ra, 0x4fc(sp)
    12d4: 51812023     	sw	s8, 0x500(sp)
    12d8: 51112223     	sw	a7, 0x504(sp)
    12dc: 51012423     	sw	a6, 0x508(sp)
    12e0: 51312623     	sw	s3, 0x50c(sp)
    12e4: 50e12823     	sw	a4, 0x510(sp)
    12e8: 50d12a23     	sw	a3, 0x514(sp)
    12ec: 50f12c23     	sw	a5, 0x518(sp)
    12f0: 50c12e23     	sw	a2, 0x51c(sp)
    12f4: 7ff10a93     	addi	s5, sp, 0x7ff
    12f8: 2faa8a93     	addi	s5, s5, 0x2fa
    12fc: 1f812603     	lw	a2, 0x1f8(sp)
    1300: 00ca8ab3     	add	s5, s5, a2
    1304: 52a12023     	sw	a0, 0x520(sp)
    1308: 52712223     	sw	t2, 0x524(sp)
    130c: 40cb84b3     	sub	s1, s7, a2
    1310: 4a010513     	addi	a0, sp, 0x4a0
    1314: 138040ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
    1318: 1964ece3     	bltu	s1, s6, 0x1cb0 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1ae8>
    131c: 000acf83     	lbu	t6, 0x0(s5)
    1320: 001acc83     	lbu	s9, 0x1(s5)
    1324: 002ac583     	lbu	a1, 0x2(s5)
    1328: 003ac603     	lbu	a2, 0x3(s5)
    132c: 004ac503     	lbu	a0, 0x4(s5)
    1330: 005ac683     	lbu	a3, 0x5(s5)
    1334: 006ac283     	lbu	t0, 0x6(s5)
    1338: 007ace03     	lbu	t3, 0x7(s5)
    133c: 1e912c23     	sw	s1, 0x1f8(sp)
    1340: 008ac483     	lbu	s1, 0x8(s5)
    1344: 009ac903     	lbu	s2, 0x9(s5)
    1348: 00aac803     	lbu	a6, 0xa(s5)
    134c: 00bac303     	lbu	t1, 0xb(s5)
    1350: 00cac703     	lbu	a4, 0xc(s5)
    1354: 00dace83     	lbu	t4, 0xd(s5)
    1358: 00eaca03     	lbu	s4, 0xe(s5)
    135c: 00facb83     	lbu	s7, 0xf(s5)
    1360: 010acd03     	lbu	s10, 0x10(s5)
    1364: 011ac983     	lbu	s3, 0x11(s5)
    1368: 012ac883     	lbu	a7, 0x12(s5)
    136c: 013ac383     	lbu	t2, 0x13(s5)
    1370: 014ac783     	lbu	a5, 0x14(s5)
    1374: 015acf03     	lbu	t5, 0x15(s5)
    1378: 016acb03     	lbu	s6, 0x16(s5)
    137c: 017acc03     	lbu	s8, 0x17(s5)
    1380: 008c9c93     	slli	s9, s9, 0x8
    1384: 01fcefb3     	or	t6, s9, t6
    1388: 1ff12a23     	sw	t6, 0x1f4(sp)
    138c: 018acd83     	lbu	s11, 0x18(s5)
    1390: 019ac083     	lbu	ra, 0x19(s5)
    1394: 01aacf83     	lbu	t6, 0x1a(s5)
    1398: 01bacc83     	lbu	s9, 0x1b(s5)
    139c: 01059593     	slli	a1, a1, 0x10
    13a0: 01861613     	slli	a2, a2, 0x18
    13a4: 00869693     	slli	a3, a3, 0x8
    13a8: 01029293     	slli	t0, t0, 0x10
    13ac: 018e1e13     	slli	t3, t3, 0x18
    13b0: 00891913     	slli	s2, s2, 0x8
    13b4: 00b665b3     	or	a1, a2, a1
    13b8: 1eb12823     	sw	a1, 0x1f0(sp)
    13bc: 00a6e533     	or	a0, a3, a0
    13c0: 1ea12623     	sw	a0, 0x1ec(sp)
    13c4: 005e6533     	or	a0, t3, t0
    13c8: 1ea12423     	sw	a0, 0x1e8(sp)
    13cc: 00996533     	or	a0, s2, s1
    13d0: 1ea12223     	sw	a0, 0x1e4(sp)
    13d4: 01cac503     	lbu	a0, 0x1c(s5)
    13d8: 01dac583     	lbu	a1, 0x1d(s5)
    13dc: 01eac603     	lbu	a2, 0x1e(s5)
    13e0: 01fac683     	lbu	a3, 0x1f(s5)
    13e4: 01081813     	slli	a6, a6, 0x10
    13e8: 01831313     	slli	t1, t1, 0x18
    13ec: 008e9e93     	slli	t4, t4, 0x8
    13f0: 010a1a13     	slli	s4, s4, 0x10
    13f4: 018b9b93     	slli	s7, s7, 0x18
    13f8: 00899993     	slli	s3, s3, 0x8
    13fc: 01036833     	or	a6, t1, a6
    1400: 1f012023     	sw	a6, 0x1e0(sp)
    1404: 00eee733     	or	a4, t4, a4
    1408: 1ce12e23     	sw	a4, 0x1dc(sp)
    140c: 014bea33     	or	s4, s7, s4
    1410: 01a9ebb3     	or	s7, s3, s10
    1414: 020ac703     	lbu	a4, 0x20(s5)
    1418: 021ac803     	lbu	a6, 0x21(s5)
    141c: 022ac283     	lbu	t0, 0x22(s5)
    1420: 023ac303     	lbu	t1, 0x23(s5)
    1424: 01089893     	slli	a7, a7, 0x10
    1428: 01839393     	slli	t2, t2, 0x18
    142c: 008f1f13     	slli	t5, t5, 0x8
    1430: 010b1b13     	slli	s6, s6, 0x10
    1434: 018c1c13     	slli	s8, s8, 0x18
    1438: 00809093     	slli	ra, ra, 0x8
    143c: 0113ed33     	or	s10, t2, a7
    1440: 00ff67b3     	or	a5, t5, a5
    1444: 1cf12c23     	sw	a5, 0x1d8(sp)
    1448: 016c67b3     	or	a5, s8, s6
    144c: 1cf12a23     	sw	a5, 0x1d4(sp)
    1450: 01b0e7b3     	or	a5, ra, s11
    1454: 1cf12823     	sw	a5, 0x1d0(sp)
    1458: 024ac783     	lbu	a5, 0x24(s5)
    145c: 025ac883     	lbu	a7, 0x25(s5)
    1460: 026ac383     	lbu	t2, 0x26(s5)
    1464: 027ace03     	lbu	t3, 0x27(s5)
    1468: 010f9f93     	slli	t6, t6, 0x10
    146c: 018c9c93     	slli	s9, s9, 0x18
    1470: 00859593     	slli	a1, a1, 0x8
    1474: 01061613     	slli	a2, a2, 0x10
    1478: 01869693     	slli	a3, a3, 0x18
    147c: 00881813     	slli	a6, a6, 0x8
    1480: 01fcefb3     	or	t6, s9, t6
    1484: 00a5ecb3     	or	s9, a1, a0
    1488: 00c6e633     	or	a2, a3, a2
    148c: 1cc12623     	sw	a2, 0x1cc(sp)
    1490: 00e86533     	or	a0, a6, a4
    1494: 1ca12423     	sw	a0, 0x1c8(sp)
    1498: 028ac503     	lbu	a0, 0x28(s5)
    149c: 029ac583     	lbu	a1, 0x29(s5)
    14a0: 02aac603     	lbu	a2, 0x2a(s5)
    14a4: 02bac683     	lbu	a3, 0x2b(s5)
    14a8: 01029293     	slli	t0, t0, 0x10
    14ac: 01831313     	slli	t1, t1, 0x18
    14b0: 00889893     	slli	a7, a7, 0x8
    14b4: 01039393     	slli	t2, t2, 0x10
    14b8: 018e1e13     	slli	t3, t3, 0x18
    14bc: 00859593     	slli	a1, a1, 0x8
    14c0: 00536733     	or	a4, t1, t0
    14c4: 1ce12223     	sw	a4, 0x1c4(sp)
    14c8: 00f8e733     	or	a4, a7, a5
    14cc: 1ce12023     	sw	a4, 0x1c0(sp)
    14d0: 007e6733     	or	a4, t3, t2
    14d4: 1ae12e23     	sw	a4, 0x1bc(sp)
    14d8: 00a5e533     	or	a0, a1, a0
    14dc: 1aa12c23     	sw	a0, 0x1b8(sp)
    14e0: 02cac503     	lbu	a0, 0x2c(s5)
    14e4: 02dac583     	lbu	a1, 0x2d(s5)
    14e8: 02eac703     	lbu	a4, 0x2e(s5)
    14ec: 02fac783     	lbu	a5, 0x2f(s5)
    14f0: 01061613     	slli	a2, a2, 0x10
    14f4: 01869693     	slli	a3, a3, 0x18
    14f8: 00859593     	slli	a1, a1, 0x8
    14fc: 01071713     	slli	a4, a4, 0x10
    1500: 01879793     	slli	a5, a5, 0x18
    1504: 00c6e633     	or	a2, a3, a2
    1508: 1ac12a23     	sw	a2, 0x1b4(sp)
    150c: 00a5e533     	or	a0, a1, a0
    1510: 1aa12823     	sw	a0, 0x1b0(sp)
    1514: 00e7e733     	or	a4, a5, a4
    1518: 1ae12623     	sw	a4, 0x1ac(sp)
    151c: 035ac503     	lbu	a0, 0x35(s5)
    1520: 034ac583     	lbu	a1, 0x34(s5)
    1524: 036ac603     	lbu	a2, 0x36(s5)
    1528: 037ac683     	lbu	a3, 0x37(s5)
    152c: 00851513     	slli	a0, a0, 0x8
    1530: 00b56533     	or	a0, a0, a1
    1534: 1aa12423     	sw	a0, 0x1a8(sp)
    1538: 01061613     	slli	a2, a2, 0x10
    153c: 01869693     	slli	a3, a3, 0x18
    1540: 00c6e633     	or	a2, a3, a2
    1544: 1ac12223     	sw	a2, 0x1a4(sp)
    1548: 031ac503     	lbu	a0, 0x31(s5)
    154c: 030ac583     	lbu	a1, 0x30(s5)
    1550: 032ac603     	lbu	a2, 0x32(s5)
    1554: 033ac683     	lbu	a3, 0x33(s5)
    1558: 00851513     	slli	a0, a0, 0x8
    155c: 00b56533     	or	a0, a0, a1
    1560: 1aa12023     	sw	a0, 0x1a0(sp)
    1564: 01061613     	slli	a2, a2, 0x10
    1568: 01869693     	slli	a3, a3, 0x18
    156c: 00c6e633     	or	a2, a3, a2
    1570: 18c12e23     	sw	a2, 0x19c(sp)
    1574: 039ac503     	lbu	a0, 0x39(s5)
    1578: 038ac583     	lbu	a1, 0x38(s5)
    157c: 03aac603     	lbu	a2, 0x3a(s5)
    1580: 03bac683     	lbu	a3, 0x3b(s5)
    1584: 00851513     	slli	a0, a0, 0x8
    1588: 00b56533     	or	a0, a0, a1
    158c: 18a12c23     	sw	a0, 0x198(sp)
    1590: 01061613     	slli	a2, a2, 0x10
    1594: 01869693     	slli	a3, a3, 0x18
    1598: 00c6e633     	or	a2, a3, a2
    159c: 18c12a23     	sw	a2, 0x194(sp)
    15a0: 03dac503     	lbu	a0, 0x3d(s5)
    15a4: 03cac583     	lbu	a1, 0x3c(s5)
    15a8: 03eac603     	lbu	a2, 0x3e(s5)
    15ac: 03fac683     	lbu	a3, 0x3f(s5)
    15b0: 00851513     	slli	a0, a0, 0x8
    15b4: 00b56533     	or	a0, a0, a1
    15b8: 18a12823     	sw	a0, 0x190(sp)
    15bc: 01061613     	slli	a2, a2, 0x10
    15c0: 01869693     	slli	a3, a3, 0x18
    15c4: 00c6e633     	or	a2, a3, a2
    15c8: 18c12623     	sw	a2, 0x18c(sp)
    15cc: 041ac503     	lbu	a0, 0x41(s5)
    15d0: 040ac583     	lbu	a1, 0x40(s5)
    15d4: 042ac603     	lbu	a2, 0x42(s5)
    15d8: 043ac683     	lbu	a3, 0x43(s5)
    15dc: 00851513     	slli	a0, a0, 0x8
    15e0: 00b56533     	or	a0, a0, a1
    15e4: 18a12423     	sw	a0, 0x188(sp)
    15e8: 01061613     	slli	a2, a2, 0x10
    15ec: 01869693     	slli	a3, a3, 0x18
    15f0: 00c6e633     	or	a2, a3, a2
    15f4: 18c12223     	sw	a2, 0x184(sp)
    15f8: 045ac503     	lbu	a0, 0x45(s5)
    15fc: 044ac583     	lbu	a1, 0x44(s5)
    1600: 046ac603     	lbu	a2, 0x46(s5)
    1604: 047ac683     	lbu	a3, 0x47(s5)
    1608: 00851513     	slli	a0, a0, 0x8
    160c: 00b56533     	or	a0, a0, a1
    1610: 18a12023     	sw	a0, 0x180(sp)
    1614: 01061613     	slli	a2, a2, 0x10
    1618: 01869693     	slli	a3, a3, 0x18
    161c: 00c6e633     	or	a2, a3, a2
    1620: 16c12e23     	sw	a2, 0x17c(sp)
    1624: 04dac503     	lbu	a0, 0x4d(s5)
    1628: 04cac583     	lbu	a1, 0x4c(s5)
    162c: 04eac603     	lbu	a2, 0x4e(s5)
    1630: 04fac683     	lbu	a3, 0x4f(s5)
    1634: 00851513     	slli	a0, a0, 0x8
    1638: 00b56533     	or	a0, a0, a1
    163c: 16a12c23     	sw	a0, 0x178(sp)
    1640: 01061613     	slli	a2, a2, 0x10
    1644: 01869693     	slli	a3, a3, 0x18
    1648: 00c6e633     	or	a2, a3, a2
    164c: 16c12a23     	sw	a2, 0x174(sp)
    1650: 049ac503     	lbu	a0, 0x49(s5)
    1654: 048ac583     	lbu	a1, 0x48(s5)
    1658: 04aac603     	lbu	a2, 0x4a(s5)
    165c: 04bac683     	lbu	a3, 0x4b(s5)
    1660: 00851513     	slli	a0, a0, 0x8
    1664: 00b56533     	or	a0, a0, a1
    1668: 16a12823     	sw	a0, 0x170(sp)
    166c: 01061613     	slli	a2, a2, 0x10
    1670: 01869693     	slli	a3, a3, 0x18
    1674: 00c6e633     	or	a2, a3, a2
    1678: 16c12623     	sw	a2, 0x16c(sp)
    167c: 055ac503     	lbu	a0, 0x55(s5)
    1680: 054ac583     	lbu	a1, 0x54(s5)
    1684: 056ac603     	lbu	a2, 0x56(s5)
    1688: 057ac683     	lbu	a3, 0x57(s5)
    168c: 00851513     	slli	a0, a0, 0x8
    1690: 00b56533     	or	a0, a0, a1
    1694: 16a12423     	sw	a0, 0x168(sp)
    1698: 01061613     	slli	a2, a2, 0x10
    169c: 01869693     	slli	a3, a3, 0x18
    16a0: 00c6e633     	or	a2, a3, a2
    16a4: 16c12223     	sw	a2, 0x164(sp)
    16a8: 051ac503     	lbu	a0, 0x51(s5)
    16ac: 050ac583     	lbu	a1, 0x50(s5)
    16b0: 052ac603     	lbu	a2, 0x52(s5)
    16b4: 053ac683     	lbu	a3, 0x53(s5)
    16b8: 00851513     	slli	a0, a0, 0x8
    16bc: 00b56533     	or	a0, a0, a1
    16c0: 16a12023     	sw	a0, 0x160(sp)
    16c4: 01061613     	slli	a2, a2, 0x10
    16c8: 01869693     	slli	a3, a3, 0x18
    16cc: 00c6e633     	or	a2, a3, a2
    16d0: 14c12e23     	sw	a2, 0x15c(sp)
    16d4: 05dac503     	lbu	a0, 0x5d(s5)
    16d8: 05cac583     	lbu	a1, 0x5c(s5)
    16dc: 05eac703     	lbu	a4, 0x5e(s5)
    16e0: 05fac783     	lbu	a5, 0x5f(s5)
    16e4: 00851513     	slli	a0, a0, 0x8
    16e8: 00b56533     	or	a0, a0, a1
    16ec: 14a12c23     	sw	a0, 0x158(sp)
    16f0: 01071713     	slli	a4, a4, 0x10
    16f4: 01879793     	slli	a5, a5, 0x18
    16f8: 00e7e733     	or	a4, a5, a4
    16fc: 14e12a23     	sw	a4, 0x154(sp)
    1700: 059ac503     	lbu	a0, 0x59(s5)
    1704: 058ac583     	lbu	a1, 0x58(s5)
    1708: 05aac703     	lbu	a4, 0x5a(s5)
    170c: 05bac783     	lbu	a5, 0x5b(s5)
    1710: 00851513     	slli	a0, a0, 0x8
    1714: 00b56533     	or	a0, a0, a1
    1718: 14a12823     	sw	a0, 0x150(sp)
    171c: 01071713     	slli	a4, a4, 0x10
    1720: 01879793     	slli	a5, a5, 0x18
    1724: 00e7e733     	or	a4, a5, a4
    1728: 14e12623     	sw	a4, 0x14c(sp)
    172c: 065ac503     	lbu	a0, 0x65(s5)
    1730: 064ac583     	lbu	a1, 0x64(s5)
    1734: 066ac703     	lbu	a4, 0x66(s5)
    1738: 067ac783     	lbu	a5, 0x67(s5)
    173c: 00851513     	slli	a0, a0, 0x8
    1740: 00b56533     	or	a0, a0, a1
    1744: 14a12423     	sw	a0, 0x148(sp)
    1748: 01071713     	slli	a4, a4, 0x10
    174c: 01879793     	slli	a5, a5, 0x18
    1750: 00e7e733     	or	a4, a5, a4
    1754: 14e12223     	sw	a4, 0x144(sp)
    1758: 061ac503     	lbu	a0, 0x61(s5)
    175c: 060ac583     	lbu	a1, 0x60(s5)
    1760: 062ac703     	lbu	a4, 0x62(s5)
    1764: 063ac783     	lbu	a5, 0x63(s5)
    1768: 00851513     	slli	a0, a0, 0x8
    176c: 00b56533     	or	a0, a0, a1
    1770: 14a12023     	sw	a0, 0x140(sp)
    1774: 01071713     	slli	a4, a4, 0x10
    1778: 01879793     	slli	a5, a5, 0x18
    177c: 00e7e733     	or	a4, a5, a4
    1780: 12e12e23     	sw	a4, 0x13c(sp)
    1784: 06dac503     	lbu	a0, 0x6d(s5)
    1788: 06cac583     	lbu	a1, 0x6c(s5)
    178c: 06eace83     	lbu	t4, 0x6e(s5)
    1790: 06facf03     	lbu	t5, 0x6f(s5)
    1794: 00851513     	slli	a0, a0, 0x8
    1798: 00b56533     	or	a0, a0, a1
    179c: 12a12c23     	sw	a0, 0x138(sp)
    17a0: 010e9e93     	slli	t4, t4, 0x10
    17a4: 018f1f13     	slli	t5, t5, 0x18
    17a8: 01df6533     	or	a0, t5, t4
    17ac: 12a12a23     	sw	a0, 0x134(sp)
    17b0: 069ac503     	lbu	a0, 0x69(s5)
    17b4: 068ac583     	lbu	a1, 0x68(s5)
    17b8: 06aac483     	lbu	s1, 0x6a(s5)
    17bc: 06bac903     	lbu	s2, 0x6b(s5)
    17c0: 00851513     	slli	a0, a0, 0x8
    17c4: 00b56533     	or	a0, a0, a1
    17c8: 12a12823     	sw	a0, 0x130(sp)
    17cc: 01049493     	slli	s1, s1, 0x10
    17d0: 01891913     	slli	s2, s2, 0x18
    17d4: 00996533     	or	a0, s2, s1
    17d8: 12a12623     	sw	a0, 0x12c(sp)
    17dc: 075ac503     	lbu	a0, 0x75(s5)
    17e0: 074ac583     	lbu	a1, 0x74(s5)
    17e4: 076ac903     	lbu	s2, 0x76(s5)
    17e8: 077acb03     	lbu	s6, 0x77(s5)
    17ec: 00851513     	slli	a0, a0, 0x8
    17f0: 00b56533     	or	a0, a0, a1
    17f4: 12a12423     	sw	a0, 0x128(sp)
    17f8: 01091913     	slli	s2, s2, 0x10
    17fc: 018b1b13     	slli	s6, s6, 0x18
    1800: 012b6533     	or	a0, s6, s2
    1804: 12a12223     	sw	a0, 0x124(sp)
    1808: 071ac503     	lbu	a0, 0x71(s5)
    180c: 070ac583     	lbu	a1, 0x70(s5)
    1810: 072ac903     	lbu	s2, 0x72(s5)
    1814: 073acb03     	lbu	s6, 0x73(s5)
    1818: 00851513     	slli	a0, a0, 0x8
    181c: 00b56533     	or	a0, a0, a1
    1820: 12a12023     	sw	a0, 0x120(sp)
    1824: 01091913     	slli	s2, s2, 0x10
    1828: 018b1b13     	slli	s6, s6, 0x18
    182c: 012b6533     	or	a0, s6, s2
    1830: 10a12e23     	sw	a0, 0x11c(sp)
    1834: 07dac503     	lbu	a0, 0x7d(s5)
    1838: 07cac583     	lbu	a1, 0x7c(s5)
    183c: 07eac903     	lbu	s2, 0x7e(s5)
    1840: 07facb03     	lbu	s6, 0x7f(s5)
    1844: 00851513     	slli	a0, a0, 0x8
    1848: 00b56533     	or	a0, a0, a1
    184c: 10a12c23     	sw	a0, 0x118(sp)
    1850: 01091913     	slli	s2, s2, 0x10
    1854: 018b1b13     	slli	s6, s6, 0x18
    1858: 012b6533     	or	a0, s6, s2
    185c: 10a12a23     	sw	a0, 0x114(sp)
    1860: 079acc03     	lbu	s8, 0x79(s5)
    1864: 078ac583     	lbu	a1, 0x78(s5)
    1868: 07aac903     	lbu	s2, 0x7a(s5)
    186c: 07bac503     	lbu	a0, 0x7b(s5)
    1870: 008c1c13     	slli	s8, s8, 0x8
    1874: 00bc65b3     	or	a1, s8, a1
    1878: 10b12823     	sw	a1, 0x110(sp)
    187c: 01091913     	slli	s2, s2, 0x10
    1880: 01851513     	slli	a0, a0, 0x18
    1884: 01256533     	or	a0, a0, s2
    1888: 10a12623     	sw	a0, 0x10c(sp)
    188c: 085ac603     	lbu	a2, 0x85(s5)
    1890: 084ac683     	lbu	a3, 0x84(s5)
    1894: 086ac583     	lbu	a1, 0x86(s5)
    1898: 087ac503     	lbu	a0, 0x87(s5)
    189c: 00861613     	slli	a2, a2, 0x8
    18a0: 00d669b3     	or	s3, a2, a3
    18a4: 01059593     	slli	a1, a1, 0x10
    18a8: 01851513     	slli	a0, a0, 0x18
    18ac: 00b56633     	or	a2, a0, a1
    18b0: 081ac583     	lbu	a1, 0x81(s5)
    18b4: 080ac683     	lbu	a3, 0x80(s5)
    18b8: 082ac703     	lbu	a4, 0x82(s5)
    18bc: 083ac783     	lbu	a5, 0x83(s5)
    18c0: 00859593     	slli	a1, a1, 0x8
    18c4: 00d5e5b3     	or	a1, a1, a3
    18c8: 01071713     	slli	a4, a4, 0x10
    18cc: 01879793     	slli	a5, a5, 0x18
    18d0: 00e7e733     	or	a4, a5, a4
    18d4: 1f412503     	lw	a0, 0x1f4(sp)
    18d8: 1f012783     	lw	a5, 0x1f0(sp)
    18dc: 00a7e7b3     	or	a5, a5, a0
    18e0: 1ec12503     	lw	a0, 0x1ec(sp)
    18e4: 1e812683     	lw	a3, 0x1e8(sp)
    18e8: 00a6e533     	or	a0, a3, a0
    18ec: 1e412683     	lw	a3, 0x1e4(sp)
    18f0: 1e012803     	lw	a6, 0x1e0(sp)
    18f4: 00d86833     	or	a6, a6, a3
    18f8: 1dc12683     	lw	a3, 0x1dc(sp)
    18fc: 00da66b3     	or	a3, s4, a3
    1900: 017d6333     	or	t1, s10, s7
    1904: 1d812883     	lw	a7, 0x1d8(sp)
    1908: 1d412283     	lw	t0, 0x1d4(sp)
    190c: 0112e2b3     	or	t0, t0, a7
    1910: 1d012883     	lw	a7, 0x1d0(sp)
    1914: 011fe3b3     	or	t2, t6, a7
    1918: 1cc12883     	lw	a7, 0x1cc(sp)
    191c: 0198e8b3     	or	a7, a7, s9
    1920: 1c812e03     	lw	t3, 0x1c8(sp)
    1924: 1c412e83     	lw	t4, 0x1c4(sp)
    1928: 01ceefb3     	or	t6, t4, t3
    192c: 1c012e03     	lw	t3, 0x1c0(sp)
    1930: 1bc12e83     	lw	t4, 0x1bc(sp)
    1934: 01ceeeb3     	or	t4, t4, t3
    1938: 1b812e03     	lw	t3, 0x1b8(sp)
    193c: 1b412f03     	lw	t5, 0x1b4(sp)
    1940: 01cf6f33     	or	t5, t5, t3
    1944: 1b012e03     	lw	t3, 0x1b0(sp)
    1948: 1ac12483     	lw	s1, 0x1ac(sp)
    194c: 01c4ee33     	or	t3, s1, t3
    1950: 1a812483     	lw	s1, 0x1a8(sp)
    1954: 1a412903     	lw	s2, 0x1a4(sp)
    1958: 00996b33     	or	s6, s2, s1
    195c: 1a012483     	lw	s1, 0x1a0(sp)
    1960: 19c12903     	lw	s2, 0x19c(sp)
    1964: 009964b3     	or	s1, s2, s1
    1968: 19812903     	lw	s2, 0x198(sp)
    196c: 19412a03     	lw	s4, 0x194(sp)
    1970: 012a6a33     	or	s4, s4, s2
    1974: 19012903     	lw	s2, 0x190(sp)
    1978: 18c12b83     	lw	s7, 0x18c(sp)
    197c: 012be933     	or	s2, s7, s2
    1980: 18812b83     	lw	s7, 0x188(sp)
    1984: 18412c03     	lw	s8, 0x184(sp)
    1988: 017c6cb3     	or	s9, s8, s7
    198c: 18012b83     	lw	s7, 0x180(sp)
    1990: 17c12c03     	lw	s8, 0x17c(sp)
    1994: 017c6bb3     	or	s7, s8, s7
    1998: 17812c03     	lw	s8, 0x178(sp)
    199c: 17412d03     	lw	s10, 0x174(sp)
    19a0: 018d6d33     	or	s10, s10, s8
    19a4: 17012c03     	lw	s8, 0x170(sp)
    19a8: 16c12083     	lw	ra, 0x16c(sp)
    19ac: 0180ec33     	or	s8, ra, s8
    19b0: 16812083     	lw	ra, 0x168(sp)
    19b4: 16412d83     	lw	s11, 0x164(sp)
    19b8: 001dedb3     	or	s11, s11, ra
    19bc: 1bb12623     	sw	s11, 0x1ac(sp)
    19c0: 16012d83     	lw	s11, 0x160(sp)
    19c4: 15c12083     	lw	ra, 0x15c(sp)
    19c8: 01b0edb3     	or	s11, ra, s11
    19cc: 1bb12023     	sw	s11, 0x1a0(sp)
    19d0: 15812d83     	lw	s11, 0x158(sp)
    19d4: 15412083     	lw	ra, 0x154(sp)
    19d8: 01b0edb3     	or	s11, ra, s11
    19dc: 1bb12423     	sw	s11, 0x1a8(sp)
    19e0: 15012d83     	lw	s11, 0x150(sp)
    19e4: 14c12083     	lw	ra, 0x14c(sp)
    19e8: 01b0edb3     	or	s11, ra, s11
    19ec: 19b12e23     	sw	s11, 0x19c(sp)
    19f0: 14812d83     	lw	s11, 0x148(sp)
    19f4: 14412083     	lw	ra, 0x144(sp)
    19f8: 01b0edb3     	or	s11, ra, s11
    19fc: 1db12623     	sw	s11, 0x1cc(sp)
    1a00: 14012d83     	lw	s11, 0x140(sp)
    1a04: 13c12083     	lw	ra, 0x13c(sp)
    1a08: 01b0edb3     	or	s11, ra, s11
    1a0c: 1bb12e23     	sw	s11, 0x1bc(sp)
    1a10: 13812d83     	lw	s11, 0x138(sp)
    1a14: 13412083     	lw	ra, 0x134(sp)
    1a18: 01b0edb3     	or	s11, ra, s11
    1a1c: 1db12423     	sw	s11, 0x1c8(sp)
    1a20: 13012d83     	lw	s11, 0x130(sp)
    1a24: 12c12083     	lw	ra, 0x12c(sp)
    1a28: 01b0edb3     	or	s11, ra, s11
    1a2c: 1bb12c23     	sw	s11, 0x1b8(sp)
    1a30: 12812d83     	lw	s11, 0x128(sp)
    1a34: 12412083     	lw	ra, 0x124(sp)
    1a38: 01b0edb3     	or	s11, ra, s11
    1a3c: 1fb12223     	sw	s11, 0x1e4(sp)
    1a40: 12012d83     	lw	s11, 0x120(sp)
    1a44: 11c12083     	lw	ra, 0x11c(sp)
    1a48: 01b0edb3     	or	s11, ra, s11
    1a4c: 1db12c23     	sw	s11, 0x1d8(sp)
    1a50: 11812d83     	lw	s11, 0x118(sp)
    1a54: 11412083     	lw	ra, 0x114(sp)
    1a58: 01b0edb3     	or	s11, ra, s11
    1a5c: 1fb12a23     	sw	s11, 0x1f4(sp)
    1a60: 11012d83     	lw	s11, 0x110(sp)
    1a64: 10c12083     	lw	ra, 0x10c(sp)
    1a68: 01b0edb3     	or	s11, ra, s11
    1a6c: 1fb12623     	sw	s11, 0x1ec(sp)
    1a70: 01366633     	or	a2, a2, s3
    1a74: 1ec12823     	sw	a2, 0x1f0(sp)
    1a78: 00b765b3     	or	a1, a4, a1
    1a7c: 1eb12423     	sw	a1, 0x1e8(sp)
    1a80: 4a412583     	lw	a1, 0x4a4(sp)
    1a84: 4a012983     	lw	s3, 0x4a0(sp)
    1a88: 4ac12d83     	lw	s11, 0x4ac(sp)
    1a8c: 4a812603     	lw	a2, 0x4a8(sp)
    1a90: 00a5c533     	xor	a0, a1, a0
    1a94: 1ea12023     	sw	a0, 0x1e0(sp)
    1a98: 00f9c533     	xor	a0, s3, a5
    1a9c: 1ca12a23     	sw	a0, 0x1d4(sp)
    1aa0: 00ddc533     	xor	a0, s11, a3
    1aa4: 1ca12e23     	sw	a0, 0x1dc(sp)
    1aa8: 01064533     	xor	a0, a2, a6
    1aac: 1ca12823     	sw	a0, 0x1d0(sp)
    1ab0: 4b412683     	lw	a3, 0x4b4(sp)
    1ab4: 4b012783     	lw	a5, 0x4b0(sp)
    1ab8: 4bc12983     	lw	s3, 0x4bc(sp)
    1abc: 4b812d83     	lw	s11, 0x4b8(sp)
    1ac0: 0056c533     	xor	a0, a3, t0
    1ac4: 1ca12223     	sw	a0, 0x1c4(sp)
    1ac8: 0067c533     	xor	a0, a5, t1
    1acc: 1aa12a23     	sw	a0, 0x1b4(sp)
    1ad0: 0119c533     	xor	a0, s3, a7
    1ad4: 1ca12023     	sw	a0, 0x1c0(sp)
    1ad8: 007dc533     	xor	a0, s11, t2
    1adc: 1aa12823     	sw	a0, 0x1b0(sp)
    1ae0: 4c412283     	lw	t0, 0x4c4(sp)
    1ae4: 4c012303     	lw	t1, 0x4c0(sp)
    1ae8: 4cc12983     	lw	s3, 0x4cc(sp)
    1aec: 4c812d83     	lw	s11, 0x4c8(sp)
    1af0: 01d2c533     	xor	a0, t0, t4
    1af4: 1aa12223     	sw	a0, 0x1a4(sp)
    1af8: 01f343b3     	xor	t2, t1, t6
    1afc: 01c9c333     	xor	t1, s3, t3
    1b00: 01edcdb3     	xor	s11, s11, t5
    1b04: 4d012e03     	lw	t3, 0x4d0(sp)
    1b08: 4d412e83     	lw	t4, 0x4d4(sp)
    1b0c: 4dc12f03     	lw	t5, 0x4dc(sp)
    1b10: 4d812f83     	lw	t6, 0x4d8(sp)
    1b14: 009e4e33     	xor	t3, t3, s1
    1b18: 016eceb3     	xor	t4, t4, s6
    1b1c: 012f4f33     	xor	t5, t5, s2
    1b20: 014fcfb3     	xor	t6, t6, s4
    1b24: 4e412483     	lw	s1, 0x4e4(sp)
    1b28: 4e012903     	lw	s2, 0x4e0(sp)
    1b2c: 4e812983     	lw	s3, 0x4e8(sp)
    1b30: 4ec12a03     	lw	s4, 0x4ec(sp)
    1b34: 0174c4b3     	xor	s1, s1, s7
    1b38: 01994933     	xor	s2, s2, s9
    1b3c: 0189c9b3     	xor	s3, s3, s8
    1b40: 01aa4a33     	xor	s4, s4, s10
    1b44: 4f012b03     	lw	s6, 0x4f0(sp)
    1b48: 4f412b83     	lw	s7, 0x4f4(sp)
    1b4c: 4f812c03     	lw	s8, 0x4f8(sp)
    1b50: 4fc12c83     	lw	s9, 0x4fc(sp)
    1b54: 1a012503     	lw	a0, 0x1a0(sp)
    1b58: 00ab4b33     	xor	s6, s6, a0
    1b5c: 1ac12503     	lw	a0, 0x1ac(sp)
    1b60: 00abcbb3     	xor	s7, s7, a0
    1b64: 19c12503     	lw	a0, 0x19c(sp)
    1b68: 00ac4c33     	xor	s8, s8, a0
    1b6c: 1a812503     	lw	a0, 0x1a8(sp)
    1b70: 00acccb3     	xor	s9, s9, a0
    1b74: 50012d03     	lw	s10, 0x500(sp)
    1b78: 50412083     	lw	ra, 0x504(sp)
    1b7c: 50812503     	lw	a0, 0x508(sp)
    1b80: 50c12583     	lw	a1, 0x50c(sp)
    1b84: 1bc12603     	lw	a2, 0x1bc(sp)
    1b88: 00cd4d33     	xor	s10, s10, a2
    1b8c: 1cc12603     	lw	a2, 0x1cc(sp)
    1b90: 00c0c0b3     	xor	ra, ra, a2
    1b94: 1b812603     	lw	a2, 0x1b8(sp)
    1b98: 00c542b3     	xor	t0, a0, a2
    1b9c: 1c812503     	lw	a0, 0x1c8(sp)
    1ba0: 00a5c8b3     	xor	a7, a1, a0
    1ba4: 51012503     	lw	a0, 0x510(sp)
    1ba8: 51412583     	lw	a1, 0x514(sp)
    1bac: 51812603     	lw	a2, 0x518(sp)
    1bb0: 51c12683     	lw	a3, 0x51c(sp)
    1bb4: 1d812783     	lw	a5, 0x1d8(sp)
    1bb8: 00f547b3     	xor	a5, a0, a5
    1bbc: 1e412503     	lw	a0, 0x1e4(sp)
    1bc0: 00a5c833     	xor	a6, a1, a0
    1bc4: 52012583     	lw	a1, 0x520(sp)
    1bc8: 52412503     	lw	a0, 0x524(sp)
    1bcc: 1ec12703     	lw	a4, 0x1ec(sp)
    1bd0: 00e64633     	xor	a2, a2, a4
    1bd4: 1f412703     	lw	a4, 0x1f4(sp)
    1bd8: 00e6c6b3     	xor	a3, a3, a4
    1bdc: 1e812703     	lw	a4, 0x1e8(sp)
    1be0: 00e5c733     	xor	a4, a1, a4
    1be4: 1f012583     	lw	a1, 0x1f0(sp)
    1be8: 00b54533     	xor	a0, a0, a1
    1bec: 1d412583     	lw	a1, 0x1d4(sp)
    1bf0: 4ab12023     	sw	a1, 0x4a0(sp)
    1bf4: 1e012583     	lw	a1, 0x1e0(sp)
    1bf8: 4ab12223     	sw	a1, 0x4a4(sp)
    1bfc: 1d012583     	lw	a1, 0x1d0(sp)
    1c00: 4ab12423     	sw	a1, 0x4a8(sp)
    1c04: 1dc12583     	lw	a1, 0x1dc(sp)
    1c08: 4ab12623     	sw	a1, 0x4ac(sp)
    1c0c: 1b412583     	lw	a1, 0x1b4(sp)
    1c10: 4ab12823     	sw	a1, 0x4b0(sp)
    1c14: 1c412583     	lw	a1, 0x1c4(sp)
    1c18: 4ab12a23     	sw	a1, 0x4b4(sp)
    1c1c: 1b012583     	lw	a1, 0x1b0(sp)
    1c20: 4ab12c23     	sw	a1, 0x4b8(sp)
    1c24: 1c012583     	lw	a1, 0x1c0(sp)
    1c28: 4ab12e23     	sw	a1, 0x4bc(sp)
    1c2c: 4c712023     	sw	t2, 0x4c0(sp)
    1c30: 1a412583     	lw	a1, 0x1a4(sp)
    1c34: 4cb12223     	sw	a1, 0x4c4(sp)
    1c38: 4db12423     	sw	s11, 0x4c8(sp)
    1c3c: 4c612623     	sw	t1, 0x4cc(sp)
    1c40: 4dc12823     	sw	t3, 0x4d0(sp)
    1c44: 4dd12a23     	sw	t4, 0x4d4(sp)
    1c48: 4df12c23     	sw	t6, 0x4d8(sp)
    1c4c: 4de12e23     	sw	t5, 0x4dc(sp)
    1c50: 4f212023     	sw	s2, 0x4e0(sp)
    1c54: 4e912223     	sw	s1, 0x4e4(sp)
    1c58: 1f812483     	lw	s1, 0x1f8(sp)
    1c5c: 4f312423     	sw	s3, 0x4e8(sp)
    1c60: 4f412623     	sw	s4, 0x4ec(sp)
    1c64: 4f612823     	sw	s6, 0x4f0(sp)
    1c68: 08800b13     	li	s6, 0x88
    1c6c: 4f712a23     	sw	s7, 0x4f4(sp)
    1c70: 20412b83     	lw	s7, 0x204(sp)
    1c74: 4f812c23     	sw	s8, 0x4f8(sp)
    1c78: 4f912e23     	sw	s9, 0x4fc(sp)
    1c7c: 51a12023     	sw	s10, 0x500(sp)
    1c80: 50112223     	sw	ra, 0x504(sp)
    1c84: 50512423     	sw	t0, 0x508(sp)
    1c88: 51112623     	sw	a7, 0x50c(sp)
    1c8c: 56812583     	lw	a1, 0x568(sp)
    1c90: 50f12823     	sw	a5, 0x510(sp)
    1c94: 51012a23     	sw	a6, 0x514(sp)
    1c98: 50c12c23     	sw	a2, 0x518(sp)
    1c9c: 50d12e23     	sw	a3, 0x51c(sp)
    1ca0: 52e12023     	sw	a4, 0x520(sp)
    1ca4: 52a12223     	sw	a0, 0x524(sp)
    1ca8: 4a010513     	addi	a0, sp, 0x4a0
    1cac: 7a0030ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
    1cb0: 0884b513     	sltiu	a0, s1, 0x88
    1cb4: fff50513     	addi	a0, a0, -0x1
    1cb8: f7857a13     	andi	s4, a0, -0x88
    1cbc: 01448a33     	add	s4, s1, s4
    1cc0: 009a8ab3     	add	s5, s5, s1
    1cc4: 414a85b3     	sub	a1, s5, s4
    1cc8: 57010513     	addi	a0, sp, 0x570
    1ccc: 000a0613     	mv	a2, s4
    1cd0: 2a1050ef     	jal	0x7770 <memcpy>
    1cd4: e44fe06f     	j	0x318 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x150>
    1cd8: 7ff10a13     	addi	s4, sp, 0x7ff
    1cdc: 2faa0a13     	addi	s4, s4, 0x2fa
    1ce0: 000b8493     	mv	s1, s7
    1ce4: b56be263     	bltu	s7, s6, 0x1028 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0xe60>
    1ce8: 9adfe06f     	j	0x694 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x4cc>
    1cec: 7ff10a93     	addi	s5, sp, 0x7ff
    1cf0: 2faa8a93     	addi	s5, s5, 0x2fa
    1cf4: 000b8493     	mv	s1, s7
    1cf8: e36bf263     	bgeu	s7, s6, 0x131c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1154>
    1cfc: fb5ff06f     	j	0x1cb0 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x1ae8>
    1d00: 49814503     	lbu	a0, 0x498(sp)
    1d04: 41010993     	addi	s3, sp, 0x410
    1d08: 00a989b3     	add	s3, s3, a0
    1d0c: 40ab0633     	sub	a2, s6, a0
    1d10: 00098513     	mv	a0, s3
    1d14: 00000593     	li	a1, 0x0
    1d18: 269050ef     	jal	0x7780 <memset>
    1d1c: 48010c23     	sb	zero, 0x498(sp)
    1d20: 00100513     	li	a0, 0x1
    1d24: 00a98023     	sb	a0, 0x0(s3)
    1d28: 34012503     	lw	a0, 0x340(sp)
    1d2c: 34412583     	lw	a1, 0x344(sp)
    1d30: 34812603     	lw	a2, 0x348(sp)
    1d34: 34c12683     	lw	a3, 0x34c(sp)
    1d38: 35012703     	lw	a4, 0x350(sp)
    1d3c: 35412783     	lw	a5, 0x354(sp)
    1d40: 35812803     	lw	a6, 0x358(sp)
    1d44: 35c12283     	lw	t0, 0x35c(sp)
    1d48: 36012303     	lw	t1, 0x360(sp)
    1d4c: 36412383     	lw	t2, 0x364(sp)
    1d50: 36812903     	lw	s2, 0x368(sp)
    1d54: 36c12f03     	lw	t5, 0x36c(sp)
    1d58: 37012a83     	lw	s5, 0x370(sp)
    1d5c: 37412b03     	lw	s6, 0x374(sp)
    1d60: 37812c83     	lw	s9, 0x378(sp)
    1d64: 37c12d03     	lw	s10, 0x37c(sp)
    1d68: 38012a03     	lw	s4, 0x380(sp)
    1d6c: 38412983     	lw	s3, 0x384(sp)
    1d70: 38812c03     	lw	s8, 0x388(sp)
    1d74: 38c12b83     	lw	s7, 0x38c(sp)
    1d78: 41012883     	lw	a7, 0x410(sp)
    1d7c: 41412e03     	lw	t3, 0x414(sp)
    1d80: 41812e83     	lw	t4, 0x418(sp)
    1d84: 41c12f83     	lw	t6, 0x41c(sp)
    1d88: 42012483     	lw	s1, 0x420(sp)
    1d8c: 42412d83     	lw	s11, 0x424(sp)
    1d90: 42812083     	lw	ra, 0x428(sp)
    1d94: 01c5c5b3     	xor	a1, a1, t3
    1d98: 1eb12623     	sw	a1, 0x1ec(sp)
    1d9c: 01154e33     	xor	t3, a0, a7
    1da0: 01f6c533     	xor	a0, a3, t6
    1da4: 1ea12423     	sw	a0, 0x1e8(sp)
    1da8: 01d64eb3     	xor	t4, a2, t4
    1dac: 42c12503     	lw	a0, 0x42c(sp)
    1db0: 43012603     	lw	a2, 0x430(sp)
    1db4: 43412683     	lw	a3, 0x434(sp)
    1db8: 43812583     	lw	a1, 0x438(sp)
    1dbc: 01b7cfb3     	xor	t6, a5, s11
    1dc0: 009744b3     	xor	s1, a4, s1
    1dc4: 00a2c533     	xor	a0, t0, a0
    1dc8: 20a12223     	sw	a0, 0x204(sp)
    1dcc: 00184533     	xor	a0, a6, ra
    1dd0: 20a12023     	sw	a0, 0x200(sp)
    1dd4: 43c12783     	lw	a5, 0x43c(sp)
    1dd8: 44012d83     	lw	s11, 0x440(sp)
    1ddc: 44412283     	lw	t0, 0x444(sp)
    1de0: 44812083     	lw	ra, 0x448(sp)
    1de4: 00d3c533     	xor	a0, t2, a3
    1de8: 1ea12e23     	sw	a0, 0x1fc(sp)
    1dec: 00c34533     	xor	a0, t1, a2
    1df0: 1ea12823     	sw	a0, 0x1f0(sp)
    1df4: 00b94533     	xor	a0, s2, a1
    1df8: 1ea12c23     	sw	a0, 0x1f8(sp)
    1dfc: 00ff4533     	xor	a0, t5, a5
    1e00: 1ea12a23     	sw	a0, 0x1f4(sp)
    1e04: 44c12583     	lw	a1, 0x44c(sp)
    1e08: 45012603     	lw	a2, 0x450(sp)
    1e0c: 45412503     	lw	a0, 0x454(sp)
    1e10: 45812683     	lw	a3, 0x458(sp)
    1e14: 005b4733     	xor	a4, s6, t0
    1e18: 1ee12223     	sw	a4, 0x1e4(sp)
    1e1c: 01bacf33     	xor	t5, s5, s11
    1e20: 001cc333     	xor	t1, s9, ra
    1e24: 00bd43b3     	xor	t2, s10, a1
    1e28: 45c12583     	lw	a1, 0x45c(sp)
    1e2c: 46012b03     	lw	s6, 0x460(sp)
    1e30: 46412c83     	lw	s9, 0x464(sp)
    1e34: 46812d03     	lw	s10, 0x468(sp)
    1e38: 00ca4933     	xor	s2, s4, a2
    1e3c: 00a9c9b3     	xor	s3, s3, a0
    1e40: 00dc4a33     	xor	s4, s8, a3
    1e44: 00bbcab3     	xor	s5, s7, a1
    1e48: 39012503     	lw	a0, 0x390(sp)
    1e4c: 39412583     	lw	a1, 0x394(sp)
    1e50: 39812603     	lw	a2, 0x398(sp)
    1e54: 39c12683     	lw	a3, 0x39c(sp)
    1e58: 01654b33     	xor	s6, a0, s6
    1e5c: 0195cbb3     	xor	s7, a1, s9
    1e60: 01a64c33     	xor	s8, a2, s10
    1e64: 46c12503     	lw	a0, 0x46c(sp)
    1e68: 47012583     	lw	a1, 0x470(sp)
    1e6c: 47412603     	lw	a2, 0x474(sp)
    1e70: 47812083     	lw	ra, 0x478(sp)
    1e74: 00a6ccb3     	xor	s9, a3, a0
    1e78: 3a012503     	lw	a0, 0x3a0(sp)
    1e7c: 3a412683     	lw	a3, 0x3a4(sp)
    1e80: 3a812d03     	lw	s10, 0x3a8(sp)
    1e84: 3ac12803     	lw	a6, 0x3ac(sp)
    1e88: 00b542b3     	xor	t0, a0, a1
    1e8c: 00c6cdb3     	xor	s11, a3, a2
    1e90: 001d40b3     	xor	ra, s10, ra
    1e94: 47c12583     	lw	a1, 0x47c(sp)
    1e98: 48012703     	lw	a4, 0x480(sp)
    1e9c: 48412783     	lw	a5, 0x484(sp)
    1ea0: 48812d03     	lw	s10, 0x488(sp)
    1ea4: 00b848b3     	xor	a7, a6, a1
    1ea8: 3b012683     	lw	a3, 0x3b0(sp)
    1eac: 3b412603     	lw	a2, 0x3b4(sp)
    1eb0: 3b812583     	lw	a1, 0x3b8(sp)
    1eb4: 3bc12803     	lw	a6, 0x3bc(sp)
    1eb8: 49714503     	lbu	a0, 0x497(sp)
    1ebc: 00e6c6b3     	xor	a3, a3, a4
    1ec0: 00f64633     	xor	a2, a2, a5
    1ec4: 01a5c733     	xor	a4, a1, s10
    1ec8: 08056513     	ori	a0, a0, 0x80
    1ecc: 48a10ba3     	sb	a0, 0x497(sp)
    1ed0: 35c12023     	sw	t3, 0x340(sp)
    1ed4: 1ec12503     	lw	a0, 0x1ec(sp)
    1ed8: 34a12223     	sw	a0, 0x344(sp)
    1edc: 35d12423     	sw	t4, 0x348(sp)
    1ee0: 1e812503     	lw	a0, 0x1e8(sp)
    1ee4: 34a12623     	sw	a0, 0x34c(sp)
    1ee8: 34912823     	sw	s1, 0x350(sp)
    1eec: 35f12a23     	sw	t6, 0x354(sp)
    1ef0: 40812583     	lw	a1, 0x408(sp)
    1ef4: 20012503     	lw	a0, 0x200(sp)
    1ef8: 34a12c23     	sw	a0, 0x358(sp)
    1efc: 20412503     	lw	a0, 0x204(sp)
    1f00: 34a12e23     	sw	a0, 0x35c(sp)
    1f04: 1f012503     	lw	a0, 0x1f0(sp)
    1f08: 36a12023     	sw	a0, 0x360(sp)
    1f0c: 1fc12503     	lw	a0, 0x1fc(sp)
    1f10: 36a12223     	sw	a0, 0x364(sp)
    1f14: 1f812503     	lw	a0, 0x1f8(sp)
    1f18: 36a12423     	sw	a0, 0x368(sp)
    1f1c: 1f412503     	lw	a0, 0x1f4(sp)
    1f20: 36a12623     	sw	a0, 0x36c(sp)
    1f24: 37e12823     	sw	t5, 0x370(sp)
    1f28: 1e412503     	lw	a0, 0x1e4(sp)
    1f2c: 36a12a23     	sw	a0, 0x374(sp)
    1f30: 36612c23     	sw	t1, 0x378(sp)
    1f34: 36712e23     	sw	t2, 0x37c(sp)
    1f38: 39212023     	sw	s2, 0x380(sp)
    1f3c: 39312223     	sw	s3, 0x384(sp)
    1f40: 39412423     	sw	s4, 0x388(sp)
    1f44: 39512623     	sw	s5, 0x38c(sp)
    1f48: 3c012503     	lw	a0, 0x3c0(sp)
    1f4c: 39612823     	sw	s6, 0x390(sp)
    1f50: 39712a23     	sw	s7, 0x394(sp)
    1f54: 39812c23     	sw	s8, 0x398(sp)
    1f58: 39912e23     	sw	s9, 0x39c(sp)
    1f5c: 48c12783     	lw	a5, 0x48c(sp)
    1f60: 3a512023     	sw	t0, 0x3a0(sp)
    1f64: 3bb12223     	sw	s11, 0x3a4(sp)
    1f68: 3a112423     	sw	ra, 0x3a8(sp)
    1f6c: 3b112623     	sw	a7, 0x3ac(sp)
    1f70: 49012883     	lw	a7, 0x490(sp)
    1f74: 49412283     	lw	t0, 0x494(sp)
    1f78: 00f847b3     	xor	a5, a6, a5
    1f7c: 3c412803     	lw	a6, 0x3c4(sp)
    1f80: 01154533     	xor	a0, a0, a7
    1f84: 3ad12823     	sw	a3, 0x3b0(sp)
    1f88: 3ac12a23     	sw	a2, 0x3b4(sp)
    1f8c: 3ae12c23     	sw	a4, 0x3b8(sp)
    1f90: 3af12e23     	sw	a5, 0x3bc(sp)
    1f94: 00584633     	xor	a2, a6, t0
    1f98: 3ca12023     	sw	a0, 0x3c0(sp)
    1f9c: 3cc12223     	sw	a2, 0x3c4(sp)
    1fa0: 34010513     	addi	a0, sp, 0x340
    1fa4: 4a8030ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
    1fa8: 35012503     	lw	a0, 0x350(sp)
    1fac: 35412583     	lw	a1, 0x354(sp)
    1fb0: 35812603     	lw	a2, 0x358(sp)
    1fb4: 35c12683     	lw	a3, 0x35c(sp)
    1fb8: 7ff10493     	addi	s1, sp, 0x7ff
    1fbc: 41548493     	addi	s1, s1, 0x415
    1fc0: fea4aa23     	sw	a0, -0xc(s1)
    1fc4: feb4ac23     	sw	a1, -0x8(s1)
    1fc8: fec4ae23     	sw	a2, -0x4(s1)
    1fcc: 00d4a023     	sw	a3, 0x0(s1)
    1fd0: 34012503     	lw	a0, 0x340(sp)
    1fd4: 34412583     	lw	a1, 0x344(sp)
    1fd8: 34812603     	lw	a2, 0x348(sp)
    1fdc: 34c12683     	lw	a3, 0x34c(sp)
    1fe0: fea4a223     	sw	a0, -0x1c(s1)
    1fe4: feb4a423     	sw	a1, -0x18(s1)
    1fe8: fec4a623     	sw	a2, -0x14(s1)
    1fec: fed4a823     	sw	a3, -0x10(s1)
    1ff0: 34010513     	addi	a0, sp, 0x340
    1ff4: 7ff10593     	addi	a1, sp, 0x7ff
    1ff8: 43958593     	addi	a1, a1, 0x439
    1ffc: 0c800613     	li	a2, 0xc8
    2000: 770050ef     	jal	0x7770 <memcpy>
    2004: 70012583     	lw	a1, 0x700(sp)
    2008: 01800513     	li	a0, 0x18
    200c: 40a12423     	sw	a0, 0x408(sp)
    2010: 48010c23     	sb	zero, 0x498(sp)
    2014: 0035d513     	srli	a0, a1, 0x3
    2018: 0f700613     	li	a2, 0xf7
    201c: 5cb66863     	bltu	a2, a1, 0x25ec <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x2424>
    2020: 00359593     	slli	a1, a1, 0x3
    2024: 00351513     	slli	a0, a0, 0x3
    2028: 0385f613     	andi	a2, a1, 0x38
    202c: 00100713     	li	a4, 0x1
    2030: 00b715b3     	sll	a1, a4, a1
    2034: 60010693     	addi	a3, sp, 0x600
    2038: 00a68533     	add	a0, a3, a0
    203c: fe060693     	addi	a3, a2, -0x20
    2040: 00c71633     	sll	a2, a4, a2
    2044: 41f6d713     	srai	a4, a3, 0x1f
    2048: 00b775b3     	and	a1, a4, a1
    204c: 00052703     	lw	a4, 0x0(a0)
    2050: 00452783     	lw	a5, 0x4(a0)
    2054: 0006a693     	slti	a3, a3, 0x0
    2058: fff68693     	addi	a3, a3, -0x1
    205c: 00c6f633     	and	a2, a3, a2
    2060: 00c7c633     	xor	a2, a5, a2
    2064: 00b745b3     	xor	a1, a4, a1
    2068: 00b52023     	sw	a1, 0x0(a0)
    206c: 00c52223     	sw	a2, 0x4(a0)
    2070: 68412503     	lw	a0, 0x684(sp)
    2074: 800005b7     	lui	a1, 0x80000
    2078: 00b54533     	xor	a0, a0, a1
    207c: 68a12223     	sw	a0, 0x684(sp)
    2080: 60010513     	addi	a0, sp, 0x600
    2084: 17d020ef     	jal	0x4a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
    2088: 61012503     	lw	a0, 0x610(sp)
    208c: 61412583     	lw	a1, 0x614(sp)
    2090: 61812603     	lw	a2, 0x618(sp)
    2094: 61c12683     	lw	a3, 0x61c(sp)
    2098: 00a4aa23     	sw	a0, 0x14(s1)
    209c: 00b4ac23     	sw	a1, 0x18(s1)
    20a0: 00c4ae23     	sw	a2, 0x1c(s1)
    20a4: 02d4a023     	sw	a3, 0x20(s1)
    20a8: 60012503     	lw	a0, 0x600(sp)
    20ac: 60412583     	lw	a1, 0x604(sp)
    20b0: 60812603     	lw	a2, 0x608(sp)
    20b4: 60c12683     	lw	a3, 0x60c(sp)
    20b8: 00a4a223     	sw	a0, 0x4(s1)
    20bc: 00b4a423     	sw	a1, 0x8(s1)
    20c0: 00c4a623     	sw	a2, 0xc(s1)
    20c4: 00d4a823     	sw	a3, 0x10(s1)
    20c8: 60010513     	addi	a0, sp, 0x600
    20cc: 0f800613     	li	a2, 0xf8
    20d0: 00000593     	li	a1, 0x0
    20d4: 6ac050ef     	jal	0x7780 <memset>
    20d8: 70012023     	sw	zero, 0x700(sp)
    20dc: 7ff10513     	addi	a0, sp, 0x7ff
    20e0: 3f950513     	addi	a0, a0, 0x3f9
    20e4: 7ff10593     	addi	a1, sp, 0x7ff
    20e8: 41958593     	addi	a1, a1, 0x419
    20ec: 02000613     	li	a2, 0x20
    20f0: 650050ef     	jal	0x7740 <memcmp>
    20f4: 4c051463     	bnez	a0, 0x25bc <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x23f4>
    20f8: 5f814503     	lbu	a0, 0x5f8(sp)
    20fc: 57010993     	addi	s3, sp, 0x570
    2100: 00a989b3     	add	s3, s3, a0
    2104: 08800613     	li	a2, 0x88
    2108: 40a60633     	sub	a2, a2, a0
    210c: 00098513     	mv	a0, s3
    2110: 00000593     	li	a1, 0x0
    2114: 66c050ef     	jal	0x7780 <memset>
    2118: 5e010c23     	sb	zero, 0x5f8(sp)
    211c: 00600513     	li	a0, 0x6
    2120: 00a98023     	sb	a0, 0x0(s3)
    2124: 4a012503     	lw	a0, 0x4a0(sp)
    2128: 4a412583     	lw	a1, 0x4a4(sp)
    212c: 4a812603     	lw	a2, 0x4a8(sp)
    2130: 4ac12683     	lw	a3, 0x4ac(sp)
    2134: 4b012703     	lw	a4, 0x4b0(sp)
    2138: 4b412783     	lw	a5, 0x4b4(sp)
    213c: 4b812803     	lw	a6, 0x4b8(sp)
    2140: 4bc12283     	lw	t0, 0x4bc(sp)
    2144: 4c012303     	lw	t1, 0x4c0(sp)
    2148: 4c412383     	lw	t2, 0x4c4(sp)
    214c: 4c812903     	lw	s2, 0x4c8(sp)
    2150: 4cc12f03     	lw	t5, 0x4cc(sp)
    2154: 4d012a83     	lw	s5, 0x4d0(sp)
    2158: 4d412b03     	lw	s6, 0x4d4(sp)
    215c: 4d812c83     	lw	s9, 0x4d8(sp)
    2160: 4dc12d03     	lw	s10, 0x4dc(sp)
    2164: 4e012a03     	lw	s4, 0x4e0(sp)
    2168: 4e412983     	lw	s3, 0x4e4(sp)
    216c: 4e812c03     	lw	s8, 0x4e8(sp)
    2170: 4ec12b83     	lw	s7, 0x4ec(sp)
    2174: 57012883     	lw	a7, 0x570(sp)
    2178: 57412e03     	lw	t3, 0x574(sp)
    217c: 57812e83     	lw	t4, 0x578(sp)
    2180: 57c12f83     	lw	t6, 0x57c(sp)
    2184: 58012483     	lw	s1, 0x580(sp)
    2188: 58412d83     	lw	s11, 0x584(sp)
    218c: 58812083     	lw	ra, 0x588(sp)
    2190: 01c5c5b3     	xor	a1, a1, t3
    2194: 1eb12623     	sw	a1, 0x1ec(sp)
    2198: 01154e33     	xor	t3, a0, a7
    219c: 01f6c533     	xor	a0, a3, t6
    21a0: 1ea12423     	sw	a0, 0x1e8(sp)
    21a4: 01d64eb3     	xor	t4, a2, t4
    21a8: 58c12503     	lw	a0, 0x58c(sp)
    21ac: 59012603     	lw	a2, 0x590(sp)
    21b0: 59412683     	lw	a3, 0x594(sp)
    21b4: 59812583     	lw	a1, 0x598(sp)
    21b8: 01b7cfb3     	xor	t6, a5, s11
    21bc: 009744b3     	xor	s1, a4, s1
    21c0: 00a2c533     	xor	a0, t0, a0
    21c4: 20a12223     	sw	a0, 0x204(sp)
    21c8: 00184533     	xor	a0, a6, ra
    21cc: 20a12023     	sw	a0, 0x200(sp)
    21d0: 59c12783     	lw	a5, 0x59c(sp)
    21d4: 5a012d83     	lw	s11, 0x5a0(sp)
    21d8: 5a412283     	lw	t0, 0x5a4(sp)
    21dc: 5a812083     	lw	ra, 0x5a8(sp)
    21e0: 00d3c533     	xor	a0, t2, a3
    21e4: 1ea12e23     	sw	a0, 0x1fc(sp)
    21e8: 00c34533     	xor	a0, t1, a2
    21ec: 1ea12823     	sw	a0, 0x1f0(sp)
    21f0: 00b94533     	xor	a0, s2, a1
    21f4: 1ea12c23     	sw	a0, 0x1f8(sp)
    21f8: 00ff4533     	xor	a0, t5, a5
    21fc: 1ea12a23     	sw	a0, 0x1f4(sp)
    2200: 5ac12583     	lw	a1, 0x5ac(sp)
    2204: 5b012603     	lw	a2, 0x5b0(sp)
    2208: 5b412503     	lw	a0, 0x5b4(sp)
    220c: 5b812683     	lw	a3, 0x5b8(sp)
    2210: 005b4733     	xor	a4, s6, t0
    2214: 1ee12223     	sw	a4, 0x1e4(sp)
    2218: 01bacf33     	xor	t5, s5, s11
    221c: 001cc333     	xor	t1, s9, ra
    2220: 00bd43b3     	xor	t2, s10, a1
    2224: 5bc12583     	lw	a1, 0x5bc(sp)
    2228: 5c012b03     	lw	s6, 0x5c0(sp)
    222c: 5c412c83     	lw	s9, 0x5c4(sp)
    2230: 5c812d03     	lw	s10, 0x5c8(sp)
    2234: 00ca4933     	xor	s2, s4, a2
    2238: 00a9c9b3     	xor	s3, s3, a0
    223c: 00dc4a33     	xor	s4, s8, a3
    2240: 00bbcab3     	xor	s5, s7, a1
    2244: 4f012503     	lw	a0, 0x4f0(sp)
    2248: 4f412583     	lw	a1, 0x4f4(sp)
    224c: 4f812603     	lw	a2, 0x4f8(sp)
    2250: 4fc12683     	lw	a3, 0x4fc(sp)
    2254: 01654b33     	xor	s6, a0, s6
    2258: 0195cbb3     	xor	s7, a1, s9
    225c: 01a64c33     	xor	s8, a2, s10
    2260: 5cc12503     	lw	a0, 0x5cc(sp)
    2264: 5d012583     	lw	a1, 0x5d0(sp)
    2268: 5d412603     	lw	a2, 0x5d4(sp)
    226c: 5d812083     	lw	ra, 0x5d8(sp)
    2270: 00a6ccb3     	xor	s9, a3, a0
    2274: 50012503     	lw	a0, 0x500(sp)
    2278: 50412683     	lw	a3, 0x504(sp)
    227c: 50812d03     	lw	s10, 0x508(sp)
    2280: 50c12803     	lw	a6, 0x50c(sp)
    2284: 00b542b3     	xor	t0, a0, a1
    2288: 00c6cdb3     	xor	s11, a3, a2
    228c: 001d40b3     	xor	ra, s10, ra
    2290: 5dc12583     	lw	a1, 0x5dc(sp)
    2294: 5e012703     	lw	a4, 0x5e0(sp)
    2298: 5e412783     	lw	a5, 0x5e4(sp)
    229c: 5e812d03     	lw	s10, 0x5e8(sp)
    22a0: 00b848b3     	xor	a7, a6, a1
    22a4: 51012683     	lw	a3, 0x510(sp)
    22a8: 51412603     	lw	a2, 0x514(sp)
    22ac: 51812583     	lw	a1, 0x518(sp)
    22b0: 51c12803     	lw	a6, 0x51c(sp)
    22b4: 5f714503     	lbu	a0, 0x5f7(sp)
    22b8: 00e6c6b3     	xor	a3, a3, a4
    22bc: 00f64633     	xor	a2, a2, a5
    22c0: 01a5c733     	xor	a4, a1, s10
    22c4: 08056513     	ori	a0, a0, 0x80
    22c8: 5ea10ba3     	sb	a0, 0x5f7(sp)
    22cc: 4bc12023     	sw	t3, 0x4a0(sp)
    22d0: 1ec12503     	lw	a0, 0x1ec(sp)
    22d4: 4aa12223     	sw	a0, 0x4a4(sp)
    22d8: 4bd12423     	sw	t4, 0x4a8(sp)
    22dc: 1e812503     	lw	a0, 0x1e8(sp)
    22e0: 4aa12623     	sw	a0, 0x4ac(sp)
    22e4: 4a912823     	sw	s1, 0x4b0(sp)
    22e8: 7ff10493     	addi	s1, sp, 0x7ff
    22ec: 41548493     	addi	s1, s1, 0x415
    22f0: 4bf12a23     	sw	t6, 0x4b4(sp)
    22f4: 56812583     	lw	a1, 0x568(sp)
    22f8: 20012503     	lw	a0, 0x200(sp)
    22fc: 4aa12c23     	sw	a0, 0x4b8(sp)
    2300: 20412503     	lw	a0, 0x204(sp)
    2304: 4aa12e23     	sw	a0, 0x4bc(sp)
    2308: 1f012503     	lw	a0, 0x1f0(sp)
    230c: 4ca12023     	sw	a0, 0x4c0(sp)
    2310: 1fc12503     	lw	a0, 0x1fc(sp)
    2314: 4ca12223     	sw	a0, 0x4c4(sp)
    2318: 1f812503     	lw	a0, 0x1f8(sp)
    231c: 4ca12423     	sw	a0, 0x4c8(sp)
    2320: 1f412503     	lw	a0, 0x1f4(sp)
    2324: 4ca12623     	sw	a0, 0x4cc(sp)
    2328: 4de12823     	sw	t5, 0x4d0(sp)
    232c: 1e412503     	lw	a0, 0x1e4(sp)
    2330: 4ca12a23     	sw	a0, 0x4d4(sp)
    2334: 4c612c23     	sw	t1, 0x4d8(sp)
    2338: 4c712e23     	sw	t2, 0x4dc(sp)
    233c: 4f212023     	sw	s2, 0x4e0(sp)
    2340: 4f312223     	sw	s3, 0x4e4(sp)
    2344: 4f412423     	sw	s4, 0x4e8(sp)
    2348: 4f512623     	sw	s5, 0x4ec(sp)
    234c: 52012503     	lw	a0, 0x520(sp)
    2350: 4f612823     	sw	s6, 0x4f0(sp)
    2354: 4f712a23     	sw	s7, 0x4f4(sp)
    2358: 4f812c23     	sw	s8, 0x4f8(sp)
    235c: 4f912e23     	sw	s9, 0x4fc(sp)
    2360: 5ec12783     	lw	a5, 0x5ec(sp)
    2364: 50512023     	sw	t0, 0x500(sp)
    2368: 51b12223     	sw	s11, 0x504(sp)
    236c: 50112423     	sw	ra, 0x508(sp)
    2370: 51112623     	sw	a7, 0x50c(sp)
    2374: 5f012883     	lw	a7, 0x5f0(sp)
    2378: 5f412283     	lw	t0, 0x5f4(sp)
    237c: 00f847b3     	xor	a5, a6, a5
    2380: 52412803     	lw	a6, 0x524(sp)
    2384: 01154533     	xor	a0, a0, a7
    2388: 50d12823     	sw	a3, 0x510(sp)
    238c: 50c12a23     	sw	a2, 0x514(sp)
    2390: 50e12c23     	sw	a4, 0x518(sp)
    2394: 50f12e23     	sw	a5, 0x51c(sp)
    2398: 00584633     	xor	a2, a6, t0
    239c: 52a12023     	sw	a0, 0x520(sp)
    23a0: 52c12223     	sw	a2, 0x524(sp)
    23a4: 4a010513     	addi	a0, sp, 0x4a0
    23a8: 0a4030ef     	jal	0x544c <keccak::p1600::h1e78a6fe180ce099>
    23ac: 4b012503     	lw	a0, 0x4b0(sp)
    23b0: 4b412583     	lw	a1, 0x4b4(sp)
    23b4: 4b812603     	lw	a2, 0x4b8(sp)
    23b8: 4bc12683     	lw	a3, 0x4bc(sp)
    23bc: fea4aa23     	sw	a0, -0xc(s1)
    23c0: feb4ac23     	sw	a1, -0x8(s1)
    23c4: fec4ae23     	sw	a2, -0x4(s1)
    23c8: 00d4a023     	sw	a3, 0x0(s1)
    23cc: 4a012503     	lw	a0, 0x4a0(sp)
    23d0: 4a412583     	lw	a1, 0x4a4(sp)
    23d4: 4a812603     	lw	a2, 0x4a8(sp)
    23d8: 4ac12683     	lw	a3, 0x4ac(sp)
    23dc: fea4a223     	sw	a0, -0x1c(s1)
    23e0: feb4a423     	sw	a1, -0x18(s1)
    23e4: fec4a623     	sw	a2, -0x14(s1)
    23e8: fed4a823     	sw	a3, -0x10(s1)
    23ec: 4a010513     	addi	a0, sp, 0x4a0
    23f0: 7ff10593     	addi	a1, sp, 0x7ff
    23f4: 43958593     	addi	a1, a1, 0x439
    23f8: 0c800613     	li	a2, 0xc8
    23fc: 374050ef     	jal	0x7770 <memcpy>
    2400: 00001537     	lui	a0, 0x1
    2404: 00a10533     	add	a0, sp, a0
    2408: 90052683     	lw	a3, -0x700(a0)
    240c: 01800513     	li	a0, 0x18
    2410: 56a12423     	sw	a0, 0x568(sp)
    2414: 5e010c23     	sb	zero, 0x5f8(sp)
    2418: 0036d513     	srli	a0, a3, 0x3
    241c: 0f700593     	li	a1, 0xf7
    2420: 1cd5e663     	bltu	a1, a3, 0x25ec <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x2424>
    2424: 00369693     	slli	a3, a3, 0x3
    2428: 0386f613     	andi	a2, a3, 0x38
    242c: fe060593     	addi	a1, a2, -0x20
    2430: 10412903     	lw	s2, 0x104(sp)
    2434: 0005c663     	bltz	a1, 0x2440 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x2278>
    2438: 00c91633     	sll	a2, s2, a2
    243c: 00c0006f     	j	0x2448 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x2280>
    2440: 10012603     	lw	a2, 0x100(sp)
    2444: 00d61633     	sll	a2, a2, a3
    2448: 08800b13     	li	s6, 0x88
    244c: 00d916b3     	sll	a3, s2, a3
    2450: 00351513     	slli	a0, a0, 0x3
    2454: 7ff10713     	addi	a4, sp, 0x7ff
    2458: 00170713     	addi	a4, a4, 0x1
    245c: 00a70533     	add	a0, a4, a0
    2460: 00052703     	lw	a4, 0x0(a0)
    2464: 00452783     	lw	a5, 0x4(a0)
    2468: 41f5d593     	srai	a1, a1, 0x1f
    246c: 00d5f5b3     	and	a1, a1, a3
    2470: 00b745b3     	xor	a1, a4, a1
    2474: 00c7c633     	xor	a2, a5, a2
    2478: 00b52023     	sw	a1, 0x0(a0)
    247c: 00c52223     	sw	a2, 0x4(a0)
    2480: 00001537     	lui	a0, 0x1
    2484: 00a10533     	add	a0, sp, a0
    2488: 88452503     	lw	a0, -0x77c(a0)
    248c: 800005b7     	lui	a1, 0x80000
    2490: 00b54533     	xor	a0, a0, a1
    2494: 000015b7     	lui	a1, 0x1
    2498: 00b105b3     	add	a1, sp, a1
    249c: 88a5a223     	sw	a0, -0x77c(a1)
    24a0: 7ff10513     	addi	a0, sp, 0x7ff
    24a4: 00150513     	addi	a0, a0, 0x1
    24a8: 558020ef     	jal	0x4a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
    24ac: 00001537     	lui	a0, 0x1
    24b0: 00a10533     	add	a0, sp, a0
    24b4: 81052503     	lw	a0, -0x7f0(a0)
    24b8: 000015b7     	lui	a1, 0x1
    24bc: 00b105b3     	add	a1, sp, a1
    24c0: 8145a583     	lw	a1, -0x7ec(a1)
    24c4: 00001637     	lui	a2, 0x1
    24c8: 00c10633     	add	a2, sp, a2
    24cc: 81862603     	lw	a2, -0x7e8(a2)
    24d0: 000016b7     	lui	a3, 0x1
    24d4: 00d106b3     	add	a3, sp, a3
    24d8: 81c6a683     	lw	a3, -0x7e4(a3)
    24dc: 00a4aa23     	sw	a0, 0x14(s1)
    24e0: 00b4ac23     	sw	a1, 0x18(s1)
    24e4: 00c4ae23     	sw	a2, 0x1c(s1)
    24e8: 02d4a023     	sw	a3, 0x20(s1)
    24ec: 00001537     	lui	a0, 0x1
    24f0: 00a10533     	add	a0, sp, a0
    24f4: 80052503     	lw	a0, -0x800(a0)
    24f8: 000015b7     	lui	a1, 0x1
    24fc: 00b105b3     	add	a1, sp, a1
    2500: 8045a583     	lw	a1, -0x7fc(a1)
    2504: 00001637     	lui	a2, 0x1
    2508: 00c10633     	add	a2, sp, a2
    250c: 80862603     	lw	a2, -0x7f8(a2)
    2510: 000016b7     	lui	a3, 0x1
    2514: 00d106b3     	add	a3, sp, a3
    2518: 80c6a683     	lw	a3, -0x7f4(a3)
    251c: 00a4a223     	sw	a0, 0x4(s1)
    2520: 00b4a423     	sw	a1, 0x8(s1)
    2524: 00c4a623     	sw	a2, 0xc(s1)
    2528: 00d4a823     	sw	a3, 0x10(s1)
    252c: 7ff10513     	addi	a0, sp, 0x7ff
    2530: 00150513     	addi	a0, a0, 0x1
    2534: 0f800613     	li	a2, 0xf8
    2538: 00000593     	li	a1, 0x0
    253c: 244050ef     	jal	0x7780 <memset>
    2540: 00001537     	lui	a0, 0x1
    2544: 00a10533     	add	a0, sp, a0
    2548: 90052023     	sw	zero, -0x700(a0)
    254c: 7ff10513     	addi	a0, sp, 0x7ff
    2550: 3f950513     	addi	a0, a0, 0x3f9
    2554: 7ff10593     	addi	a1, sp, 0x7ff
    2558: 41958593     	addi	a1, a1, 0x419
    255c: 02000613     	li	a2, 0x20
    2560: 1e0050ef     	jal	0x7740 <memcmp>
    2564: 10812583     	lw	a1, 0x108(sp)
    2568: 06051663     	bnez	a0, 0x25d4 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x240c>
    256c: 00158513     	addi	a0, a1, 0x1
    2570: 00a00593     	li	a1, 0xa
    2574: 00b50463     	beq	a0, a1, 0x257c <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x23b4>
    2578: d61fd06f     	j	0x2d8 <crypto::sha3::delegated::tests::mini_digest_test::h19de073f1b024d53+0x110>
    257c: 81040113     	addi	sp, s0, -0x7f0
    2580: 7ec12083     	lw	ra, 0x7ec(sp)
    2584: 7e812403     	lw	s0, 0x7e8(sp)
    2588: 7e412483     	lw	s1, 0x7e4(sp)
    258c: 7e012903     	lw	s2, 0x7e0(sp)
    2590: 7dc12983     	lw	s3, 0x7dc(sp)
    2594: 7d812a03     	lw	s4, 0x7d8(sp)
    2598: 7d412a83     	lw	s5, 0x7d4(sp)
    259c: 7d012b03     	lw	s6, 0x7d0(sp)
    25a0: 7cc12b83     	lw	s7, 0x7cc(sp)
    25a4: 7c812c03     	lw	s8, 0x7c8(sp)
    25a8: 7c412c83     	lw	s9, 0x7c4(sp)
    25ac: 7c012d03     	lw	s10, 0x7c0(sp)
    25b0: 7bc12d83     	lw	s11, 0x7bc(sp)
    25b4: 7f010113     	addi	sp, sp, 0x7f0
    25b8: 00008067     	ret
    25bc: 04200537     	lui	a0, 0x4200
    25c0: 12f50513     	addi	a0, a0, 0x12f
    25c4: 04200637     	lui	a2, 0x4200
    25c8: 1a060613     	addi	a2, a2, 0x1a0
    25cc: 06f00593     	li	a1, 0x6f
    25d0: 599040ef     	jal	0x7368 <core::panicking::panic::ha1ed58f4f5473d93>
    25d4: 04200537     	lui	a0, 0x4200
    25d8: 1b050513     	addi	a0, a0, 0x1b0
    25dc: 04200637     	lui	a2, 0x4200
    25e0: 21860613     	addi	a2, a2, 0x218
    25e4: 06600593     	li	a1, 0x66
    25e8: 581040ef     	jal	0x7368 <core::panicking::panic::ha1ed58f4f5473d93>
    25ec: 04200637     	lui	a2, 0x4200
    25f0: 22860613     	addi	a2, a2, 0x228
    25f4: 01f00593     	li	a1, 0x1f
    25f8: 3b9040ef     	jal	0x71b0 <core::panicking::panic_bounds_check::hf0fbe51e842a70af>

000025fc <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7>:
    25fc: 10052603     	lw	a2, 0x100(a0)
    2600: 04000593     	li	a1, 0x40
    2604: 00b67463     	bgeu	a2, a1, 0x260c <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7+0x10>
    2608: 7800106f     	j	0x3d88 <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7+0x178c>
    260c: e5010113     	addi	sp, sp, -0x1b0
    2610: 1a112623     	sw	ra, 0x1ac(sp)
    2614: 1a812423     	sw	s0, 0x1a8(sp)
    2618: 1a912223     	sw	s1, 0x1a4(sp)
    261c: 1b212023     	sw	s2, 0x1a0(sp)
    2620: 19312e23     	sw	s3, 0x19c(sp)
    2624: 19412c23     	sw	s4, 0x198(sp)
    2628: 19512a23     	sw	s5, 0x194(sp)
    262c: 19612823     	sw	s6, 0x190(sp)
    2630: 19712623     	sw	s7, 0x18c(sp)
    2634: 19812423     	sw	s8, 0x188(sp)
    2638: 19912223     	sw	s9, 0x184(sp)
    263c: 19a12023     	sw	s10, 0x180(sp)
    2640: 17b12e23     	sw	s11, 0x17c(sp)
    2644: 1b010413     	addi	s0, sp, 0x1b0
    2648: 12852703     	lw	a4, 0x128(a0)
    264c: 12c52783     	lw	a5, 0x12c(a0)
    2650: 13052803     	lw	a6, 0x130(a0)
    2654: 13452883     	lw	a7, 0x134(a0)
    2658: 10852a03     	lw	s4, 0x108(a0)
    265c: 10c52083     	lw	ra, 0x10c(a0)
    2660: f6142a23     	sw	ra, -0x8c(s0)
    2664: 11052383     	lw	t2, 0x110(a0)
    2668: 11452b83     	lw	s7, 0x114(a0)
    266c: f1742c23     	sw	s7, -0xe8(s0)
    2670: 11852a83     	lw	s5, 0x118(a0)
    2674: 11c52283     	lw	t0, 0x11c(a0)
    2678: 12052483     	lw	s1, 0x120(a0)
    267c: e8a42023     	sw	a0, -0x180(s0)
    2680: 12452303     	lw	t1, 0x124(a0)
    2684: 6b206537     	lui	a0, 0x6b206
    2688: 796235b7     	lui	a1, 0x79623
    268c: 33206637     	lui	a2, 0x33206
    2690: 617086b7     	lui	a3, 0x61708
    2694: 57450e13     	addi	t3, a0, 0x574
    2698: d3258993     	addi	s3, a1, -0x2ce
    269c: f5342223     	sw	s3, -0xbc(s0)
    26a0: 46e60d13     	addi	s10, a2, 0x46e
    26a4: 00170c13     	addi	s8, a4, 0x1
    26a8: 00270b13     	addi	s6, a4, 0x2
    26ac: 00370c93     	addi	s9, a4, 0x3
    26b0: 001c3513     	seqz	a0, s8
    26b4: 00eb35b3     	sltu	a1, s6, a4
    26b8: 00ecb633     	sltu	a2, s9, a4
    26bc: 00a78fb3     	add	t6, a5, a0
    26c0: 00b785b3     	add	a1, a5, a1
    26c4: 00c78633     	add	a2, a5, a2
    26c8: 86568693     	addi	a3, a3, -0x79b
    26cc: 00600513     	li	a0, 0x6
    26d0: fad42e23     	sw	a3, -0x44(s0)
    26d4: 000d0913     	mv	s2, s10
    26d8: f1342223     	sw	s3, -0xfc(s0)
    26dc: f3c42a23     	sw	t3, -0xcc(s0)
    26e0: fad42c23     	sw	a3, -0x48(s0)
    26e4: f5a42623     	sw	s10, -0xb4(s0)
    26e8: f3342e23     	sw	s3, -0xc4(s0)
    26ec: f3c42823     	sw	t3, -0xd0(s0)
    26f0: fcd42023     	sw	a3, -0x40(s0)
    26f4: f4d42a23     	sw	a3, -0xac(s0)
    26f8: f5a42023     	sw	s10, -0xc0(s0)
    26fc: f3342c23     	sw	s3, -0xc8(s0)
    2700: f3c42423     	sw	t3, -0xd8(s0)
    2704: fa942623     	sw	s1, -0x54(s0)
    2708: fa642423     	sw	t1, -0x58(s0)
    270c: f9542c23     	sw	s5, -0x68(s0)
    2710: f8542a23     	sw	t0, -0x6c(s0)
    2714: f6942223     	sw	s1, -0x9c(s0)
    2718: fa642023     	sw	t1, -0x60(s0)
    271c: f9542023     	sw	s5, -0x80(s0)
    2720: fa542a23     	sw	t0, -0x4c(s0)
    2724: f8942223     	sw	s1, -0x7c(s0)
    2728: fa942223     	sw	s1, -0x5c(s0)
    272c: fa642823     	sw	t1, -0x50(s0)
    2730: f8642e23     	sw	t1, -0x64(s0)
    2734: f7542623     	sw	s5, -0x94(s0)
    2738: f9542823     	sw	s5, -0x70(s0)
    273c: f6542c23     	sw	t0, -0x88(s0)
    2740: f8542623     	sw	t0, -0x74(s0)
    2744: f8742423     	sw	t2, -0x78(s0)
    2748: f7742823     	sw	s7, -0x90(s0)
    274c: f5442e23     	sw	s4, -0xa4(s0)
    2750: f0142623     	sw	ra, -0xf4(s0)
    2754: f6742423     	sw	t2, -0x98(s0)
    2758: f1742023     	sw	s7, -0x100(s0)
    275c: 000a0f13     	mv	t5, s4
    2760: fc142423     	sw	ra, -0x38(s0)
    2764: f4742c23     	sw	t2, -0xa8(s0)
    2768: f6742023     	sw	t2, -0xa0(s0)
    276c: fd442223     	sw	s4, -0x3c(s0)
    2770: f5442423     	sw	s4, -0xb8(s0)
    2774: f6142e23     	sw	ra, -0x84(s0)
    2778: f3042623     	sw	a6, -0xd4(s0)
    277c: f5142823     	sw	a7, -0xb0(s0)
    2780: e6e42e23     	sw	a4, -0x184(s0)
    2784: f2e42023     	sw	a4, -0xe0(s0)
    2788: e6f42c23     	sw	a5, -0x188(s0)
    278c: f2f42223     	sw	a5, -0xdc(s0)
    2790: 00080493     	mv	s1, a6
    2794: 00088693     	mv	a3, a7
    2798: e7842223     	sw	s8, -0x19c(s0)
    279c: 000c0713     	mv	a4, s8
    27a0: e5f42c23     	sw	t6, -0x1a8(s0)
    27a4: 000f8a93     	mv	s5, t6
    27a8: f1042823     	sw	a6, -0xf0(s0)
    27ac: f1142423     	sw	a7, -0xf8(s0)
    27b0: e7642423     	sw	s6, -0x198(s0)
    27b4: 000b0c13     	mv	s8, s6
    27b8: e4b42e23     	sw	a1, -0x1a4(s0)
    27bc: 00058393     	mv	t2, a1
    27c0: e7042a23     	sw	a6, -0x18c(s0)
    27c4: f1042a23     	sw	a6, -0xec(s0)
    27c8: e7142823     	sw	a7, -0x190(s0)
    27cc: f1142e23     	sw	a7, -0xe4(s0)
    27d0: e7942623     	sw	s9, -0x194(s0)
    27d4: 000c8893     	mv	a7, s9
    27d8: e6c42023     	sw	a2, -0x1a0(s0)
    27dc: fff50513     	addi	a0, a0, -0x1
    27e0: eea42e23     	sw	a0, -0x104(s0)
    27e4: fc042503     	lw	a0, -0x40(s0)
    27e8: 01e505b3     	add	a1, a0, t5
    27ec: eeb42a23     	sw	a1, -0x10c(s0)
    27f0: f2042503     	lw	a0, -0xe0(s0)
    27f4: 00a5c533     	xor	a0, a1, a0
    27f8: 01055593     	srli	a1, a0, 0x10
    27fc: 01051513     	slli	a0, a0, 0x10
    2800: 00b56833     	or	a6, a0, a1
    2804: ef042823     	sw	a6, -0x110(s0)
    2808: fc842583     	lw	a1, -0x38(s0)
    280c: 00bd05b3     	add	a1, s10, a1
    2810: f2b42023     	sw	a1, -0xe0(s0)
    2814: f2442503     	lw	a0, -0xdc(s0)
    2818: 00a5c5b3     	xor	a1, a1, a0
    281c: 0105d793     	srli	a5, a1, 0x10
    2820: 01059593     	slli	a1, a1, 0x10
    2824: 00f5e2b3     	or	t0, a1, a5
    2828: 000f0093     	mv	ra, t5
    282c: f6842b03     	lw	s6, -0x98(s0)
    2830: f4442503     	lw	a0, -0xbc(s0)
    2834: 016505b3     	add	a1, a0, s6
    2838: f4b42223     	sw	a1, -0xbc(s0)
    283c: f2c42503     	lw	a0, -0xd4(s0)
    2840: 00a5c5b3     	xor	a1, a1, a0
    2844: 0105d793     	srli	a5, a1, 0x10
    2848: 01059593     	slli	a1, a1, 0x10
    284c: ed742e23     	sw	s7, -0x124(s0)
    2850: 00f5ef33     	or	t5, a1, a5
    2854: efe42623     	sw	t5, -0x114(s0)
    2858: f0042d83     	lw	s11, -0x100(s0)
    285c: 01be05b3     	add	a1, t3, s11
    2860: f2b42623     	sw	a1, -0xd4(s0)
    2864: f5042503     	lw	a0, -0xb0(s0)
    2868: 00a5c5b3     	xor	a1, a1, a0
    286c: 0105d793     	srli	a5, a1, 0x10
    2870: 01059593     	slli	a1, a1, 0x10
    2874: 00f5e333     	or	t1, a1, a5
    2878: ee642423     	sw	t1, -0x118(s0)
    287c: fbc42503     	lw	a0, -0x44(s0)
    2880: f5c42a03     	lw	s4, -0xa4(s0)
    2884: 01450533     	add	a0, a0, s4
    2888: f4a42823     	sw	a0, -0xb0(s0)
    288c: 00a745b3     	xor	a1, a4, a0
    2890: 0105d793     	srli	a5, a1, 0x10
    2894: 01059593     	slli	a1, a1, 0x10
    2898: 00088e93     	mv	t4, a7
    289c: 00f5e9b3     	or	s3, a1, a5
    28a0: ed342c23     	sw	s3, -0x128(s0)
    28a4: f0c42883     	lw	a7, -0xf4(s0)
    28a8: 01190933     	add	s2, s2, a7
    28ac: f3242223     	sw	s2, -0xdc(s0)
    28b0: 012ac5b3     	xor	a1, s5, s2
    28b4: 0105d793     	srli	a5, a1, 0x10
    28b8: 01059593     	slli	a1, a1, 0x10
    28bc: 00f5efb3     	or	t6, a1, a5
    28c0: eff42223     	sw	t6, -0x11c(s0)
    28c4: f8842903     	lw	s2, -0x78(s0)
    28c8: f0442583     	lw	a1, -0xfc(s0)
    28cc: 012585b3     	add	a1, a1, s2
    28d0: f0b42223     	sw	a1, -0xfc(s0)
    28d4: 00b4c5b3     	xor	a1, s1, a1
    28d8: 0105d793     	srli	a5, a1, 0x10
    28dc: 01059593     	slli	a1, a1, 0x10
    28e0: 00f5e4b3     	or	s1, a1, a5
    28e4: ee942023     	sw	s1, -0x120(s0)
    28e8: f7042a83     	lw	s5, -0x90(s0)
    28ec: f3442503     	lw	a0, -0xcc(s0)
    28f0: 01550533     	add	a0, a0, s5
    28f4: f2a42a23     	sw	a0, -0xcc(s0)
    28f8: 00a6c5b3     	xor	a1, a3, a0
    28fc: 0105d793     	srli	a5, a1, 0x10
    2900: 01059593     	slli	a1, a1, 0x10
    2904: 00f5e5b3     	or	a1, a1, a5
    2908: ecb42223     	sw	a1, -0x13c(s0)
    290c: fb842503     	lw	a0, -0x48(s0)
    2910: fc442583     	lw	a1, -0x3c(s0)
    2914: 00b50533     	add	a0, a0, a1
    2918: eea42c23     	sw	a0, -0x108(s0)
    291c: 00ac45b3     	xor	a1, s8, a0
    2920: 0105d793     	srli	a5, a1, 0x10
    2924: 01059593     	slli	a1, a1, 0x10
    2928: 00f5e5b3     	or	a1, a1, a5
    292c: ecb42023     	sw	a1, -0x140(s0)
    2930: f7442c03     	lw	s8, -0x8c(s0)
    2934: f4c42503     	lw	a0, -0xb4(s0)
    2938: 01850533     	add	a0, a0, s8
    293c: f4a42623     	sw	a0, -0xb4(s0)
    2940: 00a3c5b3     	xor	a1, t2, a0
    2944: 0105d713     	srli	a4, a1, 0x10
    2948: 01059593     	slli	a1, a1, 0x10
    294c: 00e5e5b3     	or	a1, a1, a4
    2950: fcb42023     	sw	a1, -0x40(s0)
    2954: f5842383     	lw	t2, -0xa8(s0)
    2958: f3c42503     	lw	a0, -0xc4(s0)
    295c: 007505b3     	add	a1, a0, t2
    2960: f2b42e23     	sw	a1, -0xc4(s0)
    2964: f1042503     	lw	a0, -0xf0(s0)
    2968: 00b545b3     	xor	a1, a0, a1
    296c: 0105d713     	srli	a4, a1, 0x10
    2970: 01059593     	slli	a1, a1, 0x10
    2974: 00e5e5b3     	or	a1, a1, a4
    2978: fab42e23     	sw	a1, -0x44(s0)
    297c: f1842d03     	lw	s10, -0xe8(s0)
    2980: f3042503     	lw	a0, -0xd0(s0)
    2984: 01a505b3     	add	a1, a0, s10
    2988: f2b42823     	sw	a1, -0xd0(s0)
    298c: f0842503     	lw	a0, -0xf8(s0)
    2990: 00b545b3     	xor	a1, a0, a1
    2994: 0105d713     	srli	a4, a1, 0x10
    2998: 01059593     	slli	a1, a1, 0x10
    299c: 00e5e5b3     	or	a1, a1, a4
    29a0: eab42e23     	sw	a1, -0x144(s0)
    29a4: f5442503     	lw	a0, -0xac(s0)
    29a8: f4842e03     	lw	t3, -0xb8(s0)
    29ac: 01c50533     	add	a0, a0, t3
    29b0: f4a42a23     	sw	a0, -0xac(s0)
    29b4: 00aec5b3     	xor	a1, t4, a0
    29b8: 0105d713     	srli	a4, a1, 0x10
    29bc: 01059593     	slli	a1, a1, 0x10
    29c0: 00e5e5b3     	or	a1, a1, a4
    29c4: fab42c23     	sw	a1, -0x48(s0)
    29c8: f7c42c83     	lw	s9, -0x84(s0)
    29cc: f4042583     	lw	a1, -0xc0(s0)
    29d0: 019585b3     	add	a1, a1, s9
    29d4: f4b42023     	sw	a1, -0xc0(s0)
    29d8: 00b645b3     	xor	a1, a2, a1
    29dc: 0105d613     	srli	a2, a1, 0x10
    29e0: 01059593     	slli	a1, a1, 0x10
    29e4: 00c5eeb3     	or	t4, a1, a2
    29e8: f6042603     	lw	a2, -0xa0(s0)
    29ec: f3842583     	lw	a1, -0xc8(s0)
    29f0: 00c586b3     	add	a3, a1, a2
    29f4: f2d42c23     	sw	a3, -0xc8(s0)
    29f8: f1442583     	lw	a1, -0xec(s0)
    29fc: 00d5c5b3     	xor	a1, a1, a3
    2a00: 0105d693     	srli	a3, a1, 0x10
    2a04: 01059593     	slli	a1, a1, 0x10
    2a08: 00d5e6b3     	or	a3, a1, a3
    2a0c: f2842583     	lw	a1, -0xd8(s0)
    2a10: 01758733     	add	a4, a1, s7
    2a14: f2e42423     	sw	a4, -0xd8(s0)
    2a18: f1c42583     	lw	a1, -0xe4(s0)
    2a1c: 00e5c5b3     	xor	a1, a1, a4
    2a20: 0105d713     	srli	a4, a1, 0x10
    2a24: 01059593     	slli	a1, a1, 0x10
    2a28: 00e5e733     	or	a4, a1, a4
    2a2c: f8042583     	lw	a1, -0x80(s0)
    2a30: 00b805b3     	add	a1, a6, a1
    2a34: f0b42a23     	sw	a1, -0xec(s0)
    2a38: 0015c5b3     	xor	a1, a1, ra
    2a3c: 0145d793     	srli	a5, a1, 0x14
    2a40: 00c59593     	slli	a1, a1, 0xc
    2a44: 00f5e533     	or	a0, a1, a5
    2a48: f0a42823     	sw	a0, -0xf0(s0)
    2a4c: fb442783     	lw	a5, -0x4c(s0)
    2a50: 00f287b3     	add	a5, t0, a5
    2a54: f8f42023     	sw	a5, -0x80(s0)
    2a58: fc842583     	lw	a1, -0x38(s0)
    2a5c: 00b7c7b3     	xor	a5, a5, a1
    2a60: 0147d813     	srli	a6, a5, 0x14
    2a64: 00c79793     	slli	a5, a5, 0xc
    2a68: 0107e5b3     	or	a1, a5, a6
    2a6c: fab42a23     	sw	a1, -0x4c(s0)
    2a70: f6442783     	lw	a5, -0x9c(s0)
    2a74: 00ff07b3     	add	a5, t5, a5
    2a78: f6f42223     	sw	a5, -0x9c(s0)
    2a7c: 0167c7b3     	xor	a5, a5, s6
    2a80: 0147d813     	srli	a6, a5, 0x14
    2a84: 00c79793     	slli	a5, a5, 0xc
    2a88: 0107ef33     	or	t5, a5, a6
    2a8c: fa042783     	lw	a5, -0x60(s0)
    2a90: 00f307b3     	add	a5, t1, a5
    2a94: faf42023     	sw	a5, -0x60(s0)
    2a98: 01b7c7b3     	xor	a5, a5, s11
    2a9c: 0147d813     	srli	a6, a5, 0x14
    2aa0: 00c79793     	slli	a5, a5, 0xc
    2aa4: 0107e333     	or	t1, a5, a6
    2aa8: f9842783     	lw	a5, -0x68(s0)
    2aac: 00f987b3     	add	a5, s3, a5
    2ab0: f0f42e23     	sw	a5, -0xe4(s0)
    2ab4: 0147c7b3     	xor	a5, a5, s4
    2ab8: 0147d813     	srli	a6, a5, 0x14
    2abc: 00c79793     	slli	a5, a5, 0xc
    2ac0: 0107e5b3     	or	a1, a5, a6
    2ac4: fcb42423     	sw	a1, -0x38(s0)
    2ac8: f9442783     	lw	a5, -0x6c(s0)
    2acc: 00ff87b3     	add	a5, t6, a5
    2ad0: f8f42c23     	sw	a5, -0x68(s0)
    2ad4: 0117c7b3     	xor	a5, a5, a7
    2ad8: 0147d813     	srli	a6, a5, 0x14
    2adc: 00c79793     	slli	a5, a5, 0xc
    2ae0: 0107e5b3     	or	a1, a5, a6
    2ae4: eab42823     	sw	a1, -0x150(s0)
    2ae8: fac42783     	lw	a5, -0x54(s0)
    2aec: 00f487b3     	add	a5, s1, a5
    2af0: faf42623     	sw	a5, -0x54(s0)
    2af4: 0127c7b3     	xor	a5, a5, s2
    2af8: 0147d813     	srli	a6, a5, 0x14
    2afc: 00c79793     	slli	a5, a5, 0xc
    2b00: 0107e5b3     	or	a1, a5, a6
    2b04: f0b42623     	sw	a1, -0xf4(s0)
    2b08: fa842783     	lw	a5, -0x58(s0)
    2b0c: ec442f83     	lw	t6, -0x13c(s0)
    2b10: 00ff87b3     	add	a5, t6, a5
    2b14: faf42423     	sw	a5, -0x58(s0)
    2b18: 0157c7b3     	xor	a5, a5, s5
    2b1c: 0147d813     	srli	a6, a5, 0x14
    2b20: 00c79793     	slli	a5, a5, 0xc
    2b24: 0107edb3     	or	s11, a5, a6
    2b28: f1b42423     	sw	s11, -0xf8(s0)
    2b2c: f6c42783     	lw	a5, -0x94(s0)
    2b30: ec042a83     	lw	s5, -0x140(s0)
    2b34: 00fa87b3     	add	a5, s5, a5
    2b38: f8f42a23     	sw	a5, -0x6c(s0)
    2b3c: fc442803     	lw	a6, -0x3c(s0)
    2b40: 0107c7b3     	xor	a5, a5, a6
    2b44: 0147d813     	srli	a6, a5, 0x14
    2b48: 00c79793     	slli	a5, a5, 0xc
    2b4c: 0107e933     	or	s2, a5, a6
    2b50: ed242a23     	sw	s2, -0x12c(s0)
    2b54: f7842783     	lw	a5, -0x88(s0)
    2b58: fc042803     	lw	a6, -0x40(s0)
    2b5c: 00f807b3     	add	a5, a6, a5
    2b60: f8f42423     	sw	a5, -0x78(s0)
    2b64: 0187c7b3     	xor	a5, a5, s8
    2b68: 0147d813     	srli	a6, a5, 0x14
    2b6c: 00c79793     	slli	a5, a5, 0xc
    2b70: 0107e7b3     	or	a5, a5, a6
    2b74: eaf42423     	sw	a5, -0x158(s0)
    2b78: f8442783     	lw	a5, -0x7c(s0)
    2b7c: fbc42803     	lw	a6, -0x44(s0)
    2b80: 00f807b3     	add	a5, a6, a5
    2b84: f8f42223     	sw	a5, -0x7c(s0)
    2b88: 0077c7b3     	xor	a5, a5, t2
    2b8c: 0147d813     	srli	a6, a5, 0x14
    2b90: 00c79793     	slli	a5, a5, 0xc
    2b94: 0107eb33     	or	s6, a5, a6
    2b98: ed642823     	sw	s6, -0x130(s0)
    2b9c: fb042783     	lw	a5, -0x50(s0)
    2ba0: ebc42c03     	lw	s8, -0x144(s0)
    2ba4: 00fc07b3     	add	a5, s8, a5
    2ba8: f6f42c23     	sw	a5, -0x88(s0)
    2bac: 01a7c7b3     	xor	a5, a5, s10
    2bb0: 0147d813     	srli	a6, a5, 0x14
    2bb4: 00c79793     	slli	a5, a5, 0xc
    2bb8: 0107e7b3     	or	a5, a5, a6
    2bbc: eaf42023     	sw	a5, -0x160(s0)
    2bc0: f9042783     	lw	a5, -0x70(s0)
    2bc4: fb842803     	lw	a6, -0x48(s0)
    2bc8: 00f807b3     	add	a5, a6, a5
    2bcc: f8f42823     	sw	a5, -0x70(s0)
    2bd0: 01c7c7b3     	xor	a5, a5, t3
    2bd4: 0147d813     	srli	a6, a5, 0x14
    2bd8: 00c79793     	slli	a5, a5, 0xc
    2bdc: 0107ebb3     	or	s7, a5, a6
    2be0: ed742623     	sw	s7, -0x134(s0)
    2be4: f8c42783     	lw	a5, -0x74(s0)
    2be8: 00fe87b3     	add	a5, t4, a5
    2bec: f8f42623     	sw	a5, -0x74(s0)
    2bf0: 0197c7b3     	xor	a5, a5, s9
    2bf4: 0147d813     	srli	a6, a5, 0x14
    2bf8: 00c79793     	slli	a5, a5, 0xc
    2bfc: 0107e7b3     	or	a5, a5, a6
    2c00: e8f42e23     	sw	a5, -0x164(s0)
    2c04: fa442783     	lw	a5, -0x5c(s0)
    2c08: 00068093     	mv	ra, a3
    2c0c: 00f687b3     	add	a5, a3, a5
    2c10: f6f42e23     	sw	a5, -0x84(s0)
    2c14: 00c7c7b3     	xor	a5, a5, a2
    2c18: 0147d813     	srli	a6, a5, 0x14
    2c1c: 00c79793     	slli	a5, a5, 0xc
    2c20: 0107ecb3     	or	s9, a5, a6
    2c24: ed942423     	sw	s9, -0x138(s0)
    2c28: f9c42783     	lw	a5, -0x64(s0)
    2c2c: 00070693     	mv	a3, a4
    2c30: 00f707b3     	add	a5, a4, a5
    2c34: f8f42e23     	sw	a5, -0x64(s0)
    2c38: edc42603     	lw	a2, -0x124(s0)
    2c3c: 00c7c7b3     	xor	a5, a5, a2
    2c40: 0147d813     	srli	a6, a5, 0x14
    2c44: 00c79793     	slli	a5, a5, 0xc
    2c48: 0107e633     	or	a2, a5, a6
    2c4c: e8c42c23     	sw	a2, -0x168(s0)
    2c50: ef442a03     	lw	s4, -0x10c(s0)
    2c54: 01450a33     	add	s4, a0, s4
    2c58: f1442c23     	sw	s4, -0xe8(s0)
    2c5c: ef042503     	lw	a0, -0x110(s0)
    2c60: 00aa4533     	xor	a0, s4, a0
    2c64: 01855793     	srli	a5, a0, 0x18
    2c68: 00851513     	slli	a0, a0, 0x8
    2c6c: 00f568b3     	or	a7, a0, a5
    2c70: f2042603     	lw	a2, -0xe0(s0)
    2c74: fb442503     	lw	a0, -0x4c(s0)
    2c78: 00c50633     	add	a2, a0, a2
    2c7c: f6c42a23     	sw	a2, -0x8c(s0)
    2c80: 00564533     	xor	a0, a2, t0
    2c84: 01855793     	srli	a5, a0, 0x18
    2c88: 00851513     	slli	a0, a0, 0x8
    2c8c: 00f562b3     	or	t0, a0, a5
    2c90: f0542023     	sw	t0, -0x100(s0)
    2c94: f4442603     	lw	a2, -0xbc(s0)
    2c98: 000f0493     	mv	s1, t5
    2c9c: 00cf0633     	add	a2, t5, a2
    2ca0: f6c42823     	sw	a2, -0x90(s0)
    2ca4: eec42503     	lw	a0, -0x114(s0)
    2ca8: 00a64533     	xor	a0, a2, a0
    2cac: 01855793     	srli	a5, a0, 0x18
    2cb0: 00851513     	slli	a0, a0, 0x8
    2cb4: 00f56e33     	or	t3, a0, a5
    2cb8: edc42e23     	sw	t3, -0x124(s0)
    2cbc: f2c42603     	lw	a2, -0xd4(s0)
    2cc0: 00030993     	mv	s3, t1
    2cc4: 00c30633     	add	a2, t1, a2
    2cc8: f6c42623     	sw	a2, -0x94(s0)
    2ccc: ee842503     	lw	a0, -0x118(s0)
    2cd0: 00a64533     	xor	a0, a2, a0
    2cd4: 01855793     	srli	a5, a0, 0x18
    2cd8: 00851513     	slli	a0, a0, 0x8
    2cdc: 00f56533     	or	a0, a0, a5
    2ce0: e8a42a23     	sw	a0, -0x16c(s0)
    2ce4: f5042603     	lw	a2, -0xb0(s0)
    2ce8: fc842503     	lw	a0, -0x38(s0)
    2cec: 00c50633     	add	a2, a0, a2
    2cf0: f6c42423     	sw	a2, -0x98(s0)
    2cf4: ed842503     	lw	a0, -0x128(s0)
    2cf8: 00a64533     	xor	a0, a2, a0
    2cfc: 01855793     	srli	a5, a0, 0x18
    2d00: 00851513     	slli	a0, a0, 0x8
    2d04: 00f56333     	or	t1, a0, a5
    2d08: ee642823     	sw	t1, -0x110(s0)
    2d0c: f2442603     	lw	a2, -0xdc(s0)
    2d10: eb042d03     	lw	s10, -0x150(s0)
    2d14: 00cd0633     	add	a2, s10, a2
    2d18: f6c42023     	sw	a2, -0xa0(s0)
    2d1c: ee442503     	lw	a0, -0x11c(s0)
    2d20: 00a64533     	xor	a0, a2, a0
    2d24: 01855793     	srli	a5, a0, 0x18
    2d28: 00851513     	slli	a0, a0, 0x8
    2d2c: 00f563b3     	or	t2, a0, a5
    2d30: ee742623     	sw	t2, -0x114(s0)
    2d34: f0442603     	lw	a2, -0xfc(s0)
    2d38: 00c58533     	add	a0, a1, a2
    2d3c: f4a42e23     	sw	a0, -0xa4(s0)
    2d40: ee042583     	lw	a1, -0x120(s0)
    2d44: 00b54533     	xor	a0, a0, a1
    2d48: 01855793     	srli	a5, a0, 0x18
    2d4c: 00851513     	slli	a0, a0, 0x8
    2d50: 00f56f33     	or	t5, a0, a5
    2d54: ede42c23     	sw	t5, -0x128(s0)
    2d58: f3442603     	lw	a2, -0xcc(s0)
    2d5c: 00cd8633     	add	a2, s11, a2
    2d60: f4c42c23     	sw	a2, -0xa8(s0)
    2d64: 01f64533     	xor	a0, a2, t6
    2d68: 01855793     	srli	a5, a0, 0x18
    2d6c: 00851513     	slli	a0, a0, 0x8
    2d70: 00f56533     	or	a0, a0, a5
    2d74: eca42223     	sw	a0, -0x13c(s0)
    2d78: ef842603     	lw	a2, -0x108(s0)
    2d7c: 00c90633     	add	a2, s2, a2
    2d80: f4c42823     	sw	a2, -0xb0(s0)
    2d84: 01564533     	xor	a0, a2, s5
    2d88: 01855793     	srli	a5, a0, 0x18
    2d8c: 00851513     	slli	a0, a0, 0x8
    2d90: 00f56a33     	or	s4, a0, a5
    2d94: f4c42603     	lw	a2, -0xb4(s0)
    2d98: ea842903     	lw	s2, -0x158(s0)
    2d9c: 00c90633     	add	a2, s2, a2
    2da0: f4c42623     	sw	a2, -0xb4(s0)
    2da4: fc042503     	lw	a0, -0x40(s0)
    2da8: 00a64533     	xor	a0, a2, a0
    2dac: 01855793     	srli	a5, a0, 0x18
    2db0: 00851513     	slli	a0, a0, 0x8
    2db4: 00f56733     	or	a4, a0, a5
    2db8: eee42423     	sw	a4, -0x118(s0)
    2dbc: f3c42603     	lw	a2, -0xc4(s0)
    2dc0: 00cb0633     	add	a2, s6, a2
    2dc4: f4c42423     	sw	a2, -0xb8(s0)
    2dc8: fbc42503     	lw	a0, -0x44(s0)
    2dcc: 00a64533     	xor	a0, a2, a0
    2dd0: 01855793     	srli	a5, a0, 0x18
    2dd4: 00851513     	slli	a0, a0, 0x8
    2dd8: 00f56db3     	or	s11, a0, a5
    2ddc: f3042603     	lw	a2, -0xd0(s0)
    2de0: ea042b03     	lw	s6, -0x160(s0)
    2de4: 00cb0633     	add	a2, s6, a2
    2de8: f4c42223     	sw	a2, -0xbc(s0)
    2dec: 01864533     	xor	a0, a2, s8
    2df0: 01855793     	srli	a5, a0, 0x18
    2df4: 00851513     	slli	a0, a0, 0x8
    2df8: 00f56533     	or	a0, a0, a5
    2dfc: eca42023     	sw	a0, -0x140(s0)
    2e00: f5442603     	lw	a2, -0xac(s0)
    2e04: 00cb8633     	add	a2, s7, a2
    2e08: f4c42a23     	sw	a2, -0xac(s0)
    2e0c: fb842503     	lw	a0, -0x48(s0)
    2e10: 00a64533     	xor	a0, a2, a0
    2e14: 01855793     	srli	a5, a0, 0x18
    2e18: 00851513     	slli	a0, a0, 0x8
    2e1c: 00f56ab3     	or	s5, a0, a5
    2e20: ef542023     	sw	s5, -0x120(s0)
    2e24: f4042603     	lw	a2, -0xc0(s0)
    2e28: e9c42b83     	lw	s7, -0x164(s0)
    2e2c: 00cb8633     	add	a2, s7, a2
    2e30: f4c42023     	sw	a2, -0xc0(s0)
    2e34: 01d64533     	xor	a0, a2, t4
    2e38: 01855613     	srli	a2, a0, 0x18
    2e3c: 00851513     	slli	a0, a0, 0x8
    2e40: 00c56833     	or	a6, a0, a2
    2e44: f3842603     	lw	a2, -0xc8(s0)
    2e48: 00cc8533     	add	a0, s9, a2
    2e4c: f2a42e23     	sw	a0, -0xc4(s0)
    2e50: 00154533     	xor	a0, a0, ra
    2e54: 01855613     	srli	a2, a0, 0x18
    2e58: 00851513     	slli	a0, a0, 0x8
    2e5c: 00c56533     	or	a0, a0, a2
    2e60: fca42023     	sw	a0, -0x40(s0)
    2e64: f2842603     	lw	a2, -0xd8(s0)
    2e68: e9842e83     	lw	t4, -0x168(s0)
    2e6c: 00ce8533     	add	a0, t4, a2
    2e70: f2a42c23     	sw	a0, -0xc8(s0)
    2e74: 00d54533     	xor	a0, a0, a3
    2e78: 01855613     	srli	a2, a0, 0x18
    2e7c: 00851513     	slli	a0, a0, 0x8
    2e80: 00c56cb3     	or	s9, a0, a2
    2e84: f1442503     	lw	a0, -0xec(s0)
    2e88: 00a88533     	add	a0, a7, a0
    2e8c: 00088c13     	mv	s8, a7
    2e90: faa42e23     	sw	a0, -0x44(s0)
    2e94: f1042583     	lw	a1, -0xf0(s0)
    2e98: 00b54533     	xor	a0, a0, a1
    2e9c: 01955593     	srli	a1, a0, 0x19
    2ea0: 00751513     	slli	a0, a0, 0x7
    2ea4: 00b56533     	or	a0, a0, a1
    2ea8: fca42223     	sw	a0, -0x3c(s0)
    2eac: f8042503     	lw	a0, -0x80(s0)
    2eb0: 00a28533     	add	a0, t0, a0
    2eb4: faa42c23     	sw	a0, -0x48(s0)
    2eb8: fb442583     	lw	a1, -0x4c(s0)
    2ebc: 00b54533     	xor	a0, a0, a1
    2ec0: 01955593     	srli	a1, a0, 0x19
    2ec4: 00751513     	slli	a0, a0, 0x7
    2ec8: 00b567b3     	or	a5, a0, a1
    2ecc: f6442583     	lw	a1, -0x9c(s0)
    2ed0: 00be05b3     	add	a1, t3, a1
    2ed4: f0b42a23     	sw	a1, -0xec(s0)
    2ed8: 0095c633     	xor	a2, a1, s1
    2edc: 01965693     	srli	a3, a2, 0x19
    2ee0: 00761613     	slli	a2, a2, 0x7
    2ee4: 00d668b3     	or	a7, a2, a3
    2ee8: eb142c23     	sw	a7, -0x148(s0)
    2eec: fa042503     	lw	a0, -0x60(s0)
    2ef0: e9442583     	lw	a1, -0x16c(s0)
    2ef4: 00a58533     	add	a0, a1, a0
    2ef8: f2a42623     	sw	a0, -0xd4(s0)
    2efc: 01354633     	xor	a2, a0, s3
    2f00: 01965693     	srli	a3, a2, 0x19
    2f04: 00761613     	slli	a2, a2, 0x7
    2f08: 00d664b3     	or	s1, a2, a3
    2f0c: ea942a23     	sw	s1, -0x14c(s0)
    2f10: f1c42503     	lw	a0, -0xe4(s0)
    2f14: 00a30533     	add	a0, t1, a0
    2f18: faa42a23     	sw	a0, -0x4c(s0)
    2f1c: fc842603     	lw	a2, -0x38(s0)
    2f20: 00c54633     	xor	a2, a0, a2
    2f24: 01965693     	srli	a3, a2, 0x19
    2f28: 00761613     	slli	a2, a2, 0x7
    2f2c: 00d66633     	or	a2, a2, a3
    2f30: eec42c23     	sw	a2, -0x108(s0)
    2f34: f9842503     	lw	a0, -0x68(s0)
    2f38: 00a38533     	add	a0, t2, a0
    2f3c: faa42823     	sw	a0, -0x50(s0)
    2f40: 01a54633     	xor	a2, a0, s10
    2f44: 01965693     	srli	a3, a2, 0x19
    2f48: 00761613     	slli	a2, a2, 0x7
    2f4c: 00d66333     	or	t1, a2, a3
    2f50: fac42503     	lw	a0, -0x54(s0)
    2f54: 00af0533     	add	a0, t5, a0
    2f58: f2a42423     	sw	a0, -0xd8(s0)
    2f5c: f0c42603     	lw	a2, -0xf4(s0)
    2f60: 00c54633     	xor	a2, a0, a2
    2f64: 01965693     	srli	a3, a2, 0x19
    2f68: 00761613     	slli	a2, a2, 0x7
    2f6c: 00d66fb3     	or	t6, a2, a3
    2f70: ebf42823     	sw	t6, -0x150(s0)
    2f74: fa842503     	lw	a0, -0x58(s0)
    2f78: ec442d03     	lw	s10, -0x13c(s0)
    2f7c: 00ad0533     	add	a0, s10, a0
    2f80: f2a42223     	sw	a0, -0xdc(s0)
    2f84: f0842603     	lw	a2, -0xf8(s0)
    2f88: 00c54633     	xor	a2, a0, a2
    2f8c: 01965693     	srli	a3, a2, 0x19
    2f90: 00761613     	slli	a2, a2, 0x7
    2f94: 00d669b3     	or	s3, a2, a3
    2f98: eb342623     	sw	s3, -0x154(s0)
    2f9c: f9442503     	lw	a0, -0x6c(s0)
    2fa0: 00aa0533     	add	a0, s4, a0
    2fa4: faa42623     	sw	a0, -0x54(s0)
    2fa8: ed442603     	lw	a2, -0x12c(s0)
    2fac: 00c54633     	xor	a2, a0, a2
    2fb0: 01965693     	srli	a3, a2, 0x19
    2fb4: 00761613     	slli	a2, a2, 0x7
    2fb8: 00d66633     	or	a2, a2, a3
    2fbc: eec42a23     	sw	a2, -0x10c(s0)
    2fc0: f8842503     	lw	a0, -0x78(s0)
    2fc4: 00a70533     	add	a0, a4, a0
    2fc8: faa42423     	sw	a0, -0x58(s0)
    2fcc: 01254633     	xor	a2, a0, s2
    2fd0: 01965693     	srli	a3, a2, 0x19
    2fd4: 00761613     	slli	a2, a2, 0x7
    2fd8: 00d663b3     	or	t2, a2, a3
    2fdc: ea742423     	sw	t2, -0x158(s0)
    2fe0: f8442503     	lw	a0, -0x7c(s0)
    2fe4: 00ad8533     	add	a0, s11, a0
    2fe8: f2a42023     	sw	a0, -0xe0(s0)
    2fec: 000d8093     	mv	ra, s11
    2ff0: ed042603     	lw	a2, -0x130(s0)
    2ff4: 00c54633     	xor	a2, a0, a2
    2ff8: 01965713     	srli	a4, a2, 0x19
    2ffc: 00761613     	slli	a2, a2, 0x7
    3000: 00e66933     	or	s2, a2, a4
    3004: eb242223     	sw	s2, -0x15c(s0)
    3008: f7842503     	lw	a0, -0x88(s0)
    300c: ec042703     	lw	a4, -0x140(s0)
    3010: 00a70533     	add	a0, a4, a0
    3014: f0a42e23     	sw	a0, -0xe4(s0)
    3018: 01654633     	xor	a2, a0, s6
    301c: 01965293     	srli	t0, a2, 0x19
    3020: 00761613     	slli	a2, a2, 0x7
    3024: 00566b33     	or	s6, a2, t0
    3028: eb642023     	sw	s6, -0x160(s0)
    302c: f9042503     	lw	a0, -0x70(s0)
    3030: 00aa8533     	add	a0, s5, a0
    3034: faa42223     	sw	a0, -0x5c(s0)
    3038: ecc42603     	lw	a2, -0x134(s0)
    303c: 00c54633     	xor	a2, a0, a2
    3040: 01965293     	srli	t0, a2, 0x19
    3044: 00761613     	slli	a2, a2, 0x7
    3048: 00566533     	or	a0, a2, t0
    304c: eea42223     	sw	a0, -0x11c(s0)
    3050: f8c42503     	lw	a0, -0x74(s0)
    3054: 00a80533     	add	a0, a6, a0
    3058: 00080a93     	mv	s5, a6
    305c: faa42023     	sw	a0, -0x60(s0)
    3060: 01754633     	xor	a2, a0, s7
    3064: 01965293     	srli	t0, a2, 0x19
    3068: 00761613     	slli	a2, a2, 0x7
    306c: 00566e33     	or	t3, a2, t0
    3070: e9c42e23     	sw	t3, -0x164(s0)
    3074: f7c42d83     	lw	s11, -0x84(s0)
    3078: fc042503     	lw	a0, -0x40(s0)
    307c: 01b50db3     	add	s11, a0, s11
    3080: f1b42823     	sw	s11, -0xf0(s0)
    3084: ec842503     	lw	a0, -0x138(s0)
    3088: 00adc633     	xor	a2, s11, a0
    308c: 01965293     	srli	t0, a2, 0x19
    3090: 00761613     	slli	a2, a2, 0x7
    3094: 005662b3     	or	t0, a2, t0
    3098: f9c42503     	lw	a0, -0x64(s0)
    309c: 00ac8533     	add	a0, s9, a0
    30a0: f0a42623     	sw	a0, -0xf4(s0)
    30a4: 01d54633     	xor	a2, a0, t4
    30a8: 01965f13     	srli	t5, a2, 0x19
    30ac: 00761613     	slli	a2, a2, 0x7
    30b0: 01e66eb3     	or	t4, a2, t5
    30b4: e9d42c23     	sw	t4, -0x168(s0)
    30b8: f1842503     	lw	a0, -0xe8(s0)
    30bc: 00a78533     	add	a0, a5, a0
    30c0: f0a42423     	sw	a0, -0xf8(s0)
    30c4: 00b54633     	xor	a2, a0, a1
    30c8: 01065813     	srli	a6, a2, 0x10
    30cc: 01061613     	slli	a2, a2, 0x10
    30d0: 010665b3     	or	a1, a2, a6
    30d4: f0b42223     	sw	a1, -0xfc(s0)
    30d8: f7442503     	lw	a0, -0x8c(s0)
    30dc: 00a88533     	add	a0, a7, a0
    30e0: f8a42e23     	sw	a0, -0x64(s0)
    30e4: 01854833     	xor	a6, a0, s8
    30e8: 01085513     	srli	a0, a6, 0x10
    30ec: 01081813     	slli	a6, a6, 0x10
    30f0: 00a86f33     	or	t5, a6, a0
    30f4: ede42423     	sw	t5, -0x138(s0)
    30f8: f7042603     	lw	a2, -0x90(s0)
    30fc: 00c48633     	add	a2, s1, a2
    3100: f2c42a23     	sw	a2, -0xcc(s0)
    3104: f0042503     	lw	a0, -0x100(s0)
    3108: 00a64533     	xor	a0, a2, a0
    310c: 01055813     	srli	a6, a0, 0x10
    3110: 01051513     	slli	a0, a0, 0x10
    3114: 01056c33     	or	s8, a0, a6
    3118: f6c42b83     	lw	s7, -0x94(s0)
    311c: fc442503     	lw	a0, -0x3c(s0)
    3120: 01750bb3     	add	s7, a0, s7
    3124: f7742223     	sw	s7, -0x9c(s0)
    3128: edc42503     	lw	a0, -0x124(s0)
    312c: 00abc533     	xor	a0, s7, a0
    3130: 01055813     	srli	a6, a0, 0x10
    3134: 01051513     	slli	a0, a0, 0x10
    3138: 010564b3     	or	s1, a0, a6
    313c: ec942e23     	sw	s1, -0x124(s0)
    3140: f6842683     	lw	a3, -0x98(s0)
    3144: 00d306b3     	add	a3, t1, a3
    3148: 00030b93     	mv	s7, t1
    314c: f8d42c23     	sw	a3, -0x68(s0)
    3150: 01a6c533     	xor	a0, a3, s10
    3154: 01055813     	srli	a6, a0, 0x10
    3158: 01051513     	slli	a0, a0, 0x10
    315c: 01056d33     	or	s10, a0, a6
    3160: f6042683     	lw	a3, -0xa0(s0)
    3164: 00df86b3     	add	a3, t6, a3
    3168: f2d42823     	sw	a3, -0xd0(s0)
    316c: ef042503     	lw	a0, -0x110(s0)
    3170: 00a6c533     	xor	a0, a3, a0
    3174: 01055813     	srli	a6, a0, 0x10
    3178: 01051513     	slli	a0, a0, 0x10
    317c: 010568b3     	or	a7, a0, a6
    3180: ed142a23     	sw	a7, -0x12c(s0)
    3184: f5c42683     	lw	a3, -0xa4(s0)
    3188: 00d986b3     	add	a3, s3, a3
    318c: f8d42a23     	sw	a3, -0x6c(s0)
    3190: eec42503     	lw	a0, -0x114(s0)
    3194: 00a6c533     	xor	a0, a3, a0
    3198: 01055813     	srli	a6, a0, 0x10
    319c: 01051513     	slli	a0, a0, 0x10
    31a0: 01056db3     	or	s11, a0, a6
    31a4: edb42823     	sw	s11, -0x130(s0)
    31a8: ef842303     	lw	t1, -0x108(s0)
    31ac: f5842803     	lw	a6, -0xa8(s0)
    31b0: 01030833     	add	a6, t1, a6
    31b4: f9042823     	sw	a6, -0x70(s0)
    31b8: ed842503     	lw	a0, -0x128(s0)
    31bc: 00a84533     	xor	a0, a6, a0
    31c0: 01055813     	srli	a6, a0, 0x10
    31c4: 01051513     	slli	a0, a0, 0x10
    31c8: 01056533     	or	a0, a0, a6
    31cc: e8a42423     	sw	a0, -0x178(s0)
    31d0: f5042803     	lw	a6, -0xb0(s0)
    31d4: 01038833     	add	a6, t2, a6
    31d8: f9042623     	sw	a6, -0x74(s0)
    31dc: 00e84533     	xor	a0, a6, a4
    31e0: 01055813     	srli	a6, a0, 0x10
    31e4: 01051513     	slli	a0, a0, 0x10
    31e8: 010566b3     	or	a3, a0, a6
    31ec: ecd42223     	sw	a3, -0x13c(s0)
    31f0: f4c42803     	lw	a6, -0xb4(s0)
    31f4: 01090833     	add	a6, s2, a6
    31f8: f9042423     	sw	a6, -0x78(s0)
    31fc: 01484533     	xor	a0, a6, s4
    3200: 01055813     	srli	a6, a0, 0x10
    3204: 01051513     	slli	a0, a0, 0x10
    3208: 01056533     	or	a0, a0, a6
    320c: fca42423     	sw	a0, -0x38(s0)
    3210: f4842803     	lw	a6, -0xb8(s0)
    3214: 010b0833     	add	a6, s6, a6
    3218: f7042423     	sw	a6, -0x98(s0)
    321c: ee842503     	lw	a0, -0x118(s0)
    3220: 00a84533     	xor	a0, a6, a0
    3224: 01055813     	srli	a6, a0, 0x10
    3228: 01051513     	slli	a0, a0, 0x10
    322c: 01056633     	or	a2, a0, a6
    3230: f4c42823     	sw	a2, -0xb0(s0)
    3234: ef442b03     	lw	s6, -0x10c(s0)
    3238: f4442803     	lw	a6, -0xbc(s0)
    323c: 010b0833     	add	a6, s6, a6
    3240: f9042223     	sw	a6, -0x7c(s0)
    3244: 00184533     	xor	a0, a6, ra
    3248: 01055813     	srli	a6, a0, 0x10
    324c: 01051513     	slli	a0, a0, 0x10
    3250: 01056a33     	or	s4, a0, a6
    3254: ed442c23     	sw	s4, -0x128(s0)
    3258: f5442803     	lw	a6, -0xac(s0)
    325c: 010e0833     	add	a6, t3, a6
    3260: f9042023     	sw	a6, -0x80(s0)
    3264: 01984533     	xor	a0, a6, s9
    3268: 01055813     	srli	a6, a0, 0x10
    326c: 01051513     	slli	a0, a0, 0x10
    3270: 010563b3     	or	t2, a0, a6
    3274: ec742623     	sw	t2, -0x134(s0)
    3278: f4042803     	lw	a6, -0xc0(s0)
    327c: 01028533     	add	a0, t0, a6
    3280: f6a42e23     	sw	a0, -0x84(s0)
    3284: ee042703     	lw	a4, -0x120(s0)
    3288: 00e54533     	xor	a0, a0, a4
    328c: 01055813     	srli	a6, a0, 0x10
    3290: 01051513     	slli	a0, a0, 0x10
    3294: 01056533     	or	a0, a0, a6
    3298: e8a42223     	sw	a0, -0x17c(s0)
    329c: f3c42703     	lw	a4, -0xc4(s0)
    32a0: 00ee8533     	add	a0, t4, a4
    32a4: f6a42c23     	sw	a0, -0x88(s0)
    32a8: 01554533     	xor	a0, a0, s5
    32ac: 01055813     	srli	a6, a0, 0x10
    32b0: 01051513     	slli	a0, a0, 0x10
    32b4: 01056ab3     	or	s5, a0, a6
    32b8: ed542023     	sw	s5, -0x140(s0)
    32bc: ee442083     	lw	ra, -0x11c(s0)
    32c0: f3842703     	lw	a4, -0xc8(s0)
    32c4: 00e08533     	add	a0, ra, a4
    32c8: f6a42a23     	sw	a0, -0x8c(s0)
    32cc: fc042703     	lw	a4, -0x40(s0)
    32d0: 00e54533     	xor	a0, a0, a4
    32d4: 01055813     	srli	a6, a0, 0x10
    32d8: 01051513     	slli	a0, a0, 0x10
    32dc: 01056eb3     	or	t4, a0, a6
    32e0: ebd42e23     	sw	t4, -0x144(s0)
    32e4: f1442503     	lw	a0, -0xec(s0)
    32e8: 00a58533     	add	a0, a1, a0
    32ec: f0a42023     	sw	a0, -0x100(s0)
    32f0: 00f54533     	xor	a0, a0, a5
    32f4: 01455593     	srli	a1, a0, 0x14
    32f8: 00c51513     	slli	a0, a0, 0xc
    32fc: 00b565b3     	or	a1, a0, a1
    3300: eeb42423     	sw	a1, -0x118(s0)
    3304: f2c42503     	lw	a0, -0xd4(s0)
    3308: 00af0533     	add	a0, t5, a0
    330c: f6a42823     	sw	a0, -0x90(s0)
    3310: eb842703     	lw	a4, -0x148(s0)
    3314: 00e54533     	xor	a0, a0, a4
    3318: 01455793     	srli	a5, a0, 0x14
    331c: 00c51513     	slli	a0, a0, 0xc
    3320: 00f569b3     	or	s3, a0, a5
    3324: e9342a23     	sw	s3, -0x16c(s0)
    3328: fbc42503     	lw	a0, -0x44(s0)
    332c: 00ac0533     	add	a0, s8, a0
    3330: f6a42623     	sw	a0, -0x94(s0)
    3334: eb442703     	lw	a4, -0x14c(s0)
    3338: 00e54533     	xor	a0, a0, a4
    333c: 01455793     	srli	a5, a0, 0x14
    3340: 00c51513     	slli	a0, a0, 0xc
    3344: 00f56f33     	or	t5, a0, a5
    3348: f3e42623     	sw	t5, -0xd4(s0)
    334c: fb842503     	lw	a0, -0x48(s0)
    3350: 00a48533     	add	a0, s1, a0
    3354: f6a42023     	sw	a0, -0xa0(s0)
    3358: fc442783     	lw	a5, -0x3c(s0)
    335c: 00f547b3     	xor	a5, a0, a5
    3360: 0147d813     	srli	a6, a5, 0x14
    3364: 00c79793     	slli	a5, a5, 0xc
    3368: 0107e4b3     	or	s1, a5, a6
    336c: e8942823     	sw	s1, -0x170(s0)
    3370: f2842503     	lw	a0, -0xd8(s0)
    3374: 00ad0533     	add	a0, s10, a0
    3378: 000d0913     	mv	s2, s10
    337c: fca42223     	sw	a0, -0x3c(s0)
    3380: 017547b3     	xor	a5, a0, s7
    3384: 0147d813     	srli	a6, a5, 0x14
    3388: 00c79793     	slli	a5, a5, 0xc
    338c: 0107efb3     	or	t6, a5, a6
    3390: ebf42c23     	sw	t6, -0x148(s0)
    3394: f2442503     	lw	a0, -0xdc(s0)
    3398: 00a88533     	add	a0, a7, a0
    339c: f4a42e23     	sw	a0, -0xa4(s0)
    33a0: eb042783     	lw	a5, -0x150(s0)
    33a4: 00f547b3     	xor	a5, a0, a5
    33a8: 0147d813     	srli	a6, a5, 0x14
    33ac: 00c79793     	slli	a5, a5, 0xc
    33b0: 0107ee33     	or	t3, a5, a6
    33b4: e9c42623     	sw	t3, -0x174(s0)
    33b8: fb442503     	lw	a0, -0x4c(s0)
    33bc: 00ad8533     	add	a0, s11, a0
    33c0: f4a42c23     	sw	a0, -0xa8(s0)
    33c4: eac42783     	lw	a5, -0x154(s0)
    33c8: 00f547b3     	xor	a5, a0, a5
    33cc: 0147d893     	srli	a7, a5, 0x14
    33d0: 00c79793     	slli	a5, a5, 0xc
    33d4: 0117e7b3     	or	a5, a5, a7
    33d8: eaf42a23     	sw	a5, -0x14c(s0)
    33dc: fb042503     	lw	a0, -0x50(s0)
    33e0: e8842b83     	lw	s7, -0x178(s0)
    33e4: 00ab8533     	add	a0, s7, a0
    33e8: faa42823     	sw	a0, -0x50(s0)
    33ec: 006548b3     	xor	a7, a0, t1
    33f0: 0148d313     	srli	t1, a7, 0x14
    33f4: 00c89893     	slli	a7, a7, 0xc
    33f8: 0068e833     	or	a6, a7, t1
    33fc: eb042823     	sw	a6, -0x150(s0)
    3400: f2042503     	lw	a0, -0xe0(s0)
    3404: 00a68533     	add	a0, a3, a0
    3408: f4a42423     	sw	a0, -0xb8(s0)
    340c: ea842683     	lw	a3, -0x158(s0)
    3410: 00d546b3     	xor	a3, a0, a3
    3414: 0146d893     	srli	a7, a3, 0x14
    3418: 00c69693     	slli	a3, a3, 0xc
    341c: 0116e333     	or	t1, a3, a7
    3420: ee642023     	sw	t1, -0x120(s0)
    3424: f1c42503     	lw	a0, -0xe4(s0)
    3428: fc842683     	lw	a3, -0x38(s0)
    342c: 00a68533     	add	a0, a3, a0
    3430: f0a42e23     	sw	a0, -0xe4(s0)
    3434: ea442683     	lw	a3, -0x15c(s0)
    3438: 00d546b3     	xor	a3, a0, a3
    343c: 0146d713     	srli	a4, a3, 0x14
    3440: 00c69693     	slli	a3, a3, 0xc
    3444: 00e6ecb3     	or	s9, a3, a4
    3448: fac42503     	lw	a0, -0x54(s0)
    344c: 00a60533     	add	a0, a2, a0
    3450: f0a42c23     	sw	a0, -0xe8(s0)
    3454: ea042683     	lw	a3, -0x160(s0)
    3458: 00d546b3     	xor	a3, a0, a3
    345c: 0146d713     	srli	a4, a3, 0x14
    3460: 00c69693     	slli	a3, a3, 0xc
    3464: 00e6e6b3     	or	a3, a3, a4
    3468: ead42223     	sw	a3, -0x15c(s0)
    346c: fa842503     	lw	a0, -0x58(s0)
    3470: 00aa0533     	add	a0, s4, a0
    3474: f0a42a23     	sw	a0, -0xec(s0)
    3478: 016546b3     	xor	a3, a0, s6
    347c: 0146d713     	srli	a4, a3, 0x14
    3480: 00c69693     	slli	a3, a3, 0xc
    3484: 00e6e8b3     	or	a7, a3, a4
    3488: eb142623     	sw	a7, -0x154(s0)
    348c: f1042d83     	lw	s11, -0xf0(s0)
    3490: 01b38db3     	add	s11, t2, s11
    3494: efb42623     	sw	s11, -0x114(s0)
    3498: e9c42503     	lw	a0, -0x164(s0)
    349c: 00adc6b3     	xor	a3, s11, a0
    34a0: 0146d713     	srli	a4, a3, 0x14
    34a4: 00c69693     	slli	a3, a3, 0xc
    34a8: 00e6e6b3     	or	a3, a3, a4
    34ac: ead42023     	sw	a3, -0x160(s0)
    34b0: e8442d83     	lw	s11, -0x17c(s0)
    34b4: f0c42503     	lw	a0, -0xf4(s0)
    34b8: 00ad8533     	add	a0, s11, a0
    34bc: eea42c23     	sw	a0, -0x108(s0)
    34c0: 005546b3     	xor	a3, a0, t0
    34c4: 0146d713     	srli	a4, a3, 0x14
    34c8: 00c69693     	slli	a3, a3, 0xc
    34cc: 00e6e2b3     	or	t0, a3, a4
    34d0: ea542423     	sw	t0, -0x158(s0)
    34d4: fa442503     	lw	a0, -0x5c(s0)
    34d8: 00aa8533     	add	a0, s5, a0
    34dc: eea42a23     	sw	a0, -0x10c(s0)
    34e0: e9842683     	lw	a3, -0x168(s0)
    34e4: 00d546b3     	xor	a3, a0, a3
    34e8: 0146d713     	srli	a4, a3, 0x14
    34ec: 00c69693     	slli	a3, a3, 0xc
    34f0: 00e6ea33     	or	s4, a3, a4
    34f4: fa042503     	lw	a0, -0x60(s0)
    34f8: 00ae8533     	add	a0, t4, a0
    34fc: eea42823     	sw	a0, -0x110(s0)
    3500: 001546b3     	xor	a3, a0, ra
    3504: 0146d713     	srli	a4, a3, 0x14
    3508: 00c69693     	slli	a3, a3, 0xc
    350c: 00e6e6b3     	or	a3, a3, a4
    3510: eed42223     	sw	a3, -0x11c(s0)
    3514: f0842683     	lw	a3, -0xf8(s0)
    3518: 00d586b3     	add	a3, a1, a3
    351c: fcd42023     	sw	a3, -0x40(s0)
    3520: f0442603     	lw	a2, -0xfc(s0)
    3524: 00c6c633     	xor	a2, a3, a2
    3528: 01865693     	srli	a3, a2, 0x18
    352c: 00861613     	slli	a2, a2, 0x8
    3530: 00d663b3     	or	t2, a2, a3
    3534: f9c42d03     	lw	s10, -0x64(s0)
    3538: 01a98d33     	add	s10, s3, s10
    353c: ec842503     	lw	a0, -0x138(s0)
    3540: 00ad4633     	xor	a2, s10, a0
    3544: 01865693     	srli	a3, a2, 0x18
    3548: 00861613     	slli	a2, a2, 0x8
    354c: 00d66b33     	or	s6, a2, a3
    3550: f3442603     	lw	a2, -0xcc(s0)
    3554: 00cf0633     	add	a2, t5, a2
    3558: f4c42223     	sw	a2, -0xbc(s0)
    355c: 01864633     	xor	a2, a2, s8
    3560: 01865693     	srli	a3, a2, 0x18
    3564: 00861613     	slli	a2, a2, 0x8
    3568: 00d66f33     	or	t5, a2, a3
    356c: f6442c03     	lw	s8, -0x9c(s0)
    3570: 01848c33     	add	s8, s1, s8
    3574: edc42503     	lw	a0, -0x124(s0)
    3578: 00ac4633     	xor	a2, s8, a0
    357c: 01865693     	srli	a3, a2, 0x18
    3580: 00861613     	slli	a2, a2, 0x8
    3584: 00d664b3     	or	s1, a2, a3
    3588: f9842603     	lw	a2, -0x68(s0)
    358c: 00cf8633     	add	a2, t6, a2
    3590: fac42e23     	sw	a2, -0x44(s0)
    3594: 01264633     	xor	a2, a2, s2
    3598: 01865693     	srli	a3, a2, 0x18
    359c: 00861613     	slli	a2, a2, 0x8
    35a0: 00d66533     	or	a0, a2, a3
    35a4: f3042903     	lw	s2, -0xd0(s0)
    35a8: 012e09b3     	add	s3, t3, s2
    35ac: ed442583     	lw	a1, -0x12c(s0)
    35b0: 00b9c633     	xor	a2, s3, a1
    35b4: 01865693     	srli	a3, a2, 0x18
    35b8: 00861613     	slli	a2, a2, 0x8
    35bc: 00d660b3     	or	ra, a2, a3
    35c0: f9442e83     	lw	t4, -0x6c(s0)
    35c4: 01d78eb3     	add	t4, a5, t4
    35c8: f1d42223     	sw	t4, -0xfc(s0)
    35cc: ed042583     	lw	a1, -0x130(s0)
    35d0: 00bec633     	xor	a2, t4, a1
    35d4: 01865693     	srli	a3, a2, 0x18
    35d8: 00861613     	slli	a2, a2, 0x8
    35dc: 00d66ab3     	or	s5, a2, a3
    35e0: f9042603     	lw	a2, -0x70(s0)
    35e4: 00c80633     	add	a2, a6, a2
    35e8: f2c42a23     	sw	a2, -0xcc(s0)
    35ec: 01764633     	xor	a2, a2, s7
    35f0: 01865693     	srli	a3, a2, 0x18
    35f4: 00861613     	slli	a2, a2, 0x8
    35f8: 00d66933     	or	s2, a2, a3
    35fc: f8c42603     	lw	a2, -0x74(s0)
    3600: 00c30633     	add	a2, t1, a2
    3604: fac42c23     	sw	a2, -0x48(s0)
    3608: ec442583     	lw	a1, -0x13c(s0)
    360c: 00b64633     	xor	a2, a2, a1
    3610: 01865693     	srli	a3, a2, 0x18
    3614: 00861613     	slli	a2, a2, 0x8
    3618: 00d66733     	or	a4, a2, a3
    361c: f8842603     	lw	a2, -0x78(s0)
    3620: 00cc8633     	add	a2, s9, a2
    3624: f4c42623     	sw	a2, -0xb4(s0)
    3628: fc842583     	lw	a1, -0x38(s0)
    362c: 00b64633     	xor	a2, a2, a1
    3630: 01865693     	srli	a3, a2, 0x18
    3634: 00861613     	slli	a2, a2, 0x8
    3638: 00d66833     	or	a6, a2, a3
    363c: f6842603     	lw	a2, -0x98(s0)
    3640: ea442783     	lw	a5, -0x15c(s0)
    3644: 00c78633     	add	a2, a5, a2
    3648: f2c42e23     	sw	a2, -0xc4(s0)
    364c: f5042583     	lw	a1, -0xb0(s0)
    3650: 00b64633     	xor	a2, a2, a1
    3654: 01865693     	srli	a3, a2, 0x18
    3658: 00861613     	slli	a2, a2, 0x8
    365c: 00d66e33     	or	t3, a2, a3
    3660: f8442603     	lw	a2, -0x7c(s0)
    3664: 00c88633     	add	a2, a7, a2
    3668: f2c42823     	sw	a2, -0xd0(s0)
    366c: ed842583     	lw	a1, -0x128(s0)
    3670: 00b64633     	xor	a2, a2, a1
    3674: 01865693     	srli	a3, a2, 0x18
    3678: 00861613     	slli	a2, a2, 0x8
    367c: 00d66bb3     	or	s7, a2, a3
    3680: f8042603     	lw	a2, -0x80(s0)
    3684: ea042883     	lw	a7, -0x160(s0)
    3688: 00c88633     	add	a2, a7, a2
    368c: f4c42a23     	sw	a2, -0xac(s0)
    3690: ecc42583     	lw	a1, -0x134(s0)
    3694: 00b64633     	xor	a2, a2, a1
    3698: 01865693     	srli	a3, a2, 0x18
    369c: 00861613     	slli	a2, a2, 0x8
    36a0: 00d66fb3     	or	t6, a2, a3
    36a4: f7c42603     	lw	a2, -0x84(s0)
    36a8: 00c28633     	add	a2, t0, a2
    36ac: f4c42023     	sw	a2, -0xc0(s0)
    36b0: 01b64633     	xor	a2, a2, s11
    36b4: 01865693     	srli	a3, a2, 0x18
    36b8: 00861613     	slli	a2, a2, 0x8
    36bc: 00d66eb3     	or	t4, a2, a3
    36c0: f7842603     	lw	a2, -0x88(s0)
    36c4: 00ca0633     	add	a2, s4, a2
    36c8: f2c42c23     	sw	a2, -0xc8(s0)
    36cc: ec042583     	lw	a1, -0x140(s0)
    36d0: 00b64633     	xor	a2, a2, a1
    36d4: 01865293     	srli	t0, a2, 0x18
    36d8: 00861613     	slli	a2, a2, 0x8
    36dc: 00566db3     	or	s11, a2, t0
    36e0: f7442283     	lw	t0, -0x8c(s0)
    36e4: ee442603     	lw	a2, -0x11c(s0)
    36e8: 005602b3     	add	t0, a2, t0
    36ec: f2542423     	sw	t0, -0xd8(s0)
    36f0: ebc42583     	lw	a1, -0x144(s0)
    36f4: 00b2c2b3     	xor	t0, t0, a1
    36f8: 0182d313     	srli	t1, t0, 0x18
    36fc: 00829293     	slli	t0, t0, 0x8
    3700: 0062e333     	or	t1, t0, t1
    3704: f4742823     	sw	t2, -0xb0(s0)
    3708: f0042283     	lw	t0, -0x100(s0)
    370c: 005382b3     	add	t0, t2, t0
    3710: f6542223     	sw	t0, -0x9c(s0)
    3714: ee842583     	lw	a1, -0x118(s0)
    3718: 00b2c5b3     	xor	a1, t0, a1
    371c: 0195d293     	srli	t0, a1, 0x19
    3720: 00759593     	slli	a1, a1, 0x7
    3724: 0055e5b3     	or	a1, a1, t0
    3728: fcb42423     	sw	a1, -0x38(s0)
    372c: f3642023     	sw	s6, -0xe0(s0)
    3730: f7042583     	lw	a1, -0x90(s0)
    3734: 00bb05b3     	add	a1, s6, a1
    3738: 00050693     	mv	a3, a0
    373c: fab42023     	sw	a1, -0x60(s0)
    3740: e9442503     	lw	a0, -0x16c(s0)
    3744: 00a5c5b3     	xor	a1, a1, a0
    3748: 0195d293     	srli	t0, a1, 0x19
    374c: 00759593     	slli	a1, a1, 0x7
    3750: 0055e5b3     	or	a1, a1, t0
    3754: f6b42423     	sw	a1, -0x98(s0)
    3758: f3e42223     	sw	t5, -0xdc(s0)
    375c: f6c42583     	lw	a1, -0x94(s0)
    3760: 00bf05b3     	add	a1, t5, a1
    3764: f8b42023     	sw	a1, -0x80(s0)
    3768: f2c42503     	lw	a0, -0xd4(s0)
    376c: 00a5c533     	xor	a0, a1, a0
    3770: 01955593     	srli	a1, a0, 0x19
    3774: 00751513     	slli	a0, a0, 0x7
    3778: 00b56533     	or	a0, a0, a1
    377c: f0a42023     	sw	a0, -0x100(s0)
    3780: f2942623     	sw	s1, -0xd4(s0)
    3784: f6042503     	lw	a0, -0xa0(s0)
    3788: 00a48533     	add	a0, s1, a0
    378c: 00090493     	mv	s1, s2
    3790: faa42a23     	sw	a0, -0x4c(s0)
    3794: e9042583     	lw	a1, -0x170(s0)
    3798: 00b54533     	xor	a0, a0, a1
    379c: 01955593     	srli	a1, a0, 0x19
    37a0: 00751513     	slli	a0, a0, 0x7
    37a4: 00b56f33     	or	t5, a0, a1
    37a8: fc442503     	lw	a0, -0x3c(s0)
    37ac: 00a68533     	add	a0, a3, a0
    37b0: faa42623     	sw	a0, -0x54(s0)
    37b4: eb842583     	lw	a1, -0x148(s0)
    37b8: 00b54533     	xor	a0, a0, a1
    37bc: 01955593     	srli	a1, a0, 0x19
    37c0: 00751513     	slli	a0, a0, 0x7
    37c4: 00b56533     	or	a0, a0, a1
    37c8: f0a42623     	sw	a0, -0xf4(s0)
    37cc: f5c42503     	lw	a0, -0xa4(s0)
    37d0: 00a08533     	add	a0, ra, a0
    37d4: faa42423     	sw	a0, -0x58(s0)
    37d8: e8c42583     	lw	a1, -0x174(s0)
    37dc: 00b54533     	xor	a0, a0, a1
    37e0: 01955593     	srli	a1, a0, 0x19
    37e4: 00751513     	slli	a0, a0, 0x7
    37e8: 00b56533     	or	a0, a0, a1
    37ec: f8a42423     	sw	a0, -0x78(s0)
    37f0: f5842503     	lw	a0, -0xa8(s0)
    37f4: 00aa8533     	add	a0, s5, a0
    37f8: f8a42c23     	sw	a0, -0x68(s0)
    37fc: eb442583     	lw	a1, -0x14c(s0)
    3800: 00b54533     	xor	a0, a0, a1
    3804: 01955593     	srli	a1, a0, 0x19
    3808: 00751513     	slli	a0, a0, 0x7
    380c: 00b56533     	or	a0, a0, a1
    3810: f6a42823     	sw	a0, -0x90(s0)
    3814: fb042503     	lw	a0, -0x50(s0)
    3818: 00a90533     	add	a0, s2, a0
    381c: f8a42a23     	sw	a0, -0x6c(s0)
    3820: eb042583     	lw	a1, -0x150(s0)
    3824: 00b54533     	xor	a0, a0, a1
    3828: 01955593     	srli	a1, a0, 0x19
    382c: 00751513     	slli	a0, a0, 0x7
    3830: 00b56533     	or	a0, a0, a1
    3834: f4a42e23     	sw	a0, -0xa4(s0)
    3838: f0e42423     	sw	a4, -0xf8(s0)
    383c: f4842503     	lw	a0, -0xb8(s0)
    3840: 00a70533     	add	a0, a4, a0
    3844: 00008713     	mv	a4, ra
    3848: f8a42223     	sw	a0, -0x7c(s0)
    384c: ee042583     	lw	a1, -0x120(s0)
    3850: 00b54533     	xor	a0, a0, a1
    3854: 01955593     	srli	a1, a0, 0x19
    3858: 00751513     	slli	a0, a0, 0x7
    385c: 00b56533     	or	a0, a0, a1
    3860: f6a42a23     	sw	a0, -0x8c(s0)
    3864: f1c42503     	lw	a0, -0xe4(s0)
    3868: 00a80533     	add	a0, a6, a0
    386c: faa42823     	sw	a0, -0x50(s0)
    3870: 01954533     	xor	a0, a0, s9
    3874: 01955593     	srli	a1, a0, 0x19
    3878: 00751513     	slli	a0, a0, 0x7
    387c: 00b56533     	or	a0, a0, a1
    3880: f4a42c23     	sw	a0, -0xa8(s0)
    3884: 000e0393     	mv	t2, t3
    3888: f1842503     	lw	a0, -0xe8(s0)
    388c: 00ae0533     	add	a0, t3, a0
    3890: f6a42623     	sw	a0, -0x94(s0)
    3894: 00f54533     	xor	a0, a0, a5
    3898: 000c0e13     	mv	t3, s8
    389c: 01955593     	srli	a1, a0, 0x19
    38a0: 00751513     	slli	a0, a0, 0x7
    38a4: 00b56533     	or	a0, a0, a1
    38a8: f0a42c23     	sw	a0, -0xe8(s0)
    38ac: f1742823     	sw	s7, -0xf0(s0)
    38b0: f1442503     	lw	a0, -0xec(s0)
    38b4: 00ab8533     	add	a0, s7, a0
    38b8: 00080c13     	mv	s8, a6
    38bc: f6a42c23     	sw	a0, -0x88(s0)
    38c0: eac42583     	lw	a1, -0x154(s0)
    38c4: 00b54533     	xor	a0, a0, a1
    38c8: 01955593     	srli	a1, a0, 0x19
    38cc: 00751513     	slli	a0, a0, 0x7
    38d0: 00b56533     	or	a0, a0, a1
    38d4: fca42223     	sw	a0, -0x3c(s0)
    38d8: f1f42e23     	sw	t6, -0xe4(s0)
    38dc: eec42503     	lw	a0, -0x114(s0)
    38e0: 00af8533     	add	a0, t6, a0
    38e4: faa42223     	sw	a0, -0x5c(s0)
    38e8: 01154533     	xor	a0, a0, a7
    38ec: 00098913     	mv	s2, s3
    38f0: 01955593     	srli	a1, a0, 0x19
    38f4: 00751513     	slli	a0, a0, 0x7
    38f8: 00b56533     	or	a0, a0, a1
    38fc: f6a42e23     	sw	a0, -0x84(s0)
    3900: ef842503     	lw	a0, -0x108(s0)
    3904: 000e8893     	mv	a7, t4
    3908: 00ae8533     	add	a0, t4, a0
    390c: f8a42e23     	sw	a0, -0x64(s0)
    3910: ea842583     	lw	a1, -0x158(s0)
    3914: 00b54533     	xor	a0, a0, a1
    3918: 01955593     	srli	a1, a0, 0x19
    391c: 00751513     	slli	a0, a0, 0x7
    3920: 00b56533     	or	a0, a0, a1
    3924: f6a42023     	sw	a0, -0xa0(s0)
    3928: ef442503     	lw	a0, -0x10c(s0)
    392c: 00ad8533     	add	a0, s11, a0
    3930: f8a42823     	sw	a0, -0x70(s0)
    3934: 01454533     	xor	a0, a0, s4
    3938: 01955593     	srli	a1, a0, 0x19
    393c: 00751513     	slli	a0, a0, 0x7
    3940: 00b56bb3     	or	s7, a0, a1
    3944: f0642a23     	sw	t1, -0xec(s0)
    3948: ef042503     	lw	a0, -0x110(s0)
    394c: 00a30533     	add	a0, t1, a0
    3950: f8a42623     	sw	a0, -0x74(s0)
    3954: 00c54533     	xor	a0, a0, a2
    3958: 000d8613     	mv	a2, s11
    395c: 01955593     	srli	a1, a0, 0x19
    3960: 00751513     	slli	a0, a0, 0x7
    3964: 00b56533     	or	a0, a0, a1
    3968: f4a42423     	sw	a0, -0xb8(s0)
    396c: efc42503     	lw	a0, -0x104(s0)
    3970: 00050463     	beqz	a0, 0x3978 <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7+0x137c>
    3974: e69fe06f     	j	0x27dc <<rand_core::block::BlockRng<R> as rand_core::RngCore>::next_u32::h9d26223c70ccbdc7+0x1e0>
    3978: ee042c23     	sw	zero, -0x108(s0)
    397c: e5842503     	lw	a0, -0x1a8(s0)
    3980: 00aa8fb3     	add	t6, s5, a0
    3984: e6442503     	lw	a0, -0x19c(s0)
    3988: 00a705b3     	add	a1, a4, a0
    398c: e5c42503     	lw	a0, -0x1a4(s0)
    3990: 00a38733     	add	a4, t2, a0
    3994: e6842503     	lw	a0, -0x198(s0)
    3998: 00ac0b33     	add	s6, s8, a0
    399c: e6042503     	lw	a0, -0x1a0(s0)
    39a0: 00a60533     	add	a0, a2, a0
    39a4: eea42e23     	sw	a0, -0x104(s0)
    39a8: e6c42503     	lw	a0, -0x194(s0)
    39ac: 00a888b3     	add	a7, a7, a0
    39b0: e8042503     	lw	a0, -0x180(s0)
    39b4: 06b52823     	sw	a1, 0x70(a0)
    39b8: 0b652823     	sw	s6, 0xb0(a0)
    39bc: 0f152823     	sw	a7, 0xf0(a0)
    39c0: e7042583     	lw	a1, -0x190(s0)
    39c4: 00b68633     	add	a2, a3, a1
    39c8: f0842783     	lw	a5, -0xf8(s0)
    39cc: 00b787b3     	add	a5, a5, a1
    39d0: f1c42683     	lw	a3, -0xe4(s0)
    39d4: 00b685b3     	add	a1, a3, a1
    39d8: f0b42e23     	sw	a1, -0xe4(s0)
    39dc: e7442583     	lw	a1, -0x18c(s0)
    39e0: 00b484b3     	add	s1, s1, a1
    39e4: f1042383     	lw	t2, -0xf0(s0)
    39e8: 00b383b3     	add	t2, t2, a1
    39ec: f1442803     	lw	a6, -0xec(s0)
    39f0: 00b805b3     	add	a1, a6, a1
    39f4: f0b42a23     	sw	a1, -0xec(s0)
    39f8: 07f52a23     	sw	t6, 0x74(a0)
    39fc: 06952c23     	sw	s1, 0x78(a0)
    3a00: 06c52e23     	sw	a2, 0x7c(a0)
    3a04: 0ae52a23     	sw	a4, 0xb4(a0)
    3a08: 0a752c23     	sw	t2, 0xb8(a0)
    3a0c: 0af52e23     	sw	a5, 0xbc(a0)
    3a10: e7c42583     	lw	a1, -0x184(s0)
    3a14: 00458613     	addi	a2, a1, 0x4
    3a18: 00b63733     	sltu	a4, a2, a1
    3a1c: e7842583     	lw	a1, -0x188(s0)
    3a20: 00e58733     	add	a4, a1, a4
    3a24: 6b2067b7     	lui	a5, 0x6b206
    3a28: 57478793     	addi	a5, a5, 0x574
    3a2c: 00fe0833     	add	a6, t3, a5
    3a30: f3442883     	lw	a7, -0xcc(s0)
    3a34: 00f888b3     	add	a7, a7, a5
    3a38: 000f0093     	mv	ra, t5
    3a3c: f3042303     	lw	t1, -0xd0(s0)
    3a40: 00f30333     	add	t1, t1, a5
    3a44: f2842583     	lw	a1, -0xd8(s0)
    3a48: 00f587b3     	add	a5, a1, a5
    3a4c: 796233b7     	lui	t2, 0x79623
    3a50: d3238393     	addi	t2, t2, -0x2ce
    3a54: f4442e03     	lw	t3, -0xbc(s0)
    3a58: 007e0e33     	add	t3, t3, t2
    3a5c: f0442e83     	lw	t4, -0xfc(s0)
    3a60: 007e8eb3     	add	t4, t4, t2
    3a64: f3c42f03     	lw	t5, -0xc4(s0)
    3a68: 007f0f33     	add	t5, t5, t2
    3a6c: f3842283     	lw	t0, -0xc8(s0)
    3a70: 007282b3     	add	t0, t0, t2
    3a74: 332063b7     	lui	t2, 0x33206
    3a78: 46e38393     	addi	t2, t2, 0x46e
    3a7c: 007d0fb3     	add	t6, s10, t2
    3a80: 007904b3     	add	s1, s2, t2
    3a84: f4c42903     	lw	s2, -0xb4(s0)
    3a88: 00790933     	add	s2, s2, t2
    3a8c: f4042583     	lw	a1, -0xc0(s0)
    3a90: 007583b3     	add	t2, a1, t2
    3a94: 617089b7     	lui	s3, 0x61708
    3a98: 86598993     	addi	s3, s3, -0x79b
    3a9c: fc042a03     	lw	s4, -0x40(s0)
    3aa0: 013a0a33     	add	s4, s4, s3
    3aa4: fbc42a83     	lw	s5, -0x44(s0)
    3aa8: 013a8ab3     	add	s5, s5, s3
    3aac: fb842b03     	lw	s6, -0x48(s0)
    3ab0: 013b0b33     	add	s6, s6, s3
    3ab4: f5442583     	lw	a1, -0xac(s0)
    3ab8: 013589b3     	add	s3, a1, s3
    3abc: 12852583     	lw	a1, 0x128(a0)
    3ac0: 12c52c03     	lw	s8, 0x12c(a0)
    3ac4: 13052d83     	lw	s11, 0x130(a0)
    3ac8: 13452d03     	lw	s10, 0x134(a0)
    3acc: 12c52423     	sw	a2, 0x128(a0)
    3ad0: 12e52623     	sw	a4, 0x12c(a0)
    3ad4: 01452023     	sw	s4, 0x0(a0)
    3ad8: 01f52223     	sw	t6, 0x4(a0)
    3adc: 01c52423     	sw	t3, 0x8(a0)
    3ae0: 01052623     	sw	a6, 0xc(a0)
    3ae4: 05552023     	sw	s5, 0x40(a0)
    3ae8: 04952223     	sw	s1, 0x44(a0)
    3aec: 05d52423     	sw	t4, 0x48(a0)
    3af0: 05152623     	sw	a7, 0x4c(a0)
    3af4: 09652023     	sw	s6, 0x80(a0)
    3af8: 09252223     	sw	s2, 0x84(a0)
    3afc: 09e52423     	sw	t5, 0x88(a0)
    3b00: 08652623     	sw	t1, 0x8c(a0)
    3b04: 0d352023     	sw	s3, 0xc0(a0)
    3b08: 0c752223     	sw	t2, 0xc4(a0)
    3b0c: 0c552423     	sw	t0, 0xc8(a0)
    3b10: 0cf52623     	sw	a5, 0xcc(a0)
    3b14: f5042603     	lw	a2, -0xb0(s0)
    3b18: 01a60633     	add	a2, a2, s10
    3b1c: f4c42a23     	sw	a2, -0xac(s0)
    3b20: 11452603     	lw	a2, 0x114(a0)
    3b24: f2c42703     	lw	a4, -0xd4(s0)
    3b28: 01b70733     	add	a4, a4, s11
    3b2c: f4e42823     	sw	a4, -0xb0(s0)
    3b30: f2442703     	lw	a4, -0xdc(s0)
    3b34: 01870733     	add	a4, a4, s8
    3b38: f4e42623     	sw	a4, -0xb4(s0)
    3b3c: f2042703     	lw	a4, -0xe0(s0)
    3b40: 00b705b3     	add	a1, a4, a1
    3b44: f4b42223     	sw	a1, -0xbc(s0)
    3b48: f0042903     	lw	s2, -0x100(s0)
    3b4c: 00c90933     	add	s2, s2, a2
    3b50: 11052683     	lw	a3, 0x110(a0)
    3b54: f7042703     	lw	a4, -0x90(s0)
    3b58: 00c70733     	add	a4, a4, a2
    3b5c: f6e42823     	sw	a4, -0x90(s0)
    3b60: f1842703     	lw	a4, -0xe8(s0)
    3b64: 00c70733     	add	a4, a4, a2
    3b68: fae42c23     	sw	a4, -0x48(s0)
    3b6c: 00cb8633     	add	a2, s7, a2
    3b70: fcc42023     	sw	a2, -0x40(s0)
    3b74: f6842e83     	lw	t4, -0x98(s0)
    3b78: 00de8eb3     	add	t4, t4, a3
    3b7c: 10c52383     	lw	t2, 0x10c(a0)
    3b80: f8842c03     	lw	s8, -0x78(s0)
    3b84: 00dc0c33     	add	s8, s8, a3
    3b88: f5842603     	lw	a2, -0xa8(s0)
    3b8c: 00d60633     	add	a2, a2, a3
    3b90: f8c42423     	sw	a2, -0x78(s0)
    3b94: f6042603     	lw	a2, -0xa0(s0)
    3b98: 00d60633     	add	a2, a2, a3
    3b9c: fac42e23     	sw	a2, -0x44(s0)
    3ba0: fc842f03     	lw	t5, -0x38(s0)
    3ba4: 007f0f33     	add	t5, t5, t2
    3ba8: 10852f83     	lw	t6, 0x108(a0)
    3bac: f0c42483     	lw	s1, -0xf4(s0)
    3bb0: 007484b3     	add	s1, s1, t2
    3bb4: f7442c83     	lw	s9, -0x8c(s0)
    3bb8: 007c8cb3     	add	s9, s9, t2
    3bbc: f7c42603     	lw	a2, -0x84(s0)
    3bc0: 00760633     	add	a2, a2, t2
    3bc4: fcc42423     	sw	a2, -0x38(s0)
    3bc8: 01f089b3     	add	s3, ra, t6
    3bcc: 12452a03     	lw	s4, 0x124(a0)
    3bd0: f5c42a83     	lw	s5, -0xa4(s0)
    3bd4: 01fa8ab3     	add	s5, s5, t6
    3bd8: fc442b03     	lw	s6, -0x3c(s0)
    3bdc: 01fb0b33     	add	s6, s6, t6
    3be0: f4842583     	lw	a1, -0xb8(s0)
    3be4: 01f585b3     	add	a1, a1, t6
    3be8: fcb42223     	sw	a1, -0x3c(s0)
    3bec: fa042d03     	lw	s10, -0x60(s0)
    3bf0: 01aa0d33     	add	s10, s4, s10
    3bf4: 12052d83     	lw	s11, 0x120(a0)
    3bf8: fa842083     	lw	ra, -0x58(s0)
    3bfc: 001a00b3     	add	ra, s4, ra
    3c00: fb042f83     	lw	t6, -0x50(s0)
    3c04: 01fa0bb3     	add	s7, s4, t6
    3c08: f9c42583     	lw	a1, -0x64(s0)
    3c0c: 00ba0a33     	add	s4, s4, a1
    3c10: f6442883     	lw	a7, -0x9c(s0)
    3c14: 011d88b3     	add	a7, s11, a7
    3c18: 11c52e03     	lw	t3, 0x11c(a0)
    3c1c: fac42283     	lw	t0, -0x54(s0)
    3c20: 005d8333     	add	t1, s11, t0
    3c24: f8442f83     	lw	t6, -0x7c(s0)
    3c28: 01fd8fb3     	add	t6, s11, t6
    3c2c: fa442583     	lw	a1, -0x5c(s0)
    3c30: 00bd8db3     	add	s11, s11, a1
    3c34: fb442783     	lw	a5, -0x4c(s0)
    3c38: 00fe07b3     	add	a5, t3, a5
    3c3c: 11852603     	lw	a2, 0x118(a0)
    3c40: f9442803     	lw	a6, -0x6c(s0)
    3c44: 010e0833     	add	a6, t3, a6
    3c48: f7842383     	lw	t2, -0x88(s0)
    3c4c: 007e03b3     	add	t2, t3, t2
    3c50: f8c42583     	lw	a1, -0x74(s0)
    3c54: 00be0e33     	add	t3, t3, a1
    3c58: f8042683     	lw	a3, -0x80(s0)
    3c5c: 00d606b3     	add	a3, a2, a3
    3c60: f9842703     	lw	a4, -0x68(s0)
    3c64: 00e60733     	add	a4, a2, a4
    3c68: f6c42283     	lw	t0, -0x94(s0)
    3c6c: 005602b3     	add	t0, a2, t0
    3c70: f9042583     	lw	a1, -0x70(s0)
    3c74: 00b605b3     	add	a1, a2, a1
    3c78: ef842603     	lw	a2, -0x108(s0)
    3c7c: 01352823     	sw	s3, 0x10(a0)
    3c80: 01e52a23     	sw	t5, 0x14(a0)
    3c84: 01d52c23     	sw	t4, 0x18(a0)
    3c88: 01252e23     	sw	s2, 0x1c(a0)
    3c8c: 02d52023     	sw	a3, 0x20(a0)
    3c90: 02f52223     	sw	a5, 0x24(a0)
    3c94: 03152423     	sw	a7, 0x28(a0)
    3c98: 03a52623     	sw	s10, 0x2c(a0)
    3c9c: f4442683     	lw	a3, -0xbc(s0)
    3ca0: 02d52823     	sw	a3, 0x30(a0)
    3ca4: f4c42683     	lw	a3, -0xb4(s0)
    3ca8: 02d52a23     	sw	a3, 0x34(a0)
    3cac: f5042683     	lw	a3, -0xb0(s0)
    3cb0: 02d52c23     	sw	a3, 0x38(a0)
    3cb4: f5442683     	lw	a3, -0xac(s0)
    3cb8: 02d52e23     	sw	a3, 0x3c(a0)
    3cbc: 05552823     	sw	s5, 0x50(a0)
    3cc0: 04952a23     	sw	s1, 0x54(a0)
    3cc4: 05852c23     	sw	s8, 0x58(a0)
    3cc8: f7042683     	lw	a3, -0x90(s0)
    3ccc: 04d52e23     	sw	a3, 0x5c(a0)
    3cd0: 06e52023     	sw	a4, 0x60(a0)
    3cd4: 07052223     	sw	a6, 0x64(a0)
    3cd8: 06652423     	sw	t1, 0x68(a0)
    3cdc: 06152623     	sw	ra, 0x6c(a0)
    3ce0: 09652823     	sw	s6, 0x90(a0)
    3ce4: 09952a23     	sw	s9, 0x94(a0)
    3ce8: f8842683     	lw	a3, -0x78(s0)
    3cec: 08d52c23     	sw	a3, 0x98(a0)
    3cf0: fb842683     	lw	a3, -0x48(s0)
    3cf4: 08d52e23     	sw	a3, 0x9c(a0)
    3cf8: 0a552023     	sw	t0, 0xa0(a0)
    3cfc: 0a752223     	sw	t2, 0xa4(a0)
    3d00: 0bf52423     	sw	t6, 0xa8(a0)
    3d04: 0b752623     	sw	s7, 0xac(a0)
    3d08: fc442683     	lw	a3, -0x3c(s0)
    3d0c: 0cd52823     	sw	a3, 0xd0(a0)
    3d10: fc842683     	lw	a3, -0x38(s0)
    3d14: 0cd52a23     	sw	a3, 0xd4(a0)
    3d18: fbc42683     	lw	a3, -0x44(s0)
    3d1c: 0cd52c23     	sw	a3, 0xd8(a0)
    3d20: fc042683     	lw	a3, -0x40(s0)
    3d24: 0cd52e23     	sw	a3, 0xdc(a0)
    3d28: 0eb52023     	sw	a1, 0xe0(a0)
    3d2c: 0fc52223     	sw	t3, 0xe4(a0)
    3d30: 0fb52423     	sw	s11, 0xe8(a0)
    3d34: 0f452623     	sw	s4, 0xec(a0)
    3d38: efc42583     	lw	a1, -0x104(s0)
    3d3c: 0eb52a23     	sw	a1, 0xf4(a0)
    3d40: f1442583     	lw	a1, -0xec(s0)
    3d44: 0eb52c23     	sw	a1, 0xf8(a0)
    3d48: f1c42583     	lw	a1, -0xe4(s0)
    3d4c: 0eb52e23     	sw	a1, 0xfc(a0)
    3d50: 1ac12083     	lw	ra, 0x1ac(sp)
    3d54: 1a812403     	lw	s0, 0x1a8(sp)
    3d58: 1a412483     	lw	s1, 0x1a4(sp)
    3d5c: 1a012903     	lw	s2, 0x1a0(sp)
    3d60: 19c12983     	lw	s3, 0x19c(sp)
    3d64: 19812a03     	lw	s4, 0x198(sp)
    3d68: 19412a83     	lw	s5, 0x194(sp)
    3d6c: 19012b03     	lw	s6, 0x190(sp)
    3d70: 18c12b83     	lw	s7, 0x18c(sp)
    3d74: 18812c03     	lw	s8, 0x188(sp)
    3d78: 18412c83     	lw	s9, 0x184(sp)
    3d7c: 18012d03     	lw	s10, 0x180(sp)
    3d80: 17c12d83     	lw	s11, 0x17c(sp)
    3d84: 1b010113     	addi	sp, sp, 0x1b0
    3d88: 00261593     	slli	a1, a2, 0x2
    3d8c: 00b505b3     	add	a1, a0, a1
    3d90: 0005a583     	lw	a1, 0x0(a1)
    3d94: 00160613     	addi	a2, a2, 0x1
    3d98: 10c52023     	sw	a2, 0x100(a0)
    3d9c: 00058513     	mv	a0, a1
    3da0: 00008067     	ret

00003da4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205>:
    3da4: 44060ce3     	beqz	a2, 0x49fc <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xc58>
    3da8: ed010113     	addi	sp, sp, -0x130
    3dac: 12112623     	sw	ra, 0x12c(sp)
    3db0: 12812423     	sw	s0, 0x128(sp)
    3db4: 12912223     	sw	s1, 0x124(sp)
    3db8: 13212023     	sw	s2, 0x120(sp)
    3dbc: 11312e23     	sw	s3, 0x11c(sp)
    3dc0: 11412c23     	sw	s4, 0x118(sp)
    3dc4: 11512a23     	sw	s5, 0x114(sp)
    3dc8: 11612823     	sw	s6, 0x110(sp)
    3dcc: 11712623     	sw	s7, 0x10c(sp)
    3dd0: 11812423     	sw	s8, 0x108(sp)
    3dd4: 11912223     	sw	s9, 0x104(sp)
    3dd8: 11a12023     	sw	s10, 0x100(sp)
    3ddc: 0fb12e23     	sw	s11, 0xfc(sp)
    3de0: 13010413     	addi	s0, sp, 0x130
    3de4: 00060a93     	mv	s5, a2
    3de8: 00058b13     	mv	s6, a1
    3dec: 00050493     	mv	s1, a0
    3df0: 10052903     	lw	s2, 0x100(a0)
    3df4: 00397513     	andi	a0, s2, 0x3
    3df8: 06050063     	beqz	a0, 0x3e58 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xb4>
    3dfc: 00400593     	li	a1, 0x4
    3e00: 40a585b3     	sub	a1, a1, a0
    3e04: 000a8a13     	mv	s4, s5
    3e08: 00bae463     	bltu	s5, a1, 0x3e10 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0x6c>
    3e0c: 00058a13     	mv	s4, a1
    3e10: 014b09b3     	add	s3, s6, s4
    3e14: 414a8ab3     	sub	s5, s5, s4
    3e18: fc042423     	sw	zero, -0x38(s0)
    3e1c: fc840593     	addi	a1, s0, -0x38
    3e20: 00a5e533     	or	a0, a1, a0
    3e24: 000b0593     	mv	a1, s6
    3e28: 000a0613     	mv	a2, s4
    3e2c: 145030ef     	jal	0x7770 <memcpy>
    3e30: ffc97513     	andi	a0, s2, -0x4
    3e34: 00a48533     	add	a0, s1, a0
    3e38: 00052583     	lw	a1, 0x0(a0)
    3e3c: fc842603     	lw	a2, -0x38(s0)
    3e40: 00b645b3     	xor	a1, a2, a1
    3e44: 00b52023     	sw	a1, 0x0(a0)
    3e48: 1004a903     	lw	s2, 0x100(s1)
    3e4c: 01490933     	add	s2, s2, s4
    3e50: 1124a023     	sw	s2, 0x100(s1)
    3e54: 00098b13     	mv	s6, s3
    3e58: 08800513     	li	a0, 0x88
    3e5c: 00a91863     	bne	s2, a0, 0x3e6c <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xc8>
    3e60: 1004a023     	sw	zero, 0x100(s1)
    3e64: 00048513     	mv	a0, s1
    3e68: 399000ef     	jal	0x4a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
    3e6c: 340a8ce3     	beqz	s5, 0x49c4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xc20>
    3e70: 1004a683     	lw	a3, 0x100(s1)
    3e74: 002ad513     	srli	a0, s5, 0x2
    3e78: 08800593     	li	a1, 0x88
    3e7c: 40d585b3     	sub	a1, a1, a3
    3e80: 0025d613     	srli	a2, a1, 0x2
    3e84: 00050593     	mv	a1, a0
    3e88: 00c56463     	bltu	a0, a2, 0x3e90 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xec>
    3e8c: 00060593     	mv	a1, a2
    3e90: 00259613     	slli	a2, a1, 0x2
    3e94: 00cb03b3     	add	t2, s6, a2
    3e98: ed542e23     	sw	s5, -0x124(s0)
    3e9c: 04058c63     	beqz	a1, 0x3ef4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0x150>
    3ea0: ffc6f693     	andi	a3, a3, -0x4
    3ea4: 00d486b3     	add	a3, s1, a3
    3ea8: 000b0713     	mv	a4, s6
    3eac: 00174783     	lbu	a5, 0x1(a4)
    3eb0: 00074803     	lbu	a6, 0x0(a4)
    3eb4: 00274883     	lbu	a7, 0x2(a4)
    3eb8: 00374283     	lbu	t0, 0x3(a4)
    3ebc: 00879793     	slli	a5, a5, 0x8
    3ec0: 0107e7b3     	or	a5, a5, a6
    3ec4: 0006a803     	lw	a6, 0x0(a3)
    3ec8: 00470313     	addi	t1, a4, 0x4
    3ecc: 01089893     	slli	a7, a7, 0x10
    3ed0: 01829293     	slli	t0, t0, 0x18
    3ed4: 0112e733     	or	a4, t0, a7
    3ed8: 00f76733     	or	a4, a4, a5
    3edc: 01074733     	xor	a4, a4, a6
    3ee0: 00e6a023     	sw	a4, 0x0(a3)
    3ee4: 00468693     	addi	a3, a3, 0x4
    3ee8: 00030713     	mv	a4, t1
    3eec: fc7310e3     	bne	t1, t2, 0x3eac <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0x108>
    3ef0: 1004a683     	lw	a3, 0x100(s1)
    3ef4: ee742423     	sw	t2, -0x118(s0)
    3ef8: ed642c23     	sw	s6, -0x128(s0)
    3efc: 40b50933     	sub	s2, a0, a1
    3f00: 00c68633     	add	a2, a3, a2
    3f04: 08800513     	li	a0, 0x88
    3f08: 10c4a023     	sw	a2, 0x100(s1)
    3f0c: 00a61863     	bne	a2, a0, 0x3f1c <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0x178>
    3f10: 1004a023     	sw	zero, 0x100(s1)
    3f14: 00048513     	mv	a0, s1
    3f18: 2e9000ef     	jal	0x4a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
    3f1c: f0f0f537     	lui	a0, 0xf0f0f
    3f20: 02200593     	li	a1, 0x22
    3f24: 0f150513     	addi	a0, a0, 0xf1
    3f28: 02a93533     	mulhu	a0, s2, a0
    3f2c: 00555613     	srli	a2, a0, 0x5
    3f30: fe057513     	andi	a0, a0, -0x20
    3f34: 00161693     	slli	a3, a2, 0x1
    3f38: 00d50533     	add	a0, a0, a3
    3f3c: 00361693     	slli	a3, a2, 0x3
    3f40: eed42223     	sw	a3, -0x11c(s0)
    3f44: 40a90533     	sub	a0, s2, a0
    3f48: eca42a23     	sw	a0, -0x12c(s0)
    3f4c: 00761613     	slli	a2, a2, 0x7
    3f50: eec42023     	sw	a2, -0x120(s0)
    3f54: ed242823     	sw	s2, -0x130(s0)
    3f58: 18b96ce3     	bltu	s2, a1, 0x48f0 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xb4c>
    3f5c: ee442503     	lw	a0, -0x11c(s0)
    3f60: ee042583     	lw	a1, -0x120(s0)
    3f64: 00a58533     	add	a0, a1, a0
    3f68: ee842d03     	lw	s10, -0x118(s0)
    3f6c: fca42023     	sw	a0, -0x40(s0)
    3f70: 000d4383     	lbu	t2, 0x0(s10)
    3f74: 001d4f83     	lbu	t6, 0x1(s10)
    3f78: 002d4583     	lbu	a1, 0x2(s10)
    3f7c: 003d4503     	lbu	a0, 0x3(s10)
    3f80: faa42c23     	sw	a0, -0x48(s0)
    3f84: 004d4503     	lbu	a0, 0x4(s10)
    3f88: faa42a23     	sw	a0, -0x4c(s0)
    3f8c: 005d4283     	lbu	t0, 0x5(s10)
    3f90: 006d4e03     	lbu	t3, 0x6(s10)
    3f94: 007d4e83     	lbu	t4, 0x7(s10)
    3f98: 008d4903     	lbu	s2, 0x8(s10)
    3f9c: 009d4c83     	lbu	s9, 0x9(s10)
    3fa0: 00ad4683     	lbu	a3, 0xa(s10)
    3fa4: 00bd4703     	lbu	a4, 0xb(s10)
    3fa8: 00cd4783     	lbu	a5, 0xc(s10)
    3fac: 00dd4f03     	lbu	t5, 0xd(s10)
    3fb0: 00ed4983     	lbu	s3, 0xe(s10)
    3fb4: 00fd4a03     	lbu	s4, 0xf(s10)
    3fb8: 010d4b03     	lbu	s6, 0x10(s10)
    3fbc: 011d4d83     	lbu	s11, 0x11(s10)
    3fc0: 012d4803     	lbu	a6, 0x12(s10)
    3fc4: 013d4883     	lbu	a7, 0x13(s10)
    3fc8: 014d4303     	lbu	t1, 0x14(s10)
    3fcc: 015d4a83     	lbu	s5, 0x15(s10)
    3fd0: 016d4b83     	lbu	s7, 0x16(s10)
    3fd4: 017d4c03     	lbu	s8, 0x17(s10)
    3fd8: 008f9f93     	slli	t6, t6, 0x8
    3fdc: 007fe533     	or	a0, t6, t2
    3fe0: faa42e23     	sw	a0, -0x44(s0)
    3fe4: 018d4083     	lbu	ra, 0x18(s10)
    3fe8: 019d4603     	lbu	a2, 0x19(s10)
    3fec: 01ad4383     	lbu	t2, 0x1a(s10)
    3ff0: 01bd4f83     	lbu	t6, 0x1b(s10)
    3ff4: 01059593     	slli	a1, a1, 0x10
    3ff8: fb842503     	lw	a0, -0x48(s0)
    3ffc: 01851513     	slli	a0, a0, 0x18
    4000: 00829293     	slli	t0, t0, 0x8
    4004: 010e1e13     	slli	t3, t3, 0x10
    4008: 018e9e93     	slli	t4, t4, 0x18
    400c: 008c9c93     	slli	s9, s9, 0x8
    4010: 00b56533     	or	a0, a0, a1
    4014: faa42c23     	sw	a0, -0x48(s0)
    4018: fb442503     	lw	a0, -0x4c(s0)
    401c: 00a2e533     	or	a0, t0, a0
    4020: faa42a23     	sw	a0, -0x4c(s0)
    4024: 01cee533     	or	a0, t4, t3
    4028: faa42823     	sw	a0, -0x50(s0)
    402c: 012ce933     	or	s2, s9, s2
    4030: 01cd4503     	lbu	a0, 0x1c(s10)
    4034: 01dd4583     	lbu	a1, 0x1d(s10)
    4038: 01ed4283     	lbu	t0, 0x1e(s10)
    403c: 01fd4e03     	lbu	t3, 0x1f(s10)
    4040: 01069693     	slli	a3, a3, 0x10
    4044: 01871713     	slli	a4, a4, 0x18
    4048: 008f1f13     	slli	t5, t5, 0x8
    404c: 01099993     	slli	s3, s3, 0x10
    4050: 018a1a13     	slli	s4, s4, 0x18
    4054: 008d9d93     	slli	s11, s11, 0x8
    4058: 00d76cb3     	or	s9, a4, a3
    405c: 00ff6f33     	or	t5, t5, a5
    4060: 013a69b3     	or	s3, s4, s3
    4064: 016dea33     	or	s4, s11, s6
    4068: 020d4683     	lbu	a3, 0x20(s10)
    406c: 021d4703     	lbu	a4, 0x21(s10)
    4070: 022d4783     	lbu	a5, 0x22(s10)
    4074: 023d4e83     	lbu	t4, 0x23(s10)
    4078: 01081813     	slli	a6, a6, 0x10
    407c: 01889893     	slli	a7, a7, 0x18
    4080: 008a9a93     	slli	s5, s5, 0x8
    4084: 010b9b93     	slli	s7, s7, 0x10
    4088: 018c1c13     	slli	s8, s8, 0x18
    408c: 00861613     	slli	a2, a2, 0x8
    4090: 0108edb3     	or	s11, a7, a6
    4094: 006aeab3     	or	s5, s5, t1
    4098: 017c6c33     	or	s8, s8, s7
    409c: 001660b3     	or	ra, a2, ra
    40a0: 024d4603     	lbu	a2, 0x24(s10)
    40a4: 025d4803     	lbu	a6, 0x25(s10)
    40a8: 026d4883     	lbu	a7, 0x26(s10)
    40ac: 027d4303     	lbu	t1, 0x27(s10)
    40b0: 01039393     	slli	t2, t2, 0x10
    40b4: 018f9f93     	slli	t6, t6, 0x18
    40b8: 00859593     	slli	a1, a1, 0x8
    40bc: 01029293     	slli	t0, t0, 0x10
    40c0: 018e1e13     	slli	t3, t3, 0x18
    40c4: 00871713     	slli	a4, a4, 0x8
    40c8: 007fe3b3     	or	t2, t6, t2
    40cc: fa742623     	sw	t2, -0x54(s0)
    40d0: 00a5e533     	or	a0, a1, a0
    40d4: faa42423     	sw	a0, -0x58(s0)
    40d8: 005e6e33     	or	t3, t3, t0
    40dc: 00d766b3     	or	a3, a4, a3
    40e0: fad42223     	sw	a3, -0x5c(s0)
    40e4: 028d4503     	lbu	a0, 0x28(s10)
    40e8: 029d4583     	lbu	a1, 0x29(s10)
    40ec: 02ad4683     	lbu	a3, 0x2a(s10)
    40f0: 02bd4703     	lbu	a4, 0x2b(s10)
    40f4: 01079793     	slli	a5, a5, 0x10
    40f8: 018e9e93     	slli	t4, t4, 0x18
    40fc: 00881813     	slli	a6, a6, 0x8
    4100: 01089893     	slli	a7, a7, 0x10
    4104: 01831313     	slli	t1, t1, 0x18
    4108: 00859593     	slli	a1, a1, 0x8
    410c: 00feeeb3     	or	t4, t4, a5
    4110: 00c86633     	or	a2, a6, a2
    4114: fac42023     	sw	a2, -0x60(s0)
    4118: 01136633     	or	a2, t1, a7
    411c: f8c42e23     	sw	a2, -0x64(s0)
    4120: 00a5e533     	or	a0, a1, a0
    4124: f8a42c23     	sw	a0, -0x68(s0)
    4128: 02cd4503     	lbu	a0, 0x2c(s10)
    412c: 02dd4583     	lbu	a1, 0x2d(s10)
    4130: 02ed4603     	lbu	a2, 0x2e(s10)
    4134: 02fd4783     	lbu	a5, 0x2f(s10)
    4138: 01069693     	slli	a3, a3, 0x10
    413c: 01871713     	slli	a4, a4, 0x18
    4140: 00859593     	slli	a1, a1, 0x8
    4144: 01061613     	slli	a2, a2, 0x10
    4148: 01879793     	slli	a5, a5, 0x18
    414c: 00d766b3     	or	a3, a4, a3
    4150: f8d42a23     	sw	a3, -0x6c(s0)
    4154: 00a5e533     	or	a0, a1, a0
    4158: f8a42823     	sw	a0, -0x70(s0)
    415c: 00c7e633     	or	a2, a5, a2
    4160: f8c42623     	sw	a2, -0x74(s0)
    4164: 031d4503     	lbu	a0, 0x31(s10)
    4168: 030d4583     	lbu	a1, 0x30(s10)
    416c: 032d4603     	lbu	a2, 0x32(s10)
    4170: 033d4683     	lbu	a3, 0x33(s10)
    4174: 00851513     	slli	a0, a0, 0x8
    4178: 00b56533     	or	a0, a0, a1
    417c: f8a42423     	sw	a0, -0x78(s0)
    4180: 01061613     	slli	a2, a2, 0x10
    4184: 01869693     	slli	a3, a3, 0x18
    4188: 00c6e633     	or	a2, a3, a2
    418c: f8c42223     	sw	a2, -0x7c(s0)
    4190: 035d4503     	lbu	a0, 0x35(s10)
    4194: 034d4583     	lbu	a1, 0x34(s10)
    4198: 036d4603     	lbu	a2, 0x36(s10)
    419c: 037d4683     	lbu	a3, 0x37(s10)
    41a0: 00851513     	slli	a0, a0, 0x8
    41a4: 00b56533     	or	a0, a0, a1
    41a8: f8a42023     	sw	a0, -0x80(s0)
    41ac: 01061613     	slli	a2, a2, 0x10
    41b0: 01869693     	slli	a3, a3, 0x18
    41b4: 00c6e633     	or	a2, a3, a2
    41b8: f6c42e23     	sw	a2, -0x84(s0)
    41bc: 039d4503     	lbu	a0, 0x39(s10)
    41c0: 038d4583     	lbu	a1, 0x38(s10)
    41c4: 03ad4603     	lbu	a2, 0x3a(s10)
    41c8: 03bd4683     	lbu	a3, 0x3b(s10)
    41cc: 00851513     	slli	a0, a0, 0x8
    41d0: 00b56533     	or	a0, a0, a1
    41d4: f6a42c23     	sw	a0, -0x88(s0)
    41d8: 01061613     	slli	a2, a2, 0x10
    41dc: 01869693     	slli	a3, a3, 0x18
    41e0: 00c6e633     	or	a2, a3, a2
    41e4: f6c42a23     	sw	a2, -0x8c(s0)
    41e8: 03dd4503     	lbu	a0, 0x3d(s10)
    41ec: 03cd4583     	lbu	a1, 0x3c(s10)
    41f0: 03ed4603     	lbu	a2, 0x3e(s10)
    41f4: 03fd4683     	lbu	a3, 0x3f(s10)
    41f8: 00851513     	slli	a0, a0, 0x8
    41fc: 00b56533     	or	a0, a0, a1
    4200: f6a42823     	sw	a0, -0x90(s0)
    4204: 01061613     	slli	a2, a2, 0x10
    4208: 01869693     	slli	a3, a3, 0x18
    420c: 00c6e633     	or	a2, a3, a2
    4210: f6c42623     	sw	a2, -0x94(s0)
    4214: 041d4503     	lbu	a0, 0x41(s10)
    4218: 040d4583     	lbu	a1, 0x40(s10)
    421c: 042d4603     	lbu	a2, 0x42(s10)
    4220: 043d4683     	lbu	a3, 0x43(s10)
    4224: 00851513     	slli	a0, a0, 0x8
    4228: 00b56533     	or	a0, a0, a1
    422c: f6a42423     	sw	a0, -0x98(s0)
    4230: 01061613     	slli	a2, a2, 0x10
    4234: 01869693     	slli	a3, a3, 0x18
    4238: 00c6e633     	or	a2, a3, a2
    423c: f6c42223     	sw	a2, -0x9c(s0)
    4240: 045d4503     	lbu	a0, 0x45(s10)
    4244: 044d4583     	lbu	a1, 0x44(s10)
    4248: 046d4603     	lbu	a2, 0x46(s10)
    424c: 047d4683     	lbu	a3, 0x47(s10)
    4250: 00851513     	slli	a0, a0, 0x8
    4254: 00b56533     	or	a0, a0, a1
    4258: f6a42023     	sw	a0, -0xa0(s0)
    425c: 01061613     	slli	a2, a2, 0x10
    4260: 01869693     	slli	a3, a3, 0x18
    4264: 00c6e633     	or	a2, a3, a2
    4268: f4c42e23     	sw	a2, -0xa4(s0)
    426c: 049d4503     	lbu	a0, 0x49(s10)
    4270: 048d4583     	lbu	a1, 0x48(s10)
    4274: 04ad4603     	lbu	a2, 0x4a(s10)
    4278: 04bd4683     	lbu	a3, 0x4b(s10)
    427c: 00851513     	slli	a0, a0, 0x8
    4280: 00b56533     	or	a0, a0, a1
    4284: f4a42c23     	sw	a0, -0xa8(s0)
    4288: 01061613     	slli	a2, a2, 0x10
    428c: 01869693     	slli	a3, a3, 0x18
    4290: 00c6e633     	or	a2, a3, a2
    4294: f4c42a23     	sw	a2, -0xac(s0)
    4298: 04dd4503     	lbu	a0, 0x4d(s10)
    429c: 04cd4583     	lbu	a1, 0x4c(s10)
    42a0: 04ed4603     	lbu	a2, 0x4e(s10)
    42a4: 04fd4683     	lbu	a3, 0x4f(s10)
    42a8: 00851513     	slli	a0, a0, 0x8
    42ac: 00b56533     	or	a0, a0, a1
    42b0: f4a42823     	sw	a0, -0xb0(s0)
    42b4: 01061613     	slli	a2, a2, 0x10
    42b8: 01869693     	slli	a3, a3, 0x18
    42bc: 00c6e633     	or	a2, a3, a2
    42c0: f4c42623     	sw	a2, -0xb4(s0)
    42c4: 051d4503     	lbu	a0, 0x51(s10)
    42c8: 050d4583     	lbu	a1, 0x50(s10)
    42cc: 052d4603     	lbu	a2, 0x52(s10)
    42d0: 053d4703     	lbu	a4, 0x53(s10)
    42d4: 00851513     	slli	a0, a0, 0x8
    42d8: 00b56533     	or	a0, a0, a1
    42dc: f4a42423     	sw	a0, -0xb8(s0)
    42e0: 01061613     	slli	a2, a2, 0x10
    42e4: 01871713     	slli	a4, a4, 0x18
    42e8: 00c76633     	or	a2, a4, a2
    42ec: f4c42223     	sw	a2, -0xbc(s0)
    42f0: 055d4503     	lbu	a0, 0x55(s10)
    42f4: 054d4583     	lbu	a1, 0x54(s10)
    42f8: 056d4603     	lbu	a2, 0x56(s10)
    42fc: 057d4703     	lbu	a4, 0x57(s10)
    4300: 00851513     	slli	a0, a0, 0x8
    4304: 00b56533     	or	a0, a0, a1
    4308: f4a42023     	sw	a0, -0xc0(s0)
    430c: 01061613     	slli	a2, a2, 0x10
    4310: 01871713     	slli	a4, a4, 0x18
    4314: 00c76633     	or	a2, a4, a2
    4318: f2c42e23     	sw	a2, -0xc4(s0)
    431c: 059d4503     	lbu	a0, 0x59(s10)
    4320: 058d4583     	lbu	a1, 0x58(s10)
    4324: 05ad4603     	lbu	a2, 0x5a(s10)
    4328: 05bd4703     	lbu	a4, 0x5b(s10)
    432c: 00851513     	slli	a0, a0, 0x8
    4330: 00b56533     	or	a0, a0, a1
    4334: f2a42c23     	sw	a0, -0xc8(s0)
    4338: 01061613     	slli	a2, a2, 0x10
    433c: 01871593     	slli	a1, a4, 0x18
    4340: 00c5e5b3     	or	a1, a1, a2
    4344: f2b42a23     	sw	a1, -0xcc(s0)
    4348: 05dd4503     	lbu	a0, 0x5d(s10)
    434c: 05cd4603     	lbu	a2, 0x5c(s10)
    4350: 05ed4703     	lbu	a4, 0x5e(s10)
    4354: 05fd4783     	lbu	a5, 0x5f(s10)
    4358: 00851513     	slli	a0, a0, 0x8
    435c: 00c56533     	or	a0, a0, a2
    4360: f2a42823     	sw	a0, -0xd0(s0)
    4364: 01071713     	slli	a4, a4, 0x10
    4368: 01879793     	slli	a5, a5, 0x18
    436c: 00e7e733     	or	a4, a5, a4
    4370: f2e42623     	sw	a4, -0xd4(s0)
    4374: 061d4503     	lbu	a0, 0x61(s10)
    4378: 060d4703     	lbu	a4, 0x60(s10)
    437c: 062d4803     	lbu	a6, 0x62(s10)
    4380: 063d4283     	lbu	t0, 0x63(s10)
    4384: 00851513     	slli	a0, a0, 0x8
    4388: 00e56533     	or	a0, a0, a4
    438c: f2a42423     	sw	a0, -0xd8(s0)
    4390: 01081813     	slli	a6, a6, 0x10
    4394: 01829293     	slli	t0, t0, 0x18
    4398: 0102e533     	or	a0, t0, a6
    439c: f2a42223     	sw	a0, -0xdc(s0)
    43a0: 065d4503     	lbu	a0, 0x65(s10)
    43a4: 064d4703     	lbu	a4, 0x64(s10)
    43a8: 066d4803     	lbu	a6, 0x66(s10)
    43ac: 067d4283     	lbu	t0, 0x67(s10)
    43b0: 00851513     	slli	a0, a0, 0x8
    43b4: 00e56533     	or	a0, a0, a4
    43b8: f2a42023     	sw	a0, -0xe0(s0)
    43bc: 01081813     	slli	a6, a6, 0x10
    43c0: 01829293     	slli	t0, t0, 0x18
    43c4: 0102e533     	or	a0, t0, a6
    43c8: f0a42e23     	sw	a0, -0xe4(s0)
    43cc: 069d4503     	lbu	a0, 0x69(s10)
    43d0: 068d4703     	lbu	a4, 0x68(s10)
    43d4: 06ad4803     	lbu	a6, 0x6a(s10)
    43d8: 06bd4283     	lbu	t0, 0x6b(s10)
    43dc: 00851513     	slli	a0, a0, 0x8
    43e0: 00e56533     	or	a0, a0, a4
    43e4: f0a42c23     	sw	a0, -0xe8(s0)
    43e8: 01081813     	slli	a6, a6, 0x10
    43ec: 01829293     	slli	t0, t0, 0x18
    43f0: 0102e533     	or	a0, t0, a6
    43f4: f0a42a23     	sw	a0, -0xec(s0)
    43f8: 06dd4503     	lbu	a0, 0x6d(s10)
    43fc: 06cd4703     	lbu	a4, 0x6c(s10)
    4400: 06ed4803     	lbu	a6, 0x6e(s10)
    4404: 06fd4283     	lbu	t0, 0x6f(s10)
    4408: 00851513     	slli	a0, a0, 0x8
    440c: 00e56533     	or	a0, a0, a4
    4410: f0a42823     	sw	a0, -0xf0(s0)
    4414: 01081813     	slli	a6, a6, 0x10
    4418: 01829293     	slli	t0, t0, 0x18
    441c: 0102e533     	or	a0, t0, a6
    4420: f0a42623     	sw	a0, -0xf4(s0)
    4424: 071d4503     	lbu	a0, 0x71(s10)
    4428: 070d4703     	lbu	a4, 0x70(s10)
    442c: 072d4803     	lbu	a6, 0x72(s10)
    4430: 073d4283     	lbu	t0, 0x73(s10)
    4434: 00851513     	slli	a0, a0, 0x8
    4438: 00e56533     	or	a0, a0, a4
    443c: f0a42423     	sw	a0, -0xf8(s0)
    4440: 01081813     	slli	a6, a6, 0x10
    4444: 01829293     	slli	t0, t0, 0x18
    4448: 0102e533     	or	a0, t0, a6
    444c: f0a42223     	sw	a0, -0xfc(s0)
    4450: 075d4503     	lbu	a0, 0x75(s10)
    4454: 074d4703     	lbu	a4, 0x74(s10)
    4458: 076d4283     	lbu	t0, 0x76(s10)
    445c: 077d4303     	lbu	t1, 0x77(s10)
    4460: 00851513     	slli	a0, a0, 0x8
    4464: 00e56533     	or	a0, a0, a4
    4468: f0a42023     	sw	a0, -0x100(s0)
    446c: 01029293     	slli	t0, t0, 0x10
    4470: 01831313     	slli	t1, t1, 0x18
    4474: 00536533     	or	a0, t1, t0
    4478: eea42e23     	sw	a0, -0x104(s0)
    447c: 079d4503     	lbu	a0, 0x79(s10)
    4480: 078d4283     	lbu	t0, 0x78(s10)
    4484: 07ad4303     	lbu	t1, 0x7a(s10)
    4488: 07bd4f83     	lbu	t6, 0x7b(s10)
    448c: 00851513     	slli	a0, a0, 0x8
    4490: 00556533     	or	a0, a0, t0
    4494: eea42c23     	sw	a0, -0x108(s0)
    4498: 01031313     	slli	t1, t1, 0x10
    449c: 018f9f93     	slli	t6, t6, 0x18
    44a0: 006fe533     	or	a0, t6, t1
    44a4: eea42a23     	sw	a0, -0x10c(s0)
    44a8: 07dd4283     	lbu	t0, 0x7d(s10)
    44ac: 07cd4f83     	lbu	t6, 0x7c(s10)
    44b0: 07ed4583     	lbu	a1, 0x7e(s10)
    44b4: 07fd4503     	lbu	a0, 0x7f(s10)
    44b8: 00829293     	slli	t0, t0, 0x8
    44bc: 01f2e633     	or	a2, t0, t6
    44c0: eec42823     	sw	a2, -0x110(s0)
    44c4: 01059593     	slli	a1, a1, 0x10
    44c8: 01851513     	slli	a0, a0, 0x18
    44cc: 00b56533     	or	a0, a0, a1
    44d0: eea42623     	sw	a0, -0x114(s0)
    44d4: 081d4603     	lbu	a2, 0x81(s10)
    44d8: 080d4683     	lbu	a3, 0x80(s10)
    44dc: 082d4583     	lbu	a1, 0x82(s10)
    44e0: 083d4503     	lbu	a0, 0x83(s10)
    44e4: 00861613     	slli	a2, a2, 0x8
    44e8: 00d66bb3     	or	s7, a2, a3
    44ec: 01059593     	slli	a1, a1, 0x10
    44f0: 01851513     	slli	a0, a0, 0x18
    44f4: 00b56b33     	or	s6, a0, a1
    44f8: 085d4583     	lbu	a1, 0x85(s10)
    44fc: 084d4683     	lbu	a3, 0x84(s10)
    4500: 086d4703     	lbu	a4, 0x86(s10)
    4504: 087d4503     	lbu	a0, 0x87(s10)
    4508: 00859593     	slli	a1, a1, 0x8
    450c: 00d5e5b3     	or	a1, a1, a3
    4510: 01071713     	slli	a4, a4, 0x10
    4514: 01851513     	slli	a0, a0, 0x18
    4518: 00e56533     	or	a0, a0, a4
    451c: fbc42603     	lw	a2, -0x44(s0)
    4520: fb842683     	lw	a3, -0x48(s0)
    4524: 00c6e633     	or	a2, a3, a2
    4528: fb442683     	lw	a3, -0x4c(s0)
    452c: fb042703     	lw	a4, -0x50(s0)
    4530: 00d766b3     	or	a3, a4, a3
    4534: 012ce833     	or	a6, s9, s2
    4538: 01e9e7b3     	or	a5, s3, t5
    453c: 014de3b3     	or	t2, s11, s4
    4540: 015c68b3     	or	a7, s8, s5
    4544: fac42703     	lw	a4, -0x54(s0)
    4548: 001762b3     	or	t0, a4, ra
    454c: fa842703     	lw	a4, -0x58(s0)
    4550: 00ee6333     	or	t1, t3, a4
    4554: fa442703     	lw	a4, -0x5c(s0)
    4558: 00eeefb3     	or	t6, t4, a4
    455c: fa042703     	lw	a4, -0x60(s0)
    4560: f9c42e03     	lw	t3, -0x64(s0)
    4564: 00ee6e33     	or	t3, t3, a4
    4568: f9842703     	lw	a4, -0x68(s0)
    456c: f9442e83     	lw	t4, -0x6c(s0)
    4570: 00eeeeb3     	or	t4, t4, a4
    4574: f9042703     	lw	a4, -0x70(s0)
    4578: f8c42f03     	lw	t5, -0x74(s0)
    457c: 00ef6f33     	or	t5, t5, a4
    4580: f8842703     	lw	a4, -0x78(s0)
    4584: f8442903     	lw	s2, -0x7c(s0)
    4588: 00e96933     	or	s2, s2, a4
    458c: f8042703     	lw	a4, -0x80(s0)
    4590: f7c42983     	lw	s3, -0x84(s0)
    4594: 00e9e9b3     	or	s3, s3, a4
    4598: f7842703     	lw	a4, -0x88(s0)
    459c: f7442a03     	lw	s4, -0x8c(s0)
    45a0: 00ea6a33     	or	s4, s4, a4
    45a4: f7042703     	lw	a4, -0x90(s0)
    45a8: f6c42a83     	lw	s5, -0x94(s0)
    45ac: 00eaeab3     	or	s5, s5, a4
    45b0: f6842703     	lw	a4, -0x98(s0)
    45b4: f6442c03     	lw	s8, -0x9c(s0)
    45b8: 00ec60b3     	or	ra, s8, a4
    45bc: f6042703     	lw	a4, -0xa0(s0)
    45c0: f5c42c03     	lw	s8, -0xa4(s0)
    45c4: 00ec6733     	or	a4, s8, a4
    45c8: f4e42e23     	sw	a4, -0xa4(s0)
    45cc: f5842703     	lw	a4, -0xa8(s0)
    45d0: f5442c83     	lw	s9, -0xac(s0)
    45d4: 00ececb3     	or	s9, s9, a4
    45d8: f5042703     	lw	a4, -0xb0(s0)
    45dc: f4c42d83     	lw	s11, -0xb4(s0)
    45e0: 00ededb3     	or	s11, s11, a4
    45e4: f4842703     	lw	a4, -0xb8(s0)
    45e8: f4442c03     	lw	s8, -0xbc(s0)
    45ec: 00ec6733     	or	a4, s8, a4
    45f0: f6e42a23     	sw	a4, -0x8c(s0)
    45f4: f4042703     	lw	a4, -0xc0(s0)
    45f8: f3c42c03     	lw	s8, -0xc4(s0)
    45fc: 00ec6733     	or	a4, s8, a4
    4600: f6e42823     	sw	a4, -0x90(s0)
    4604: f3842703     	lw	a4, -0xc8(s0)
    4608: f3442c03     	lw	s8, -0xcc(s0)
    460c: 00ec6733     	or	a4, s8, a4
    4610: f6e42623     	sw	a4, -0x94(s0)
    4614: f3042703     	lw	a4, -0xd0(s0)
    4618: f2c42c03     	lw	s8, -0xd4(s0)
    461c: 00ec6733     	or	a4, s8, a4
    4620: f6e42423     	sw	a4, -0x98(s0)
    4624: f2842703     	lw	a4, -0xd8(s0)
    4628: f2442c03     	lw	s8, -0xdc(s0)
    462c: 00ec6733     	or	a4, s8, a4
    4630: f8e42a23     	sw	a4, -0x6c(s0)
    4634: f2042703     	lw	a4, -0xe0(s0)
    4638: f1c42c03     	lw	s8, -0xe4(s0)
    463c: 00ec6733     	or	a4, s8, a4
    4640: f8e42823     	sw	a4, -0x70(s0)
    4644: f1842703     	lw	a4, -0xe8(s0)
    4648: f1442c03     	lw	s8, -0xec(s0)
    464c: 00ec6733     	or	a4, s8, a4
    4650: f8e42623     	sw	a4, -0x74(s0)
    4654: f1042703     	lw	a4, -0xf0(s0)
    4658: f0c42c03     	lw	s8, -0xf4(s0)
    465c: 00ec6733     	or	a4, s8, a4
    4660: f8e42423     	sw	a4, -0x78(s0)
    4664: f0842703     	lw	a4, -0xf8(s0)
    4668: f0442c03     	lw	s8, -0xfc(s0)
    466c: 00ec6733     	or	a4, s8, a4
    4670: fae42623     	sw	a4, -0x54(s0)
    4674: f0042703     	lw	a4, -0x100(s0)
    4678: efc42c03     	lw	s8, -0x104(s0)
    467c: 00ec6733     	or	a4, s8, a4
    4680: fae42423     	sw	a4, -0x58(s0)
    4684: ef842703     	lw	a4, -0x108(s0)
    4688: ef442c03     	lw	s8, -0x10c(s0)
    468c: 00ec6733     	or	a4, s8, a4
    4690: fae42e23     	sw	a4, -0x44(s0)
    4694: ef042703     	lw	a4, -0x110(s0)
    4698: eec42c03     	lw	s8, -0x114(s0)
    469c: 00ec6733     	or	a4, s8, a4
    46a0: fae42c23     	sw	a4, -0x48(s0)
    46a4: 017b6733     	or	a4, s6, s7
    46a8: fae42a23     	sw	a4, -0x4c(s0)
    46ac: 00b56533     	or	a0, a0, a1
    46b0: faa42823     	sw	a0, -0x50(s0)
    46b4: 0004a583     	lw	a1, 0x0(s1)
    46b8: 0044a503     	lw	a0, 0x4(s1)
    46bc: 0084ab03     	lw	s6, 0x8(s1)
    46c0: 00c4a703     	lw	a4, 0xc(s1)
    46c4: 00c5c5b3     	xor	a1, a1, a2
    46c8: fab42223     	sw	a1, -0x5c(s0)
    46cc: 00d54533     	xor	a0, a0, a3
    46d0: faa42023     	sw	a0, -0x60(s0)
    46d4: 010b4533     	xor	a0, s6, a6
    46d8: f8a42e23     	sw	a0, -0x64(s0)
    46dc: 00f74733     	xor	a4, a4, a5
    46e0: f8e42c23     	sw	a4, -0x68(s0)
    46e4: 0104a503     	lw	a0, 0x10(s1)
    46e8: 0144a803     	lw	a6, 0x14(s1)
    46ec: 0184ab03     	lw	s6, 0x18(s1)
    46f0: 01c4ab83     	lw	s7, 0x1c(s1)
    46f4: 00754533     	xor	a0, a0, t2
    46f8: f8a42223     	sw	a0, -0x7c(s0)
    46fc: 01184533     	xor	a0, a6, a7
    4700: f8a42023     	sw	a0, -0x80(s0)
    4704: 005b4533     	xor	a0, s6, t0
    4708: f6a42e23     	sw	a0, -0x84(s0)
    470c: 006bc533     	xor	a0, s7, t1
    4710: f6a42c23     	sw	a0, -0x88(s0)
    4714: 0204a503     	lw	a0, 0x20(s1)
    4718: 0244a383     	lw	t2, 0x24(s1)
    471c: 0284ab03     	lw	s6, 0x28(s1)
    4720: 02c4ab83     	lw	s7, 0x2c(s1)
    4724: 01f54533     	xor	a0, a0, t6
    4728: f6a42223     	sw	a0, -0x9c(s0)
    472c: 01c3c3b3     	xor	t2, t2, t3
    4730: 01db4e33     	xor	t3, s6, t4
    4734: 01ebc533     	xor	a0, s7, t5
    4738: f6a42023     	sw	a0, -0xa0(s0)
    473c: 0304ae83     	lw	t4, 0x30(s1)
    4740: 0344af03     	lw	t5, 0x34(s1)
    4744: 0384af83     	lw	t6, 0x38(s1)
    4748: 03c4ab03     	lw	s6, 0x3c(s1)
    474c: 012eceb3     	xor	t4, t4, s2
    4750: 013f4f33     	xor	t5, t5, s3
    4754: 014fcfb3     	xor	t6, t6, s4
    4758: 015b4933     	xor	s2, s6, s5
    475c: 0404a983     	lw	s3, 0x40(s1)
    4760: 0444aa03     	lw	s4, 0x44(s1)
    4764: 0484aa83     	lw	s5, 0x48(s1)
    4768: 04c4ab03     	lw	s6, 0x4c(s1)
    476c: 0019c9b3     	xor	s3, s3, ra
    4770: f5c42503     	lw	a0, -0xa4(s0)
    4774: 00aa4a33     	xor	s4, s4, a0
    4778: 019acab3     	xor	s5, s5, s9
    477c: 01bb4b33     	xor	s6, s6, s11
    4780: 0504ab83     	lw	s7, 0x50(s1)
    4784: 0544ac03     	lw	s8, 0x54(s1)
    4788: 0584ac83     	lw	s9, 0x58(s1)
    478c: 05c4ad83     	lw	s11, 0x5c(s1)
    4790: f7442503     	lw	a0, -0x8c(s0)
    4794: 00abcbb3     	xor	s7, s7, a0
    4798: f7042503     	lw	a0, -0x90(s0)
    479c: 00ac4c33     	xor	s8, s8, a0
    47a0: f6c42503     	lw	a0, -0x94(s0)
    47a4: 00acccb3     	xor	s9, s9, a0
    47a8: f6842503     	lw	a0, -0x98(s0)
    47ac: 00adcdb3     	xor	s11, s11, a0
    47b0: 0604a083     	lw	ra, 0x60(s1)
    47b4: 0644a503     	lw	a0, 0x64(s1)
    47b8: 0684a583     	lw	a1, 0x68(s1)
    47bc: 06c4a603     	lw	a2, 0x6c(s1)
    47c0: f9442683     	lw	a3, -0x6c(s0)
    47c4: 00d0c0b3     	xor	ra, ra, a3
    47c8: f9042683     	lw	a3, -0x70(s0)
    47cc: 00d54333     	xor	t1, a0, a3
    47d0: f8c42503     	lw	a0, -0x74(s0)
    47d4: 00a5c8b3     	xor	a7, a1, a0
    47d8: f8842503     	lw	a0, -0x78(s0)
    47dc: 00a642b3     	xor	t0, a2, a0
    47e0: 0704a503     	lw	a0, 0x70(s1)
    47e4: 0744a583     	lw	a1, 0x74(s1)
    47e8: 0784a703     	lw	a4, 0x78(s1)
    47ec: 07c4a683     	lw	a3, 0x7c(s1)
    47f0: fac42603     	lw	a2, -0x54(s0)
    47f4: 00c54833     	xor	a6, a0, a2
    47f8: fa842783     	lw	a5, -0x58(s0)
    47fc: 00f5c7b3     	xor	a5, a1, a5
    4800: 0804a503     	lw	a0, 0x80(s1)
    4804: 0844a583     	lw	a1, 0x84(s1)
    4808: fbc42603     	lw	a2, -0x44(s0)
    480c: 00c74633     	xor	a2, a4, a2
    4810: fb842703     	lw	a4, -0x48(s0)
    4814: 00e6c6b3     	xor	a3, a3, a4
    4818: fb442703     	lw	a4, -0x4c(s0)
    481c: 00e54533     	xor	a0, a0, a4
    4820: fb042703     	lw	a4, -0x50(s0)
    4824: 00e5c5b3     	xor	a1, a1, a4
    4828: fa442703     	lw	a4, -0x5c(s0)
    482c: 00e4a023     	sw	a4, 0x0(s1)
    4830: fa042703     	lw	a4, -0x60(s0)
    4834: 00e4a223     	sw	a4, 0x4(s1)
    4838: f9c42703     	lw	a4, -0x64(s0)
    483c: 00e4a423     	sw	a4, 0x8(s1)
    4840: f9842703     	lw	a4, -0x68(s0)
    4844: 00e4a623     	sw	a4, 0xc(s1)
    4848: f8442703     	lw	a4, -0x7c(s0)
    484c: 00e4a823     	sw	a4, 0x10(s1)
    4850: f8042703     	lw	a4, -0x80(s0)
    4854: 00e4aa23     	sw	a4, 0x14(s1)
    4858: f7c42703     	lw	a4, -0x84(s0)
    485c: 00e4ac23     	sw	a4, 0x18(s1)
    4860: f7842703     	lw	a4, -0x88(s0)
    4864: 00e4ae23     	sw	a4, 0x1c(s1)
    4868: f6442703     	lw	a4, -0x9c(s0)
    486c: 02e4a023     	sw	a4, 0x20(s1)
    4870: 0274a223     	sw	t2, 0x24(s1)
    4874: 03c4a423     	sw	t3, 0x28(s1)
    4878: f6042703     	lw	a4, -0xa0(s0)
    487c: 02e4a623     	sw	a4, 0x2c(s1)
    4880: 03d4a823     	sw	t4, 0x30(s1)
    4884: 03e4aa23     	sw	t5, 0x34(s1)
    4888: 03f4ac23     	sw	t6, 0x38(s1)
    488c: 0324ae23     	sw	s2, 0x3c(s1)
    4890: 0534a023     	sw	s3, 0x40(s1)
    4894: 0544a223     	sw	s4, 0x44(s1)
    4898: 0554a423     	sw	s5, 0x48(s1)
    489c: 0564a623     	sw	s6, 0x4c(s1)
    48a0: 0574a823     	sw	s7, 0x50(s1)
    48a4: 0584aa23     	sw	s8, 0x54(s1)
    48a8: 0594ac23     	sw	s9, 0x58(s1)
    48ac: 05b4ae23     	sw	s11, 0x5c(s1)
    48b0: 0614a023     	sw	ra, 0x60(s1)
    48b4: 0664a223     	sw	t1, 0x64(s1)
    48b8: 0714a423     	sw	a7, 0x68(s1)
    48bc: 0654a623     	sw	t0, 0x6c(s1)
    48c0: 0704a823     	sw	a6, 0x70(s1)
    48c4: 06f4aa23     	sw	a5, 0x74(s1)
    48c8: 06c4ac23     	sw	a2, 0x78(s1)
    48cc: 06d4ae23     	sw	a3, 0x7c(s1)
    48d0: 08a4a023     	sw	a0, 0x80(s1)
    48d4: 08b4a223     	sw	a1, 0x84(s1)
    48d8: 00048513     	mv	a0, s1
    48dc: 124000ef     	jal	0x4a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
    48e0: fc042503     	lw	a0, -0x40(s0)
    48e4: f7850513     	addi	a0, a0, -0x88
    48e8: 088d0d13     	addi	s10, s10, 0x88
    48ec: e8051063     	bnez	a0, 0x3f6c <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0x1c8>
    48f0: edc42883     	lw	a7, -0x124(s0)
    48f4: 0038fa13     	andi	s4, a7, 0x3
    48f8: ed442283     	lw	t0, -0x12c(s0)
    48fc: 06028a63     	beqz	t0, 0x4970 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xbcc>
    4900: ed042583     	lw	a1, -0x130(s0)
    4904: 40558533     	sub	a0, a1, t0
    4908: 00259593     	slli	a1, a1, 0x2
    490c: ee442603     	lw	a2, -0x11c(s0)
    4910: ee042683     	lw	a3, -0x120(s0)
    4914: 00c68633     	add	a2, a3, a2
    4918: 00251513     	slli	a0, a0, 0x2
    491c: 40c585b3     	sub	a1, a1, a2
    4920: ee842603     	lw	a2, -0x118(s0)
    4924: 00a60633     	add	a2, a2, a0
    4928: 00b48533     	add	a0, s1, a1
    492c: 00048593     	mv	a1, s1
    4930: 00164683     	lbu	a3, 0x1(a2)
    4934: 00064703     	lbu	a4, 0x0(a2)
    4938: 00264783     	lbu	a5, 0x2(a2)
    493c: 00364803     	lbu	a6, 0x3(a2)
    4940: 00869693     	slli	a3, a3, 0x8
    4944: 00e6e6b3     	or	a3, a3, a4
    4948: 0005a703     	lw	a4, 0x0(a1)
    494c: 00460613     	addi	a2, a2, 0x4
    4950: 01079793     	slli	a5, a5, 0x10
    4954: 01881813     	slli	a6, a6, 0x18
    4958: 00f867b3     	or	a5, a6, a5
    495c: 00d7e6b3     	or	a3, a5, a3
    4960: 00e6c6b3     	xor	a3, a3, a4
    4964: 00d5a023     	sw	a3, 0x0(a1)
    4968: 00458593     	addi	a1, a1, 0x4
    496c: fca592e3     	bne	a1, a0, 0x4930 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xb8c>
    4970: 1004a903     	lw	s2, 0x100(s1)
    4974: 00229513     	slli	a0, t0, 0x2
    4978: 00a90933     	add	s2, s2, a0
    497c: 1124a023     	sw	s2, 0x100(s1)
    4980: ed842503     	lw	a0, -0x128(s0)
    4984: 040a0063     	beqz	s4, 0x49c4 <<crypto::sha3::delegated::Keccak256Core<_> as crypto::MiniDigest>::update::hd9dcb4c995169205+0xc20>
    4988: ffc8f593     	andi	a1, a7, -0x4
    498c: 00b505b3     	add	a1, a0, a1
    4990: fc042223     	sw	zero, -0x3c(s0)
    4994: fc440513     	addi	a0, s0, -0x3c
    4998: 000a0613     	mv	a2, s4
    499c: 5d5020ef     	jal	0x7770 <memcpy>
    49a0: ffc97513     	andi	a0, s2, -0x4
    49a4: 00a48533     	add	a0, s1, a0
    49a8: 00052583     	lw	a1, 0x0(a0)
    49ac: fc442603     	lw	a2, -0x3c(s0)
    49b0: 00b645b3     	xor	a1, a2, a1
    49b4: 00b52023     	sw	a1, 0x0(a0)
    49b8: 1004a503     	lw	a0, 0x100(s1)
    49bc: 01450533     	add	a0, a0, s4
    49c0: 10a4a023     	sw	a0, 0x100(s1)
    49c4: 12c12083     	lw	ra, 0x12c(sp)
    49c8: 12812403     	lw	s0, 0x128(sp)
    49cc: 12412483     	lw	s1, 0x124(sp)
    49d0: 12012903     	lw	s2, 0x120(sp)
    49d4: 11c12983     	lw	s3, 0x11c(sp)
    49d8: 11812a03     	lw	s4, 0x118(sp)
    49dc: 11412a83     	lw	s5, 0x114(sp)
    49e0: 11012b03     	lw	s6, 0x110(sp)
    49e4: 10c12b83     	lw	s7, 0x10c(sp)
    49e8: 10812c03     	lw	s8, 0x108(sp)
    49ec: 10412c83     	lw	s9, 0x104(sp)
    49f0: 10012d03     	lw	s10, 0x100(sp)
    49f4: 0fc12d83     	lw	s11, 0xfc(sp)
    49f8: 13010113     	addi	sp, sp, 0x130
    49fc: 00008067     	ret

00004a00 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>:
    4a00: ff010113     	addi	sp, sp, -0x10
    4a04: 00112623     	sw	ra, 0xc(sp)
    4a08: 00812423     	sw	s0, 0x8(sp)
    4a0c: 01010413     	addi	s0, sp, 0x10
    4a10: 00050593     	mv	a1, a0
    4a14: 00000533     	add	a0, zero, zero
    4a18: 7cb01073     	csrw	0x7cb, zero
    4a1c: 7cb01073     	csrw	0x7cb, zero
    4a20: 7cb01073     	csrw	0x7cb, zero
    4a24: 7cb01073     	csrw	0x7cb, zero
    4a28: 7cb01073     	csrw	0x7cb, zero
    4a2c: 7cb01073     	csrw	0x7cb, zero
    4a30: 7cb01073     	csrw	0x7cb, zero
    4a34: 7cb01073     	csrw	0x7cb, zero
    4a38: 7cb01073     	csrw	0x7cb, zero
    4a3c: 7cb01073     	csrw	0x7cb, zero
    4a40: 7cb01073     	csrw	0x7cb, zero
    4a44: 7cb01073     	csrw	0x7cb, zero
    4a48: 7cb01073     	csrw	0x7cb, zero
    4a4c: 7cb01073     	csrw	0x7cb, zero
    4a50: 7cb01073     	csrw	0x7cb, zero
    4a54: 7cb01073     	csrw	0x7cb, zero
    4a58: 7cb01073     	csrw	0x7cb, zero
    4a5c: 7cb01073     	csrw	0x7cb, zero
    4a60: 7cb01073     	csrw	0x7cb, zero
    4a64: 7cb01073     	csrw	0x7cb, zero
    4a68: 7cb01073     	csrw	0x7cb, zero
    4a6c: 7cb01073     	csrw	0x7cb, zero
    4a70: 7cb01073     	csrw	0x7cb, zero
    4a74: 7cb01073     	csrw	0x7cb, zero
    4a78: 7cb01073     	csrw	0x7cb, zero
    4a7c: 7cb01073     	csrw	0x7cb, zero
    4a80: 7cb01073     	csrw	0x7cb, zero
    4a84: 7cb01073     	csrw	0x7cb, zero
    4a88: 7cb01073     	csrw	0x7cb, zero
    4a8c: 7cb01073     	csrw	0x7cb, zero
    4a90: 7cb01073     	csrw	0x7cb, zero
    4a94: 7cb01073     	csrw	0x7cb, zero
    4a98: 7cb01073     	csrw	0x7cb, zero
    4a9c: 7cb01073     	csrw	0x7cb, zero
    4aa0: 7cb01073     	csrw	0x7cb, zero
    4aa4: 7cb01073     	csrw	0x7cb, zero
    4aa8: 7cb01073     	csrw	0x7cb, zero
    4aac: 7cb01073     	csrw	0x7cb, zero
    4ab0: 7cb01073     	csrw	0x7cb, zero
    4ab4: 7cb01073     	csrw	0x7cb, zero
    4ab8: 7cb01073     	csrw	0x7cb, zero
    4abc: 7cb01073     	csrw	0x7cb, zero
    4ac0: 7cb01073     	csrw	0x7cb, zero
    4ac4: 7cb01073     	csrw	0x7cb, zero
    4ac8: 7cb01073     	csrw	0x7cb, zero
    4acc: 7cb01073     	csrw	0x7cb, zero
    4ad0: 7cb01073     	csrw	0x7cb, zero
    4ad4: 7cb01073     	csrw	0x7cb, zero
    4ad8: 7cb01073     	csrw	0x7cb, zero
    4adc: 7cb01073     	csrw	0x7cb, zero
    4ae0: 7cb01073     	csrw	0x7cb, zero
    4ae4: 7cb01073     	csrw	0x7cb, zero
    4ae8: 7cb01073     	csrw	0x7cb, zero
    4aec: 7cb01073     	csrw	0x7cb, zero
    4af0: 7cb01073     	csrw	0x7cb, zero
    4af4: 7cb01073     	csrw	0x7cb, zero
    4af8: 7cb01073     	csrw	0x7cb, zero
    4afc: 7cb01073     	csrw	0x7cb, zero
    4b00: 7cb01073     	csrw	0x7cb, zero
    4b04: 7cb01073     	csrw	0x7cb, zero
    4b08: 7cb01073     	csrw	0x7cb, zero
    4b0c: 7cb01073     	csrw	0x7cb, zero
    4b10: 7cb01073     	csrw	0x7cb, zero
    4b14: 7cb01073     	csrw	0x7cb, zero
    4b18: 7cb01073     	csrw	0x7cb, zero
    4b1c: 7cb01073     	csrw	0x7cb, zero
    4b20: 7cb01073     	csrw	0x7cb, zero
    4b24: 7cb01073     	csrw	0x7cb, zero
    4b28: 7cb01073     	csrw	0x7cb, zero
    4b2c: 7cb01073     	csrw	0x7cb, zero
    4b30: 7cb01073     	csrw	0x7cb, zero
    4b34: 7cb01073     	csrw	0x7cb, zero
    4b38: 7cb01073     	csrw	0x7cb, zero
    4b3c: 7cb01073     	csrw	0x7cb, zero
    4b40: 7cb01073     	csrw	0x7cb, zero
    4b44: 7cb01073     	csrw	0x7cb, zero
    4b48: 7cb01073     	csrw	0x7cb, zero
    4b4c: 7cb01073     	csrw	0x7cb, zero
    4b50: 7cb01073     	csrw	0x7cb, zero
    4b54: 7cb01073     	csrw	0x7cb, zero
    4b58: 7cb01073     	csrw	0x7cb, zero
    4b5c: 7cb01073     	csrw	0x7cb, zero
    4b60: 7cb01073     	csrw	0x7cb, zero
    4b64: 7cb01073     	csrw	0x7cb, zero
    4b68: 7cb01073     	csrw	0x7cb, zero
    4b6c: 7cb01073     	csrw	0x7cb, zero
    4b70: 7cb01073     	csrw	0x7cb, zero
    4b74: 7cb01073     	csrw	0x7cb, zero
    4b78: 7cb01073     	csrw	0x7cb, zero
    4b7c: 7cb01073     	csrw	0x7cb, zero
    4b80: 7cb01073     	csrw	0x7cb, zero
    4b84: 7cb01073     	csrw	0x7cb, zero
    4b88: 7cb01073     	csrw	0x7cb, zero
    4b8c: 7cb01073     	csrw	0x7cb, zero
    4b90: 7cb01073     	csrw	0x7cb, zero
    4b94: 7cb01073     	csrw	0x7cb, zero
    4b98: 7cb01073     	csrw	0x7cb, zero
    4b9c: 7cb01073     	csrw	0x7cb, zero
    4ba0: 7cb01073     	csrw	0x7cb, zero
    4ba4: 7cb01073     	csrw	0x7cb, zero
    4ba8: 7cb01073     	csrw	0x7cb, zero
    4bac: 7cb01073     	csrw	0x7cb, zero
    4bb0: 7cb01073     	csrw	0x7cb, zero
    4bb4: 7cb01073     	csrw	0x7cb, zero
    4bb8: 7cb01073     	csrw	0x7cb, zero
    4bbc: 7cb01073     	csrw	0x7cb, zero
    4bc0: 7cb01073     	csrw	0x7cb, zero
    4bc4: 7cb01073     	csrw	0x7cb, zero
    4bc8: 7cb01073     	csrw	0x7cb, zero
    4bcc: 7cb01073     	csrw	0x7cb, zero
    4bd0: 7cb01073     	csrw	0x7cb, zero
    4bd4: 7cb01073     	csrw	0x7cb, zero
    4bd8: 7cb01073     	csrw	0x7cb, zero
    4bdc: 7cb01073     	csrw	0x7cb, zero
    4be0: 7cb01073     	csrw	0x7cb, zero
    4be4: 7cb01073     	csrw	0x7cb, zero
    4be8: 7cb01073     	csrw	0x7cb, zero
    4bec: 7cb01073     	csrw	0x7cb, zero
    4bf0: 7cb01073     	csrw	0x7cb, zero
    4bf4: 7cb01073     	csrw	0x7cb, zero
    4bf8: 7cb01073     	csrw	0x7cb, zero
    4bfc: 7cb01073     	csrw	0x7cb, zero
    4c00: 7cb01073     	csrw	0x7cb, zero
    4c04: 7cb01073     	csrw	0x7cb, zero
    4c08: 7cb01073     	csrw	0x7cb, zero
    4c0c: 7cb01073     	csrw	0x7cb, zero
    4c10: 7cb01073     	csrw	0x7cb, zero
    4c14: 7cb01073     	csrw	0x7cb, zero
    4c18: 7cb01073     	csrw	0x7cb, zero
    4c1c: 7cb01073     	csrw	0x7cb, zero
    4c20: 7cb01073     	csrw	0x7cb, zero
    4c24: 7cb01073     	csrw	0x7cb, zero
    4c28: 7cb01073     	csrw	0x7cb, zero
    4c2c: 7cb01073     	csrw	0x7cb, zero
    4c30: 7cb01073     	csrw	0x7cb, zero
    4c34: 7cb01073     	csrw	0x7cb, zero
    4c38: 7cb01073     	csrw	0x7cb, zero
    4c3c: 7cb01073     	csrw	0x7cb, zero
    4c40: 7cb01073     	csrw	0x7cb, zero
    4c44: 7cb01073     	csrw	0x7cb, zero
    4c48: 7cb01073     	csrw	0x7cb, zero
    4c4c: 7cb01073     	csrw	0x7cb, zero
    4c50: 7cb01073     	csrw	0x7cb, zero
    4c54: 7cb01073     	csrw	0x7cb, zero
    4c58: 7cb01073     	csrw	0x7cb, zero
    4c5c: 7cb01073     	csrw	0x7cb, zero
    4c60: 7cb01073     	csrw	0x7cb, zero
    4c64: 7cb01073     	csrw	0x7cb, zero
    4c68: 7cb01073     	csrw	0x7cb, zero
    4c6c: 7cb01073     	csrw	0x7cb, zero
    4c70: 7cb01073     	csrw	0x7cb, zero
    4c74: 7cb01073     	csrw	0x7cb, zero
    4c78: 7cb01073     	csrw	0x7cb, zero
    4c7c: 7cb01073     	csrw	0x7cb, zero
    4c80: 7cb01073     	csrw	0x7cb, zero
    4c84: 7cb01073     	csrw	0x7cb, zero
    4c88: 7cb01073     	csrw	0x7cb, zero
    4c8c: 7cb01073     	csrw	0x7cb, zero
    4c90: 7cb01073     	csrw	0x7cb, zero
    4c94: 7cb01073     	csrw	0x7cb, zero
    4c98: 7cb01073     	csrw	0x7cb, zero
    4c9c: 7cb01073     	csrw	0x7cb, zero
    4ca0: 7cb01073     	csrw	0x7cb, zero
    4ca4: 7cb01073     	csrw	0x7cb, zero
    4ca8: 7cb01073     	csrw	0x7cb, zero
    4cac: 7cb01073     	csrw	0x7cb, zero
    4cb0: 7cb01073     	csrw	0x7cb, zero
    4cb4: 7cb01073     	csrw	0x7cb, zero
    4cb8: 7cb01073     	csrw	0x7cb, zero
    4cbc: 7cb01073     	csrw	0x7cb, zero
    4cc0: 7cb01073     	csrw	0x7cb, zero
    4cc4: 7cb01073     	csrw	0x7cb, zero
    4cc8: 7cb01073     	csrw	0x7cb, zero
    4ccc: 7cb01073     	csrw	0x7cb, zero
    4cd0: 7cb01073     	csrw	0x7cb, zero
    4cd4: 7cb01073     	csrw	0x7cb, zero
    4cd8: 7cb01073     	csrw	0x7cb, zero
    4cdc: 7cb01073     	csrw	0x7cb, zero
    4ce0: 7cb01073     	csrw	0x7cb, zero
    4ce4: 7cb01073     	csrw	0x7cb, zero
    4ce8: 7cb01073     	csrw	0x7cb, zero
    4cec: 7cb01073     	csrw	0x7cb, zero
    4cf0: 7cb01073     	csrw	0x7cb, zero
    4cf4: 7cb01073     	csrw	0x7cb, zero
    4cf8: 7cb01073     	csrw	0x7cb, zero
    4cfc: 7cb01073     	csrw	0x7cb, zero
    4d00: 7cb01073     	csrw	0x7cb, zero
    4d04: 7cb01073     	csrw	0x7cb, zero
    4d08: 7cb01073     	csrw	0x7cb, zero
    4d0c: 7cb01073     	csrw	0x7cb, zero
    4d10: 7cb01073     	csrw	0x7cb, zero
    4d14: 7cb01073     	csrw	0x7cb, zero
    4d18: 7cb01073     	csrw	0x7cb, zero
    4d1c: 7cb01073     	csrw	0x7cb, zero
    4d20: 7cb01073     	csrw	0x7cb, zero
    4d24: 7cb01073     	csrw	0x7cb, zero
    4d28: 7cb01073     	csrw	0x7cb, zero
    4d2c: 7cb01073     	csrw	0x7cb, zero
    4d30: 7cb01073     	csrw	0x7cb, zero
    4d34: 7cb01073     	csrw	0x7cb, zero
    4d38: 7cb01073     	csrw	0x7cb, zero
    4d3c: 7cb01073     	csrw	0x7cb, zero
    4d40: 7cb01073     	csrw	0x7cb, zero
    4d44: 7cb01073     	csrw	0x7cb, zero
    4d48: 7cb01073     	csrw	0x7cb, zero
    4d4c: 7cb01073     	csrw	0x7cb, zero
    4d50: 7cb01073     	csrw	0x7cb, zero
    4d54: 7cb01073     	csrw	0x7cb, zero
    4d58: 7cb01073     	csrw	0x7cb, zero
    4d5c: 7cb01073     	csrw	0x7cb, zero
    4d60: 7cb01073     	csrw	0x7cb, zero
    4d64: 7cb01073     	csrw	0x7cb, zero
    4d68: 7cb01073     	csrw	0x7cb, zero
    4d6c: 7cb01073     	csrw	0x7cb, zero
    4d70: 7cb01073     	csrw	0x7cb, zero
    4d74: 7cb01073     	csrw	0x7cb, zero
    4d78: 7cb01073     	csrw	0x7cb, zero
    4d7c: 7cb01073     	csrw	0x7cb, zero
    4d80: 7cb01073     	csrw	0x7cb, zero
    4d84: 7cb01073     	csrw	0x7cb, zero
    4d88: 7cb01073     	csrw	0x7cb, zero
    4d8c: 7cb01073     	csrw	0x7cb, zero
    4d90: 7cb01073     	csrw	0x7cb, zero
    4d94: 7cb01073     	csrw	0x7cb, zero
    4d98: 7cb01073     	csrw	0x7cb, zero
    4d9c: 7cb01073     	csrw	0x7cb, zero
    4da0: 7cb01073     	csrw	0x7cb, zero
    4da4: 7cb01073     	csrw	0x7cb, zero
    4da8: 7cb01073     	csrw	0x7cb, zero
    4dac: 7cb01073     	csrw	0x7cb, zero
    4db0: 7cb01073     	csrw	0x7cb, zero
    4db4: 7cb01073     	csrw	0x7cb, zero
    4db8: 7cb01073     	csrw	0x7cb, zero
    4dbc: 7cb01073     	csrw	0x7cb, zero
    4dc0: 7cb01073     	csrw	0x7cb, zero
    4dc4: 7cb01073     	csrw	0x7cb, zero
    4dc8: 7cb01073     	csrw	0x7cb, zero
    4dcc: 7cb01073     	csrw	0x7cb, zero
    4dd0: 7cb01073     	csrw	0x7cb, zero
    4dd4: 7cb01073     	csrw	0x7cb, zero
    4dd8: 7cb01073     	csrw	0x7cb, zero
    4ddc: 7cb01073     	csrw	0x7cb, zero
    4de0: 7cb01073     	csrw	0x7cb, zero
    4de4: 7cb01073     	csrw	0x7cb, zero
    4de8: 7cb01073     	csrw	0x7cb, zero
    4dec: 7cb01073     	csrw	0x7cb, zero
    4df0: 7cb01073     	csrw	0x7cb, zero
    4df4: 7cb01073     	csrw	0x7cb, zero
    4df8: 7cb01073     	csrw	0x7cb, zero
    4dfc: 7cb01073     	csrw	0x7cb, zero
    4e00: 7cb01073     	csrw	0x7cb, zero
    4e04: 7cb01073     	csrw	0x7cb, zero
    4e08: 7cb01073     	csrw	0x7cb, zero
    4e0c: 7cb01073     	csrw	0x7cb, zero
    4e10: 7cb01073     	csrw	0x7cb, zero
    4e14: 7cb01073     	csrw	0x7cb, zero
    4e18: 7cb01073     	csrw	0x7cb, zero
    4e1c: 7cb01073     	csrw	0x7cb, zero
    4e20: 7cb01073     	csrw	0x7cb, zero
    4e24: 7cb01073     	csrw	0x7cb, zero
    4e28: 7cb01073     	csrw	0x7cb, zero
    4e2c: 7cb01073     	csrw	0x7cb, zero
    4e30: 7cb01073     	csrw	0x7cb, zero
    4e34: 7cb01073     	csrw	0x7cb, zero
    4e38: 7cb01073     	csrw	0x7cb, zero
    4e3c: 7cb01073     	csrw	0x7cb, zero
    4e40: 7cb01073     	csrw	0x7cb, zero
    4e44: 7cb01073     	csrw	0x7cb, zero
    4e48: 7cb01073     	csrw	0x7cb, zero
    4e4c: 7cb01073     	csrw	0x7cb, zero
    4e50: 7cb01073     	csrw	0x7cb, zero
    4e54: 7cb01073     	csrw	0x7cb, zero
    4e58: 7cb01073     	csrw	0x7cb, zero
    4e5c: 7cb01073     	csrw	0x7cb, zero
    4e60: 7cb01073     	csrw	0x7cb, zero
    4e64: 7cb01073     	csrw	0x7cb, zero
    4e68: 7cb01073     	csrw	0x7cb, zero
    4e6c: 7cb01073     	csrw	0x7cb, zero
    4e70: 7cb01073     	csrw	0x7cb, zero
    4e74: 7cb01073     	csrw	0x7cb, zero
    4e78: 7cb01073     	csrw	0x7cb, zero
    4e7c: 7cb01073     	csrw	0x7cb, zero
    4e80: 7cb01073     	csrw	0x7cb, zero
    4e84: 7cb01073     	csrw	0x7cb, zero
    4e88: 7cb01073     	csrw	0x7cb, zero
    4e8c: 7cb01073     	csrw	0x7cb, zero
    4e90: 7cb01073     	csrw	0x7cb, zero
    4e94: 7cb01073     	csrw	0x7cb, zero
    4e98: 7cb01073     	csrw	0x7cb, zero
    4e9c: 7cb01073     	csrw	0x7cb, zero
    4ea0: 7cb01073     	csrw	0x7cb, zero
    4ea4: 7cb01073     	csrw	0x7cb, zero
    4ea8: 7cb01073     	csrw	0x7cb, zero
    4eac: 7cb01073     	csrw	0x7cb, zero
    4eb0: 7cb01073     	csrw	0x7cb, zero
    4eb4: 7cb01073     	csrw	0x7cb, zero
    4eb8: 7cb01073     	csrw	0x7cb, zero
    4ebc: 7cb01073     	csrw	0x7cb, zero
    4ec0: 7cb01073     	csrw	0x7cb, zero
    4ec4: 7cb01073     	csrw	0x7cb, zero
    4ec8: 7cb01073     	csrw	0x7cb, zero
    4ecc: 7cb01073     	csrw	0x7cb, zero
    4ed0: 7cb01073     	csrw	0x7cb, zero
    4ed4: 7cb01073     	csrw	0x7cb, zero
    4ed8: 7cb01073     	csrw	0x7cb, zero
    4edc: 7cb01073     	csrw	0x7cb, zero
    4ee0: 7cb01073     	csrw	0x7cb, zero
    4ee4: 7cb01073     	csrw	0x7cb, zero
    4ee8: 7cb01073     	csrw	0x7cb, zero
    4eec: 7cb01073     	csrw	0x7cb, zero
    4ef0: 7cb01073     	csrw	0x7cb, zero
    4ef4: 7cb01073     	csrw	0x7cb, zero
    4ef8: 7cb01073     	csrw	0x7cb, zero
    4efc: 7cb01073     	csrw	0x7cb, zero
    4f00: 7cb01073     	csrw	0x7cb, zero
    4f04: 7cb01073     	csrw	0x7cb, zero
    4f08: 7cb01073     	csrw	0x7cb, zero
    4f0c: 7cb01073     	csrw	0x7cb, zero
    4f10: 7cb01073     	csrw	0x7cb, zero
    4f14: 7cb01073     	csrw	0x7cb, zero
    4f18: 7cb01073     	csrw	0x7cb, zero
    4f1c: 7cb01073     	csrw	0x7cb, zero
    4f20: 7cb01073     	csrw	0x7cb, zero
    4f24: 7cb01073     	csrw	0x7cb, zero
    4f28: 7cb01073     	csrw	0x7cb, zero
    4f2c: 7cb01073     	csrw	0x7cb, zero
    4f30: 7cb01073     	csrw	0x7cb, zero
    4f34: 7cb01073     	csrw	0x7cb, zero
    4f38: 7cb01073     	csrw	0x7cb, zero
    4f3c: 7cb01073     	csrw	0x7cb, zero
    4f40: 7cb01073     	csrw	0x7cb, zero
    4f44: 7cb01073     	csrw	0x7cb, zero
    4f48: 7cb01073     	csrw	0x7cb, zero
    4f4c: 7cb01073     	csrw	0x7cb, zero
    4f50: 7cb01073     	csrw	0x7cb, zero
    4f54: 7cb01073     	csrw	0x7cb, zero
    4f58: 7cb01073     	csrw	0x7cb, zero
    4f5c: 7cb01073     	csrw	0x7cb, zero
    4f60: 7cb01073     	csrw	0x7cb, zero
    4f64: 7cb01073     	csrw	0x7cb, zero
    4f68: 7cb01073     	csrw	0x7cb, zero
    4f6c: 7cb01073     	csrw	0x7cb, zero
    4f70: 7cb01073     	csrw	0x7cb, zero
    4f74: 7cb01073     	csrw	0x7cb, zero
    4f78: 7cb01073     	csrw	0x7cb, zero
    4f7c: 7cb01073     	csrw	0x7cb, zero
    4f80: 7cb01073     	csrw	0x7cb, zero
    4f84: 7cb01073     	csrw	0x7cb, zero
    4f88: 7cb01073     	csrw	0x7cb, zero
    4f8c: 7cb01073     	csrw	0x7cb, zero
    4f90: 7cb01073     	csrw	0x7cb, zero
    4f94: 7cb01073     	csrw	0x7cb, zero
    4f98: 7cb01073     	csrw	0x7cb, zero
    4f9c: 7cb01073     	csrw	0x7cb, zero
    4fa0: 7cb01073     	csrw	0x7cb, zero
    4fa4: 7cb01073     	csrw	0x7cb, zero
    4fa8: 7cb01073     	csrw	0x7cb, zero
    4fac: 7cb01073     	csrw	0x7cb, zero
    4fb0: 7cb01073     	csrw	0x7cb, zero
    4fb4: 7cb01073     	csrw	0x7cb, zero
    4fb8: 7cb01073     	csrw	0x7cb, zero
    4fbc: 7cb01073     	csrw	0x7cb, zero
    4fc0: 7cb01073     	csrw	0x7cb, zero
    4fc4: 7cb01073     	csrw	0x7cb, zero
    4fc8: 7cb01073     	csrw	0x7cb, zero
    4fcc: 7cb01073     	csrw	0x7cb, zero
    4fd0: 7cb01073     	csrw	0x7cb, zero
    4fd4: 7cb01073     	csrw	0x7cb, zero
    4fd8: 7cb01073     	csrw	0x7cb, zero
    4fdc: 7cb01073     	csrw	0x7cb, zero
    4fe0: 7cb01073     	csrw	0x7cb, zero
    4fe4: 7cb01073     	csrw	0x7cb, zero
    4fe8: 7cb01073     	csrw	0x7cb, zero
    4fec: 7cb01073     	csrw	0x7cb, zero
    4ff0: 7cb01073     	csrw	0x7cb, zero
    4ff4: 7cb01073     	csrw	0x7cb, zero
    4ff8: 7cb01073     	csrw	0x7cb, zero
    4ffc: 7cb01073     	csrw	0x7cb, zero
    5000: 7cb01073     	csrw	0x7cb, zero
    5004: 7cb01073     	csrw	0x7cb, zero
    5008: 7cb01073     	csrw	0x7cb, zero
    500c: 7cb01073     	csrw	0x7cb, zero
    5010: 7cb01073     	csrw	0x7cb, zero
    5014: 7cb01073     	csrw	0x7cb, zero
    5018: 7cb01073     	csrw	0x7cb, zero
    501c: 7cb01073     	csrw	0x7cb, zero
    5020: 7cb01073     	csrw	0x7cb, zero
    5024: 7cb01073     	csrw	0x7cb, zero
    5028: 7cb01073     	csrw	0x7cb, zero
    502c: 7cb01073     	csrw	0x7cb, zero
    5030: 7cb01073     	csrw	0x7cb, zero
    5034: 7cb01073     	csrw	0x7cb, zero
    5038: 7cb01073     	csrw	0x7cb, zero
    503c: 7cb01073     	csrw	0x7cb, zero
    5040: 7cb01073     	csrw	0x7cb, zero
    5044: 7cb01073     	csrw	0x7cb, zero
    5048: 7cb01073     	csrw	0x7cb, zero
    504c: 7cb01073     	csrw	0x7cb, zero
    5050: 7cb01073     	csrw	0x7cb, zero
    5054: 7cb01073     	csrw	0x7cb, zero
    5058: 7cb01073     	csrw	0x7cb, zero
    505c: 7cb01073     	csrw	0x7cb, zero
    5060: 7cb01073     	csrw	0x7cb, zero
    5064: 7cb01073     	csrw	0x7cb, zero
    5068: 7cb01073     	csrw	0x7cb, zero
    506c: 7cb01073     	csrw	0x7cb, zero
    5070: 7cb01073     	csrw	0x7cb, zero
    5074: 7cb01073     	csrw	0x7cb, zero
    5078: 7cb01073     	csrw	0x7cb, zero
    507c: 7cb01073     	csrw	0x7cb, zero
    5080: 7cb01073     	csrw	0x7cb, zero
    5084: 7cb01073     	csrw	0x7cb, zero
    5088: 7cb01073     	csrw	0x7cb, zero
    508c: 7cb01073     	csrw	0x7cb, zero
    5090: 7cb01073     	csrw	0x7cb, zero
    5094: 7cb01073     	csrw	0x7cb, zero
    5098: 7cb01073     	csrw	0x7cb, zero
    509c: 7cb01073     	csrw	0x7cb, zero
    50a0: 7cb01073     	csrw	0x7cb, zero
    50a4: 7cb01073     	csrw	0x7cb, zero
    50a8: 7cb01073     	csrw	0x7cb, zero
    50ac: 7cb01073     	csrw	0x7cb, zero
    50b0: 7cb01073     	csrw	0x7cb, zero
    50b4: 7cb01073     	csrw	0x7cb, zero
    50b8: 7cb01073     	csrw	0x7cb, zero
    50bc: 7cb01073     	csrw	0x7cb, zero
    50c0: 7cb01073     	csrw	0x7cb, zero
    50c4: 7cb01073     	csrw	0x7cb, zero
    50c8: 7cb01073     	csrw	0x7cb, zero
    50cc: 7cb01073     	csrw	0x7cb, zero
    50d0: 7cb01073     	csrw	0x7cb, zero
    50d4: 7cb01073     	csrw	0x7cb, zero
    50d8: 7cb01073     	csrw	0x7cb, zero
    50dc: 7cb01073     	csrw	0x7cb, zero
    50e0: 7cb01073     	csrw	0x7cb, zero
    50e4: 7cb01073     	csrw	0x7cb, zero
    50e8: 7cb01073     	csrw	0x7cb, zero
    50ec: 7cb01073     	csrw	0x7cb, zero
    50f0: 7cb01073     	csrw	0x7cb, zero
    50f4: 7cb01073     	csrw	0x7cb, zero
    50f8: 7cb01073     	csrw	0x7cb, zero
    50fc: 7cb01073     	csrw	0x7cb, zero
    5100: 7cb01073     	csrw	0x7cb, zero
    5104: 7cb01073     	csrw	0x7cb, zero
    5108: 7cb01073     	csrw	0x7cb, zero
    510c: 7cb01073     	csrw	0x7cb, zero
    5110: 7cb01073     	csrw	0x7cb, zero
    5114: 7cb01073     	csrw	0x7cb, zero
    5118: 7cb01073     	csrw	0x7cb, zero
    511c: 7cb01073     	csrw	0x7cb, zero
    5120: 7cb01073     	csrw	0x7cb, zero
    5124: 7cb01073     	csrw	0x7cb, zero
    5128: 7cb01073     	csrw	0x7cb, zero
    512c: 7cb01073     	csrw	0x7cb, zero
    5130: 7cb01073     	csrw	0x7cb, zero
    5134: 7cb01073     	csrw	0x7cb, zero
    5138: 7cb01073     	csrw	0x7cb, zero
    513c: 7cb01073     	csrw	0x7cb, zero
    5140: 7cb01073     	csrw	0x7cb, zero
    5144: 7cb01073     	csrw	0x7cb, zero
    5148: 7cb01073     	csrw	0x7cb, zero
    514c: 7cb01073     	csrw	0x7cb, zero
    5150: 7cb01073     	csrw	0x7cb, zero
    5154: 7cb01073     	csrw	0x7cb, zero
    5158: 7cb01073     	csrw	0x7cb, zero
    515c: 7cb01073     	csrw	0x7cb, zero
    5160: 7cb01073     	csrw	0x7cb, zero
    5164: 7cb01073     	csrw	0x7cb, zero
    5168: 7cb01073     	csrw	0x7cb, zero
    516c: 7cb01073     	csrw	0x7cb, zero
    5170: 7cb01073     	csrw	0x7cb, zero
    5174: 7cb01073     	csrw	0x7cb, zero
    5178: 7cb01073     	csrw	0x7cb, zero
    517c: 7cb01073     	csrw	0x7cb, zero
    5180: 7cb01073     	csrw	0x7cb, zero
    5184: 7cb01073     	csrw	0x7cb, zero
    5188: 7cb01073     	csrw	0x7cb, zero
    518c: 7cb01073     	csrw	0x7cb, zero
    5190: 7cb01073     	csrw	0x7cb, zero
    5194: 7cb01073     	csrw	0x7cb, zero
    5198: 7cb01073     	csrw	0x7cb, zero
    519c: 7cb01073     	csrw	0x7cb, zero
    51a0: 7cb01073     	csrw	0x7cb, zero
    51a4: 7cb01073     	csrw	0x7cb, zero
    51a8: 7cb01073     	csrw	0x7cb, zero
    51ac: 7cb01073     	csrw	0x7cb, zero
    51b0: 7cb01073     	csrw	0x7cb, zero
    51b4: 7cb01073     	csrw	0x7cb, zero
    51b8: 7cb01073     	csrw	0x7cb, zero
    51bc: 7cb01073     	csrw	0x7cb, zero
    51c0: 7cb01073     	csrw	0x7cb, zero
    51c4: 7cb01073     	csrw	0x7cb, zero
    51c8: 7cb01073     	csrw	0x7cb, zero
    51cc: 7cb01073     	csrw	0x7cb, zero
    51d0: 7cb01073     	csrw	0x7cb, zero
    51d4: 7cb01073     	csrw	0x7cb, zero
    51d8: 7cb01073     	csrw	0x7cb, zero
    51dc: 7cb01073     	csrw	0x7cb, zero
    51e0: 7cb01073     	csrw	0x7cb, zero
    51e4: 7cb01073     	csrw	0x7cb, zero
    51e8: 7cb01073     	csrw	0x7cb, zero
    51ec: 7cb01073     	csrw	0x7cb, zero
    51f0: 7cb01073     	csrw	0x7cb, zero
    51f4: 7cb01073     	csrw	0x7cb, zero
    51f8: 7cb01073     	csrw	0x7cb, zero
    51fc: 7cb01073     	csrw	0x7cb, zero
    5200: 7cb01073     	csrw	0x7cb, zero
    5204: 7cb01073     	csrw	0x7cb, zero
    5208: 7cb01073     	csrw	0x7cb, zero
    520c: 7cb01073     	csrw	0x7cb, zero
    5210: 7cb01073     	csrw	0x7cb, zero
    5214: 7cb01073     	csrw	0x7cb, zero
    5218: 7cb01073     	csrw	0x7cb, zero
    521c: 7cb01073     	csrw	0x7cb, zero
    5220: 7cb01073     	csrw	0x7cb, zero
    5224: 7cb01073     	csrw	0x7cb, zero
    5228: 7cb01073     	csrw	0x7cb, zero
    522c: 7cb01073     	csrw	0x7cb, zero
    5230: 7cb01073     	csrw	0x7cb, zero
    5234: 7cb01073     	csrw	0x7cb, zero
    5238: 7cb01073     	csrw	0x7cb, zero
    523c: 7cb01073     	csrw	0x7cb, zero
    5240: 7cb01073     	csrw	0x7cb, zero
    5244: 7cb01073     	csrw	0x7cb, zero
    5248: 7cb01073     	csrw	0x7cb, zero
    524c: 7cb01073     	csrw	0x7cb, zero
    5250: 7cb01073     	csrw	0x7cb, zero
    5254: 7cb01073     	csrw	0x7cb, zero
    5258: 7cb01073     	csrw	0x7cb, zero
    525c: 7cb01073     	csrw	0x7cb, zero
    5260: 7cb01073     	csrw	0x7cb, zero
    5264: 7cb01073     	csrw	0x7cb, zero
    5268: 7cb01073     	csrw	0x7cb, zero
    526c: 7cb01073     	csrw	0x7cb, zero
    5270: 7cb01073     	csrw	0x7cb, zero
    5274: 7cb01073     	csrw	0x7cb, zero
    5278: 7cb01073     	csrw	0x7cb, zero
    527c: 7cb01073     	csrw	0x7cb, zero
    5280: 7cb01073     	csrw	0x7cb, zero
    5284: 7cb01073     	csrw	0x7cb, zero
    5288: 7cb01073     	csrw	0x7cb, zero
    528c: 7cb01073     	csrw	0x7cb, zero
    5290: 7cb01073     	csrw	0x7cb, zero
    5294: 7cb01073     	csrw	0x7cb, zero
    5298: 7cb01073     	csrw	0x7cb, zero
    529c: 7cb01073     	csrw	0x7cb, zero
    52a0: 7cb01073     	csrw	0x7cb, zero
    52a4: 7cb01073     	csrw	0x7cb, zero
    52a8: 7cb01073     	csrw	0x7cb, zero
    52ac: 7cb01073     	csrw	0x7cb, zero
    52b0: 7cb01073     	csrw	0x7cb, zero
    52b4: 7cb01073     	csrw	0x7cb, zero
    52b8: 7cb01073     	csrw	0x7cb, zero
    52bc: 7cb01073     	csrw	0x7cb, zero
    52c0: 7cb01073     	csrw	0x7cb, zero
    52c4: 7cb01073     	csrw	0x7cb, zero
    52c8: 7cb01073     	csrw	0x7cb, zero
    52cc: 7cb01073     	csrw	0x7cb, zero
    52d0: 7cb01073     	csrw	0x7cb, zero
    52d4: 7cb01073     	csrw	0x7cb, zero
    52d8: 7cb01073     	csrw	0x7cb, zero
    52dc: 7cb01073     	csrw	0x7cb, zero
    52e0: 7cb01073     	csrw	0x7cb, zero
    52e4: 7cb01073     	csrw	0x7cb, zero
    52e8: 7cb01073     	csrw	0x7cb, zero
    52ec: 7cb01073     	csrw	0x7cb, zero
    52f0: 7cb01073     	csrw	0x7cb, zero
    52f4: 7cb01073     	csrw	0x7cb, zero
    52f8: 7cb01073     	csrw	0x7cb, zero
    52fc: 7cb01073     	csrw	0x7cb, zero
    5300: 7cb01073     	csrw	0x7cb, zero
    5304: 7cb01073     	csrw	0x7cb, zero
    5308: 7cb01073     	csrw	0x7cb, zero
    530c: 7cb01073     	csrw	0x7cb, zero
    5310: 7cb01073     	csrw	0x7cb, zero
    5314: 7cb01073     	csrw	0x7cb, zero
    5318: 7cb01073     	csrw	0x7cb, zero
    531c: 7cb01073     	csrw	0x7cb, zero
    5320: 7cb01073     	csrw	0x7cb, zero
    5324: 7cb01073     	csrw	0x7cb, zero
    5328: 7cb01073     	csrw	0x7cb, zero
    532c: 7cb01073     	csrw	0x7cb, zero
    5330: 7cb01073     	csrw	0x7cb, zero
    5334: 7cb01073     	csrw	0x7cb, zero
    5338: 7cb01073     	csrw	0x7cb, zero
    533c: 7cb01073     	csrw	0x7cb, zero
    5340: 7cb01073     	csrw	0x7cb, zero
    5344: 7cb01073     	csrw	0x7cb, zero
    5348: 7cb01073     	csrw	0x7cb, zero
    534c: 7cb01073     	csrw	0x7cb, zero
    5350: 7cb01073     	csrw	0x7cb, zero
    5354: 7cb01073     	csrw	0x7cb, zero
    5358: 7cb01073     	csrw	0x7cb, zero
    535c: 7cb01073     	csrw	0x7cb, zero
    5360: 7cb01073     	csrw	0x7cb, zero
    5364: 7cb01073     	csrw	0x7cb, zero
    5368: 7cb01073     	csrw	0x7cb, zero
    536c: 7cb01073     	csrw	0x7cb, zero
    5370: 7cb01073     	csrw	0x7cb, zero
    5374: 7cb01073     	csrw	0x7cb, zero
    5378: 7cb01073     	csrw	0x7cb, zero
    537c: 7cb01073     	csrw	0x7cb, zero
    5380: 7cb01073     	csrw	0x7cb, zero
    5384: 7cb01073     	csrw	0x7cb, zero
    5388: 7cb01073     	csrw	0x7cb, zero
    538c: 7cb01073     	csrw	0x7cb, zero
    5390: 7cb01073     	csrw	0x7cb, zero
    5394: 7cb01073     	csrw	0x7cb, zero
    5398: 7cb01073     	csrw	0x7cb, zero
    539c: 7cb01073     	csrw	0x7cb, zero
    53a0: 7cb01073     	csrw	0x7cb, zero
    53a4: 7cb01073     	csrw	0x7cb, zero
    53a8: 7cb01073     	csrw	0x7cb, zero
    53ac: 7cb01073     	csrw	0x7cb, zero
    53b0: 7cb01073     	csrw	0x7cb, zero
    53b4: 7cb01073     	csrw	0x7cb, zero
    53b8: 7cb01073     	csrw	0x7cb, zero
    53bc: 7cb01073     	csrw	0x7cb, zero
    53c0: 7cb01073     	csrw	0x7cb, zero
    53c4: 7cb01073     	csrw	0x7cb, zero
    53c8: 7cb01073     	csrw	0x7cb, zero
    53cc: 7cb01073     	csrw	0x7cb, zero
    53d0: 7cb01073     	csrw	0x7cb, zero
    53d4: 7cb01073     	csrw	0x7cb, zero
    53d8: 7cb01073     	csrw	0x7cb, zero
    53dc: 7cb01073     	csrw	0x7cb, zero
    53e0: 7cb01073     	csrw	0x7cb, zero
    53e4: 7cb01073     	csrw	0x7cb, zero
    53e8: 7cb01073     	csrw	0x7cb, zero
    53ec: 7cb01073     	csrw	0x7cb, zero
    53f0: 7cb01073     	csrw	0x7cb, zero
    53f4: 7cb01073     	csrw	0x7cb, zero
    53f8: 7cb01073     	csrw	0x7cb, zero
    53fc: 7cb01073     	csrw	0x7cb, zero
    5400: 7cb01073     	csrw	0x7cb, zero
    5404: 7cb01073     	csrw	0x7cb, zero
    5408: 7cb01073     	csrw	0x7cb, zero
    540c: 7cb01073     	csrw	0x7cb, zero
    5410: 7cb01073     	csrw	0x7cb, zero
    5414: 7cb01073     	csrw	0x7cb, zero
    5418: 7cb01073     	csrw	0x7cb, zero
    541c: 7cb01073     	csrw	0x7cb, zero
    5420: 7cb01073     	csrw	0x7cb, zero
    5424: 7cb01073     	csrw	0x7cb, zero
    5428: 7cb01073     	csrw	0x7cb, zero
    542c: 7cb01073     	csrw	0x7cb, zero
    5430: 7cb01073     	csrw	0x7cb, zero
    5434: 7cb01073     	csrw	0x7cb, zero
    5438: 7cb01073     	csrw	0x7cb, zero
    543c: 00c12083     	lw	ra, 0xc(sp)
    5440: 00812403     	lw	s0, 0x8(sp)
    5444: 01010113     	addi	sp, sp, 0x10
    5448: 00008067     	ret

0000544c <keccak::p1600::h1e78a6fe180ce099>:
    544c: ff010113     	addi	sp, sp, -0x10
    5450: 00112623     	sw	ra, 0xc(sp)
    5454: 00812423     	sw	s0, 0x8(sp)
    5458: 01010413     	addi	s0, sp, 0x10
    545c: 00c12083     	lw	ra, 0xc(sp)
    5460: 00812403     	lw	s0, 0x8(sp)
    5464: 01010113     	addi	sp, sp, 0x10
    5468: 0040006f     	j	0x546c <keccak::keccak_p::h2c32d13bf019081a>

0000546c <keccak::keccak_p::h2c32d13bf019081a>:
    546c: ef010113     	addi	sp, sp, -0x110
    5470: 10112623     	sw	ra, 0x10c(sp)
    5474: 10812423     	sw	s0, 0x108(sp)
    5478: 10912223     	sw	s1, 0x104(sp)
    547c: 11212023     	sw	s2, 0x100(sp)
    5480: 0f312e23     	sw	s3, 0xfc(sp)
    5484: 0f412c23     	sw	s4, 0xf8(sp)
    5488: 0f512a23     	sw	s5, 0xf4(sp)
    548c: 0f612823     	sw	s6, 0xf0(sp)
    5490: 0f712623     	sw	s7, 0xec(sp)
    5494: 0f812423     	sw	s8, 0xe8(sp)
    5498: 0f912223     	sw	s9, 0xe4(sp)
    549c: 0fa12023     	sw	s10, 0xe0(sp)
    54a0: 0db12e23     	sw	s11, 0xdc(sp)
    54a4: 11010413     	addi	s0, sp, 0x110
    54a8: 01900613     	li	a2, 0x19
    54ac: 56c5fce3     	bgeu	a1, a2, 0x6224 <keccak::keccak_p::h2c32d13bf019081a+0xdb8>
    54b0: 52058ce3     	beqz	a1, 0x61e8 <keccak::keccak_p::h2c32d13bf019081a+0xd7c>
    54b4: 00359613     	slli	a2, a1, 0x3
    54b8: 02052583     	lw	a1, 0x20(a0)
    54bc: fab42e23     	sw	a1, -0x44(s0)
    54c0: 02452583     	lw	a1, 0x24(a0)
    54c4: f4b42c23     	sw	a1, -0xa8(s0)
    54c8: 02852583     	lw	a1, 0x28(a0)
    54cc: fab42a23     	sw	a1, -0x4c(s0)
    54d0: 02c52583     	lw	a1, 0x2c(a0)
    54d4: fab42223     	sw	a1, -0x5c(s0)
    54d8: 07052583     	lw	a1, 0x70(a0)
    54dc: fab42423     	sw	a1, -0x58(s0)
    54e0: 07452583     	lw	a1, 0x74(a0)
    54e4: f8b42823     	sw	a1, -0x70(s0)
    54e8: 07852303     	lw	t1, 0x78(a0)
    54ec: 07c52283     	lw	t0, 0x7c(a0)
    54f0: 00052683     	lw	a3, 0x0(a0)
    54f4: 00452f83     	lw	t6, 0x4(a0)
    54f8: 00852583     	lw	a1, 0x8(a0)
    54fc: fab42c23     	sw	a1, -0x48(s0)
    5500: 00c52583     	lw	a1, 0xc(a0)
    5504: f4b42823     	sw	a1, -0xb0(s0)
    5508: 05052b83     	lw	s7, 0x50(a0)
    550c: 05452583     	lw	a1, 0x54(a0)
    5510: f6b42423     	sw	a1, -0x98(s0)
    5514: 05852983     	lw	s3, 0x58(a0)
    5518: 05c52583     	lw	a1, 0x5c(a0)
    551c: f6b42c23     	sw	a1, -0x88(s0)
    5520: 0a052b03     	lw	s6, 0xa0(a0)
    5524: 0a452903     	lw	s2, 0xa4(a0)
    5528: 0a852f03     	lw	t5, 0xa8(a0)
    552c: 0ac52483     	lw	s1, 0xac(a0)
    5530: 03052583     	lw	a1, 0x30(a0)
    5534: f8b42423     	sw	a1, -0x78(s0)
    5538: 03452583     	lw	a1, 0x34(a0)
    553c: f8b42e23     	sw	a1, -0x64(s0)
    5540: 03852583     	lw	a1, 0x38(a0)
    5544: f8b42623     	sw	a1, -0x74(s0)
    5548: 03c52583     	lw	a1, 0x3c(a0)
    554c: f8b42c23     	sw	a1, -0x68(s0)
    5550: 08052883     	lw	a7, 0x80(a0)
    5554: 08452c03     	lw	s8, 0x84(a0)
    5558: 08852d83     	lw	s11, 0x88(a0)
    555c: 08c52d03     	lw	s10, 0x8c(a0)
    5560: 01052583     	lw	a1, 0x10(a0)
    5564: fcb42023     	sw	a1, -0x40(s0)
    5568: 01452583     	lw	a1, 0x14(a0)
    556c: fcb42223     	sw	a1, -0x3c(s0)
    5570: 01852583     	lw	a1, 0x18(a0)
    5574: f4b42a23     	sw	a1, -0xac(s0)
    5578: 01c52583     	lw	a1, 0x1c(a0)
    557c: fcb42423     	sw	a1, -0x38(s0)
    5580: 06052583     	lw	a1, 0x60(a0)
    5584: f4b42e23     	sw	a1, -0xa4(s0)
    5588: 06452583     	lw	a1, 0x64(a0)
    558c: f6b42823     	sw	a1, -0x90(s0)
    5590: 06852583     	lw	a1, 0x68(a0)
    5594: f6b42a23     	sw	a1, -0x8c(s0)
    5598: 06c52583     	lw	a1, 0x6c(a0)
    559c: f6b42e23     	sw	a1, -0x84(s0)
    55a0: 0b052e83     	lw	t4, 0xb0(a0)
    55a4: 0b452583     	lw	a1, 0xb4(a0)
    55a8: f6b42023     	sw	a1, -0xa0(s0)
    55ac: 0b852583     	lw	a1, 0xb8(a0)
    55b0: f6b42223     	sw	a1, -0x9c(s0)
    55b4: 0bc52583     	lw	a1, 0xbc(a0)
    55b8: f6b42623     	sw	a1, -0x94(s0)
    55bc: 04052583     	lw	a1, 0x40(a0)
    55c0: fab42023     	sw	a1, -0x60(s0)
    55c4: 04452583     	lw	a1, 0x44(a0)
    55c8: f8b42223     	sw	a1, -0x7c(s0)
    55cc: 04852583     	lw	a1, 0x48(a0)
    55d0: fab42623     	sw	a1, -0x54(s0)
    55d4: 04c52583     	lw	a1, 0x4c(a0)
    55d8: fab42823     	sw	a1, -0x50(s0)
    55dc: 09052583     	lw	a1, 0x90(a0)
    55e0: f8b42023     	sw	a1, -0x80(s0)
    55e4: 09452e03     	lw	t3, 0x94(a0)
    55e8: 09852803     	lw	a6, 0x98(a0)
    55ec: 09c52583     	lw	a1, 0x9c(a0)
    55f0: f8b42a23     	sw	a1, -0x6c(s0)
    55f4: 0c052783     	lw	a5, 0xc0(a0)
    55f8: eea42a23     	sw	a0, -0x10c(s0)
    55fc: 0c452703     	lw	a4, 0xc4(a0)
    5600: 04200537     	lui	a0, 0x4200
    5604: 23850513     	addi	a0, a0, 0x238
    5608: 40c50633     	sub	a2, a0, a2
    560c: 0c060c93     	addi	s9, a2, 0xc0
    5610: 0c050513     	addi	a0, a0, 0xc0
    5614: eea42c23     	sw	a0, -0x108(s0)
    5618: eef42e23     	sw	a5, -0x104(s0)
    561c: f0542c23     	sw	t0, -0xe8(s0)
    5620: f1c42823     	sw	t3, -0xf0(s0)
    5624: f2642223     	sw	t1, -0xdc(s0)
    5628: f3a42e23     	sw	s10, -0xc4(s0)
    562c: f5942623     	sw	s9, -0xb4(s0)
    5630: f3242823     	sw	s2, -0xd0(s0)
    5634: 0122c533     	xor	a0, t0, s2
    5638: f6842583     	lw	a1, -0x98(s0)
    563c: fa442603     	lw	a2, -0x5c(s0)
    5640: 00c5c633     	xor	a2, a1, a2
    5644: 00c54633     	xor	a2, a0, a2
    5648: f3642c23     	sw	s6, -0xc8(s0)
    564c: 01634533     	xor	a0, t1, s6
    5650: 00068a93     	mv	s5, a3
    5654: f3742023     	sw	s7, -0xe0(s0)
    5658: fb442583     	lw	a1, -0x4c(s0)
    565c: 00bbc6b3     	xor	a3, s7, a1
    5660: 00d54333     	xor	t1, a0, a3
    5664: f5142223     	sw	a7, -0xbc(s0)
    5668: f5e42423     	sw	t5, -0xb8(s0)
    566c: 01e8c533     	xor	a0, a7, t5
    5670: f3342423     	sw	s3, -0xd8(s0)
    5674: f8842683     	lw	a3, -0x78(s0)
    5678: 00d9c6b3     	xor	a3, s3, a3
    567c: 00d54533     	xor	a0, a0, a3
    5680: f1842223     	sw	s8, -0xfc(s0)
    5684: f0942423     	sw	s1, -0xf8(s0)
    5688: 009c46b3     	xor	a3, s8, s1
    568c: 00070393     	mv	t2, a4
    5690: f4e42023     	sw	a4, -0xc0(s0)
    5694: f7842703     	lw	a4, -0x88(s0)
    5698: f9c42883     	lw	a7, -0x64(s0)
    569c: 01174733     	xor	a4, a4, a7
    56a0: 00e6c6b3     	xor	a3, a3, a4
    56a4: f3d42623     	sw	t4, -0xd4(s0)
    56a8: f1b42e23     	sw	s11, -0xe4(s0)
    56ac: 01ddc733     	xor	a4, s11, t4
    56b0: f5c42883     	lw	a7, -0xa4(s0)
    56b4: f8c42283     	lw	t0, -0x74(s0)
    56b8: 0058c2b3     	xor	t0, a7, t0
    56bc: 005742b3     	xor	t0, a4, t0
    56c0: f6042703     	lw	a4, -0xa0(s0)
    56c4: 00ed4733     	xor	a4, s10, a4
    56c8: 000f8d13     	mv	s10, t6
    56cc: f7042883     	lw	a7, -0x90(s0)
    56d0: f9842e83     	lw	t4, -0x68(s0)
    56d4: 01d8ceb3     	xor	t4, a7, t4
    56d8: 01d74eb3     	xor	t4, a4, t4
    56dc: f6442703     	lw	a4, -0x9c(s0)
    56e0: f8042883     	lw	a7, -0x80(s0)
    56e4: 00e8c733     	xor	a4, a7, a4
    56e8: f7442883     	lw	a7, -0x8c(s0)
    56ec: fa042f03     	lw	t5, -0x60(s0)
    56f0: 01e8cf33     	xor	t5, a7, t5
    56f4: 01e74f33     	xor	t5, a4, t5
    56f8: f6c42703     	lw	a4, -0x94(s0)
    56fc: 00ee4733     	xor	a4, t3, a4
    5700: f8442883     	lw	a7, -0x7c(s0)
    5704: f7c42e03     	lw	t3, -0x84(s0)
    5708: 011e4fb3     	xor	t6, t3, a7
    570c: 01f74b33     	xor	s6, a4, t6
    5710: f1042623     	sw	a6, -0xf4(s0)
    5714: 00f84733     	xor	a4, a6, a5
    5718: fa842783     	lw	a5, -0x58(s0)
    571c: fac42483     	lw	s1, -0x54(s0)
    5720: 0097c4b3     	xor	s1, a5, s1
    5724: 009744b3     	xor	s1, a4, s1
    5728: f9442583     	lw	a1, -0x6c(s0)
    572c: 0075c733     	xor	a4, a1, t2
    5730: f9042803     	lw	a6, -0x70(s0)
    5734: fb042783     	lw	a5, -0x50(s0)
    5738: 00f84fb3     	xor	t6, a6, a5
    573c: f5042c83     	lw	s9, -0xb0(s0)
    5740: 01f74db3     	xor	s11, a4, t6
    5744: 0196ce33     	xor	t3, a3, s9
    5748: fb842683     	lw	a3, -0x48(s0)
    574c: 00d54833     	xor	a6, a0, a3
    5750: 01f85693     	srli	a3, a6, 0x1f
    5754: 001e1f93     	slli	t6, t3, 0x1
    5758: 00dfefb3     	or	t6, t6, a3
    575c: 00181693     	slli	a3, a6, 0x1
    5760: 01fe5993     	srli	s3, t3, 0x1f
    5764: 0136e9b3     	or	s3, a3, s3
    5768: fc442503     	lw	a0, -0x3c(s0)
    576c: 00aec733     	xor	a4, t4, a0
    5770: fc042503     	lw	a0, -0x40(s0)
    5774: 00a2ceb3     	xor	t4, t0, a0
    5778: 001e9293     	slli	t0, t4, 0x1
    577c: 01f75b93     	srli	s7, a4, 0x1f
    5780: 0172ebb3     	or	s7, t0, s7
    5784: 01fed293     	srli	t0, t4, 0x1f
    5788: 00171093     	slli	ra, a4, 0x1
    578c: 0050e0b3     	or	ra, ra, t0
    5790: fc842503     	lw	a0, -0x38(s0)
    5794: 00ab4933     	xor	s2, s6, a0
    5798: f5442883     	lw	a7, -0xac(s0)
    579c: 011f4b33     	xor	s6, t5, a7
    57a0: 01fb5f13     	srli	t5, s6, 0x1f
    57a4: 00191293     	slli	t0, s2, 0x1
    57a8: 01e2e2b3     	or	t0, t0, t5
    57ac: 001b1f13     	slli	t5, s6, 0x1
    57b0: 01f95793     	srli	a5, s2, 0x1f
    57b4: 00ff67b3     	or	a5, t5, a5
    57b8: f5842383     	lw	t2, -0xa8(s0)
    57bc: 007dca33     	xor	s4, s11, t2
    57c0: fbc42503     	lw	a0, -0x44(s0)
    57c4: 00a4c4b3     	xor	s1, s1, a0
    57c8: 00149d93     	slli	s11, s1, 0x1
    57cc: 01fa5593     	srli	a1, s4, 0x1f
    57d0: 00bde5b3     	or	a1, s11, a1
    57d4: 01f4dd93     	srli	s11, s1, 0x1f
    57d8: 001a1f13     	slli	t5, s4, 0x1
    57dc: 01bf6f33     	or	t5, t5, s11
    57e0: 000a8693     	mv	a3, s5
    57e4: 01534333     	xor	t1, t1, s5
    57e8: 01a64ab3     	xor	s5, a2, s10
    57ec: 00131d93     	slli	s11, t1, 0x1
    57f0: 01fad613     	srli	a2, s5, 0x1f
    57f4: 00cde633     	or	a2, s11, a2
    57f8: 01f35d93     	srli	s11, t1, 0x1f
    57fc: 001a9c13     	slli	s8, s5, 0x1
    5800: 01bc6c33     	or	s8, s8, s11
    5804: 0099c4b3     	xor	s1, s3, s1
    5808: 014fcfb3     	xor	t6, t6, s4
    580c: 001ac9b3     	xor	s3, s5, ra
    5810: 01734333     	xor	t1, t1, s7
    5814: 00f84533     	xor	a0, a6, a5
    5818: 005e42b3     	xor	t0, t3, t0
    581c: 01e74ab3     	xor	s5, a4, t5
    5820: 00becbb3     	xor	s7, t4, a1
    5824: 012c4db3     	xor	s11, s8, s2
    5828: 016645b3     	xor	a1, a2, s6
    582c: 01afc633     	xor	a2, t6, s10
    5830: f2c42a23     	sw	a2, -0xcc(s0)
    5834: fa442603     	lw	a2, -0x5c(s0)
    5838: 00cfc0b3     	xor	ra, t6, a2
    583c: f6842603     	lw	a2, -0x98(s0)
    5840: 00cfc933     	xor	s2, t6, a2
    5844: f1842603     	lw	a2, -0xe8(s0)
    5848: 00cfc633     	xor	a2, t6, a2
    584c: f0c42023     	sw	a2, -0x100(s0)
    5850: f3042603     	lw	a2, -0xd0(s0)
    5854: 00cfc633     	xor	a2, t6, a2
    5858: f0c42c23     	sw	a2, -0xe8(s0)
    585c: 00d4c633     	xor	a2, s1, a3
    5860: fac42223     	sw	a2, -0x5c(s0)
    5864: 00038613     	mv	a2, t2
    5868: fb442683     	lw	a3, -0x4c(s0)
    586c: 00d4cd33     	xor	s10, s1, a3
    5870: f2042703     	lw	a4, -0xe0(s0)
    5874: 00e4c3b3     	xor	t2, s1, a4
    5878: f2442703     	lw	a4, -0xdc(s0)
    587c: 00e4c733     	xor	a4, s1, a4
    5880: f2e42023     	sw	a4, -0xe0(s0)
    5884: f3842703     	lw	a4, -0xc8(s0)
    5888: 00e4c733     	xor	a4, s1, a4
    588c: f0e42a23     	sw	a4, -0xec(s0)
    5890: 0199c7b3     	xor	a5, s3, s9
    5894: f9c42703     	lw	a4, -0x64(s0)
    5898: 00e9c733     	xor	a4, s3, a4
    589c: f6e42423     	sw	a4, -0x98(s0)
    58a0: f7842703     	lw	a4, -0x88(s0)
    58a4: 00e9c4b3     	xor	s1, s3, a4
    58a8: f0442703     	lw	a4, -0xfc(s0)
    58ac: 00e9ccb3     	xor	s9, s3, a4
    58b0: f0842703     	lw	a4, -0xf8(s0)
    58b4: 00e9c733     	xor	a4, s3, a4
    58b8: f2e42c23     	sw	a4, -0xc8(s0)
    58bc: fb842703     	lw	a4, -0x48(s0)
    58c0: 00e34733     	xor	a4, t1, a4
    58c4: f8842e03     	lw	t3, -0x78(s0)
    58c8: 01c34e33     	xor	t3, t1, t3
    58cc: fbc42c23     	sw	t3, -0x48(s0)
    58d0: f2842e03     	lw	t3, -0xd8(s0)
    58d4: 01c34e33     	xor	t3, t1, t3
    58d8: f4442e83     	lw	t4, -0xbc(s0)
    58dc: 01d34c33     	xor	s8, t1, t4
    58e0: f4842e83     	lw	t4, -0xb8(s0)
    58e4: 01d34833     	xor	a6, t1, t4
    58e8: f5042423     	sw	a6, -0xb8(s0)
    58ec: fc042803     	lw	a6, -0x40(s0)
    58f0: 01054833     	xor	a6, a0, a6
    58f4: f9042423     	sw	a6, -0x78(s0)
    58f8: f8c42803     	lw	a6, -0x74(s0)
    58fc: 01054833     	xor	a6, a0, a6
    5900: f5c42303     	lw	t1, -0xa4(s0)
    5904: 00654333     	xor	t1, a0, t1
    5908: f4642e23     	sw	t1, -0xa4(s0)
    590c: f1c42303     	lw	t1, -0xe4(s0)
    5910: 00654f33     	xor	t5, a0, t1
    5914: f2c42303     	lw	t1, -0xd4(s0)
    5918: 00654533     	xor	a0, a0, t1
    591c: fca42023     	sw	a0, -0x40(s0)
    5920: fc442503     	lw	a0, -0x3c(s0)
    5924: 00a2c533     	xor	a0, t0, a0
    5928: f8a42623     	sw	a0, -0x74(s0)
    592c: f9842503     	lw	a0, -0x68(s0)
    5930: 00a2c533     	xor	a0, t0, a0
    5934: f7042303     	lw	t1, -0x90(s0)
    5938: 0062c333     	xor	t1, t0, t1
    593c: f8642c23     	sw	t1, -0x68(s0)
    5940: f3c42303     	lw	t1, -0xc4(s0)
    5944: 0062c333     	xor	t1, t0, t1
    5948: f6042e83     	lw	t4, -0xa0(s0)
    594c: 01d2c2b3     	xor	t0, t0, t4
    5950: fc542223     	sw	t0, -0x3c(s0)
    5954: 011bcfb3     	xor	t6, s7, a7
    5958: fa042883     	lw	a7, -0x60(s0)
    595c: 011bcb33     	xor	s6, s7, a7
    5960: f7442883     	lw	a7, -0x8c(s0)
    5964: 011bc8b3     	xor	a7, s7, a7
    5968: f5142a23     	sw	a7, -0xac(s0)
    596c: f8042883     	lw	a7, -0x80(s0)
    5970: 011bc2b3     	xor	t0, s7, a7
    5974: f6442883     	lw	a7, -0x9c(s0)
    5978: 011bc8b3     	xor	a7, s7, a7
    597c: f3142423     	sw	a7, -0xd8(s0)
    5980: fc842883     	lw	a7, -0x38(s0)
    5984: 011aceb3     	xor	t4, s5, a7
    5988: f8442883     	lw	a7, -0x7c(s0)
    598c: 011ac9b3     	xor	s3, s5, a7
    5990: f7c42883     	lw	a7, -0x84(s0)
    5994: 011ac8b3     	xor	a7, s5, a7
    5998: f5142823     	sw	a7, -0xb0(s0)
    599c: f1042883     	lw	a7, -0xf0(s0)
    59a0: 011ac8b3     	xor	a7, s5, a7
    59a4: f6c42a03     	lw	s4, -0x94(s0)
    59a8: 014aca33     	xor	s4, s5, s4
    59ac: f1442823     	sw	s4, -0xf0(s0)
    59b0: 00cdcbb3     	xor	s7, s11, a2
    59b4: fb042603     	lw	a2, -0x50(s0)
    59b8: 00cdc633     	xor	a2, s11, a2
    59bc: fcc42423     	sw	a2, -0x38(s0)
    59c0: f9042603     	lw	a2, -0x70(s0)
    59c4: 00cdc633     	xor	a2, s11, a2
    59c8: f6c42a23     	sw	a2, -0x8c(s0)
    59cc: f9442603     	lw	a2, -0x6c(s0)
    59d0: 00cdc633     	xor	a2, s11, a2
    59d4: f4c42c23     	sw	a2, -0xa8(s0)
    59d8: f4042603     	lw	a2, -0xc0(s0)
    59dc: 00cdca33     	xor	s4, s11, a2
    59e0: fbc42603     	lw	a2, -0x44(s0)
    59e4: 00c5cab3     	xor	s5, a1, a2
    59e8: fac42683     	lw	a3, -0x54(s0)
    59ec: 00d5c6b3     	xor	a3, a1, a3
    59f0: fad42e23     	sw	a3, -0x44(s0)
    59f4: fa842603     	lw	a2, -0x58(s0)
    59f8: 00c5c633     	xor	a2, a1, a2
    59fc: f6c42823     	sw	a2, -0x90(s0)
    5a00: f0c42603     	lw	a2, -0xf4(s0)
    5a04: 00c5cdb3     	xor	s11, a1, a2
    5a08: efc42603     	lw	a2, -0x104(s0)
    5a0c: 00c5c5b3     	xor	a1, a1, a2
    5a10: 01f7d693     	srli	a3, a5, 0x1f
    5a14: 00171613     	slli	a2, a4, 0x1
    5a18: 00d66633     	or	a2, a2, a3
    5a1c: fac42423     	sw	a2, -0x58(s0)
    5a20: 01f75713     	srli	a4, a4, 0x1f
    5a24: 00179793     	slli	a5, a5, 0x1
    5a28: 00e7e733     	or	a4, a5, a4
    5a2c: f8e42823     	sw	a4, -0x70(s0)
    5a30: 01d3d613     	srli	a2, t2, 0x1d
    5a34: 00391693     	slli	a3, s2, 0x3
    5a38: 00c6e633     	or	a2, a3, a2
    5a3c: fac42823     	sw	a2, -0x50(s0)
    5a40: 01d95613     	srli	a2, s2, 0x1d
    5a44: 00339393     	slli	t2, t2, 0x3
    5a48: 00c3e633     	or	a2, t2, a2
    5a4c: fac42623     	sw	a2, -0x54(s0)
    5a50: 01a55613     	srli	a2, a0, 0x1a
    5a54: 00681693     	slli	a3, a6, 0x6
    5a58: 00c6e633     	or	a2, a3, a2
    5a5c: f6c42e23     	sw	a2, -0x84(s0)
    5a60: 01a85613     	srli	a2, a6, 0x1a
    5a64: 00651513     	slli	a0, a0, 0x6
    5a68: 00c56533     	or	a0, a0, a2
    5a6c: f6a42c23     	sw	a0, -0x88(s0)
    5a70: 016e5513     	srli	a0, t3, 0x16
    5a74: 00a49613     	slli	a2, s1, 0xa
    5a78: 00a66533     	or	a0, a2, a0
    5a7c: f6a42223     	sw	a0, -0x9c(s0)
    5a80: 0164d493     	srli	s1, s1, 0x16
    5a84: 00ae1e13     	slli	t3, t3, 0xa
    5a88: 009e6533     	or	a0, t3, s1
    5a8c: f8a42a23     	sw	a0, -0x6c(s0)
    5a90: 011f5513     	srli	a0, t5, 0x11
    5a94: 00f31613     	slli	a2, t1, 0xf
    5a98: 00a66533     	or	a0, a2, a0
    5a9c: f8a42023     	sw	a0, -0x80(s0)
    5aa0: 01135513     	srli	a0, t1, 0x11
    5aa4: 00ff1f13     	slli	t5, t5, 0xf
    5aa8: 00af6533     	or	a0, t5, a0
    5aac: f6a42023     	sw	a0, -0xa0(s0)
    5ab0: 00b2d513     	srli	a0, t0, 0xb
    5ab4: 01589613     	slli	a2, a7, 0x15
    5ab8: 00a66933     	or	s2, a2, a0
    5abc: 00b8d513     	srli	a0, a7, 0xb
    5ac0: 01529293     	slli	t0, t0, 0x15
    5ac4: 00a2e533     	or	a0, t0, a0
    5ac8: f8a42e23     	sw	a0, -0x64(s0)
    5acc: 004ed513     	srli	a0, t4, 0x4
    5ad0: 01cf9613     	slli	a2, t6, 0x1c
    5ad4: 00a66533     	or	a0, a2, a0
    5ad8: faa42023     	sw	a0, -0x60(s0)
    5adc: 004fd513     	srli	a0, t6, 0x4
    5ae0: 01ce9e93     	slli	t4, t4, 0x1c
    5ae4: 00aee533     	or	a0, t4, a0
    5ae8: faa42a23     	sw	a0, -0x4c(s0)
    5aec: 01cd5513     	srli	a0, s10, 0x1c
    5af0: 00409613     	slli	a2, ra, 0x4
    5af4: 00a66533     	or	a0, a2, a0
    5af8: f2a42e23     	sw	a0, -0xc4(s0)
    5afc: 01c0d513     	srli	a0, ra, 0x1c
    5b00: 004d1d13     	slli	s10, s10, 0x4
    5b04: 00ad6d33     	or	s10, s10, a0
    5b08: 013cd513     	srli	a0, s9, 0x13
    5b0c: 00dc1613     	slli	a2, s8, 0xd
    5b10: 00a66533     	or	a0, a2, a0
    5b14: f8a42223     	sw	a0, -0x7c(s0)
    5b18: 013c5513     	srli	a0, s8, 0x13
    5b1c: 00dc9c93     	slli	s9, s9, 0xd
    5b20: 00ace0b3     	or	ra, s9, a0
    5b24: f4c42c83     	lw	s9, -0xb4(s0)
    5b28: 009b5513     	srli	a0, s6, 0x9
    5b2c: 01799613     	slli	a2, s3, 0x17
    5b30: 00a66533     	or	a0, a2, a0
    5b34: f6a42623     	sw	a0, -0x94(s0)
    5b38: 0099d513     	srli	a0, s3, 0x9
    5b3c: 017b1b13     	slli	s6, s6, 0x17
    5b40: 00ab6533     	or	a0, s6, a0
    5b44: f4a42223     	sw	a0, -0xbc(s0)
    5b48: f4842703     	lw	a4, -0xb8(s0)
    5b4c: 01e75513     	srli	a0, a4, 0x1e
    5b50: f3842683     	lw	a3, -0xc8(s0)
    5b54: 00269613     	slli	a2, a3, 0x2
    5b58: 00a66533     	or	a0, a2, a0
    5b5c: f4a42023     	sw	a0, -0xc0(s0)
    5b60: 01e6d513     	srli	a0, a3, 0x1e
    5b64: 00271613     	slli	a2, a4, 0x2
    5b68: 00a66533     	or	a0, a2, a0
    5b6c: f4a42423     	sw	a0, -0xb8(s0)
    5b70: 0125d513     	srli	a0, a1, 0x12
    5b74: 00ea1613     	slli	a2, s4, 0xe
    5b78: 00a663b3     	or	t2, a2, a0
    5b7c: 012a5513     	srli	a0, s4, 0x12
    5b80: 00e59593     	slli	a1, a1, 0xe
    5b84: 00a5eeb3     	or	t4, a1, a0
    5b88: 005bd513     	srli	a0, s7, 0x5
    5b8c: 01ba9593     	slli	a1, s5, 0x1b
    5b90: 00a5e533     	or	a0, a1, a0
    5b94: f2a42623     	sw	a0, -0xd4(s0)
    5b98: 005ad513     	srli	a0, s5, 0x5
    5b9c: 01bb9b93     	slli	s7, s7, 0x1b
    5ba0: 00abe533     	or	a0, s7, a0
    5ba4: f2a42223     	sw	a0, -0xdc(s0)
    5ba8: f0042603     	lw	a2, -0x100(s0)
    5bac: 01765513     	srli	a0, a2, 0x17
    5bb0: f2042683     	lw	a3, -0xe0(s0)
    5bb4: 00969593     	slli	a1, a3, 0x9
    5bb8: 00a5e533     	or	a0, a1, a0
    5bbc: f2a42823     	sw	a0, -0xd0(s0)
    5bc0: 0176d513     	srli	a0, a3, 0x17
    5bc4: 00961593     	slli	a1, a2, 0x9
    5bc8: 00a5e533     	or	a0, a1, a0
    5bcc: f2a42c23     	sw	a0, -0xc8(s0)
    5bd0: f1042683     	lw	a3, -0xf0(s0)
    5bd4: 0086d513     	srli	a0, a3, 0x8
    5bd8: f2842603     	lw	a2, -0xd8(s0)
    5bdc: 01861593     	slli	a1, a2, 0x18
    5be0: 00a5e533     	or	a0, a1, a0
    5be4: f0a42e23     	sw	a0, -0xe4(s0)
    5be8: 00865513     	srli	a0, a2, 0x8
    5bec: 01869593     	slli	a1, a3, 0x18
    5bf0: 00a5e533     	or	a0, a1, a0
    5bf4: f2a42023     	sw	a0, -0xe0(s0)
    5bf8: 018dd513     	srli	a0, s11, 0x18
    5bfc: f5842603     	lw	a2, -0xa8(s0)
    5c00: 00861593     	slli	a1, a2, 0x8
    5c04: 00a5ec33     	or	s8, a1, a0
    5c08: 01865513     	srli	a0, a2, 0x18
    5c0c: 008d9d93     	slli	s11, s11, 0x8
    5c10: 00adeab3     	or	s5, s11, a0
    5c14: f5442603     	lw	a2, -0xac(s0)
    5c18: 00765513     	srli	a0, a2, 0x7
    5c1c: f5042683     	lw	a3, -0xb0(s0)
    5c20: 01969593     	slli	a1, a3, 0x19
    5c24: 00a5ee33     	or	t3, a1, a0
    5c28: 0076d513     	srli	a0, a3, 0x7
    5c2c: 01961593     	slli	a1, a2, 0x19
    5c30: 00a5ea33     	or	s4, a1, a0
    5c34: f5c42603     	lw	a2, -0xa4(s0)
    5c38: 01565513     	srli	a0, a2, 0x15
    5c3c: f9842683     	lw	a3, -0x68(s0)
    5c40: 00b69593     	slli	a1, a3, 0xb
    5c44: 00a5e8b3     	or	a7, a1, a0
    5c48: 0156d513     	srli	a0, a3, 0x15
    5c4c: 00b61613     	slli	a2, a2, 0xb
    5c50: 00a66633     	or	a2, a2, a0
    5c54: f8842683     	lw	a3, -0x78(s0)
    5c58: 0026d513     	srli	a0, a3, 0x2
    5c5c: f8c42703     	lw	a4, -0x74(s0)
    5c60: 01e71593     	slli	a1, a4, 0x1e
    5c64: 00a5e533     	or	a0, a1, a0
    5c68: f2a42423     	sw	a0, -0xd8(s0)
    5c6c: 00275513     	srli	a0, a4, 0x2
    5c70: 01e69593     	slli	a1, a3, 0x1e
    5c74: 00a5ef33     	or	t5, a1, a0
    5c78: f1442703     	lw	a4, -0xec(s0)
    5c7c: 00e75513     	srli	a0, a4, 0xe
    5c80: f1842683     	lw	a3, -0xe8(s0)
    5c84: 01269593     	slli	a1, a3, 0x12
    5c88: 00a5e333     	or	t1, a1, a0
    5c8c: 00e6d513     	srli	a0, a3, 0xe
    5c90: 01271713     	slli	a4, a4, 0x12
    5c94: 00a76733     	or	a4, a4, a0
    5c98: f7442683     	lw	a3, -0x8c(s0)
    5c9c: 0196d513     	srli	a0, a3, 0x19
    5ca0: f7042783     	lw	a5, -0x90(s0)
    5ca4: 00779593     	slli	a1, a5, 0x7
    5ca8: 00a5e533     	or	a0, a1, a0
    5cac: f0a42a23     	sw	a0, -0xec(s0)
    5cb0: 0197d513     	srli	a0, a5, 0x19
    5cb4: 00769593     	slli	a1, a3, 0x7
    5cb8: 00a5e533     	or	a0, a1, a0
    5cbc: f0a42c23     	sw	a0, -0xe8(s0)
    5cc0: fc442683     	lw	a3, -0x3c(s0)
    5cc4: 0036d513     	srli	a0, a3, 0x3
    5cc8: fc042583     	lw	a1, -0x40(s0)
    5ccc: 01d59793     	slli	a5, a1, 0x1d
    5cd0: 00a7e7b3     	or	a5, a5, a0
    5cd4: 0035d513     	srli	a0, a1, 0x3
    5cd8: 01d69593     	slli	a1, a3, 0x1d
    5cdc: 00a5e833     	or	a6, a1, a0
    5ce0: fc842583     	lw	a1, -0x38(s0)
    5ce4: 00c5d513     	srli	a0, a1, 0xc
    5ce8: fbc42283     	lw	t0, -0x44(s0)
    5cec: 01429693     	slli	a3, t0, 0x14
    5cf0: 00a6e6b3     	or	a3, a3, a0
    5cf4: 00c2d513     	srli	a0, t0, 0xc
    5cf8: 01459593     	slli	a1, a1, 0x14
    5cfc: 00a5e5b3     	or	a1, a1, a0
    5d00: f6842f83     	lw	t6, -0x98(s0)
    5d04: 014fd513     	srli	a0, t6, 0x14
    5d08: fb842483     	lw	s1, -0x48(s0)
    5d0c: 00c49293     	slli	t0, s1, 0xc
    5d10: 00a2e533     	or	a0, t0, a0
    5d14: 004cab03     	lw	s6, 0x4(s9)
    5d18: 0144d293     	srli	t0, s1, 0x14
    5d1c: 00cf9f93     	slli	t6, t6, 0xc
    5d20: 005fe2b3     	or	t0, t6, t0
    5d24: f3442983     	lw	s3, -0xcc(s0)
    5d28: 013b4fb3     	xor	t6, s6, s3
    5d2c: f1f42823     	sw	t6, -0xf0(s0)
    5d30: fff3cf93     	not	t6, t2
    5d34: 01f9f4b3     	and	s1, s3, t6
    5d38: fff9cf93     	not	t6, s3
    5d3c: 01f57fb3     	and	t6, a0, t6
    5d40: 000ca983     	lw	s3, 0x0(s9)
    5d44: fff94b13     	not	s6, s2
    5d48: 0163fb33     	and	s6, t2, s6
    5d4c: 007fc3b3     	xor	t2, t6, t2
    5d50: f4742c23     	sw	t2, -0xa8(s0)
    5d54: fa442f83     	lw	t6, -0x5c(s0)
    5d58: 01f9c3b3     	xor	t2, s3, t6
    5d5c: f2742a23     	sw	t2, -0xcc(s0)
    5d60: fffec393     	not	t2, t4
    5d64: 007ff3b3     	and	t2, t6, t2
    5d68: ffffc993     	not	s3, t6
    5d6c: 0132f9b3     	and	s3, t0, s3
    5d70: f9c42f83     	lw	t6, -0x64(s0)
    5d74: ffffcb93     	not	s7, t6
    5d78: 017efbb3     	and	s7, t4, s7
    5d7c: 01d9ceb3     	xor	t4, s3, t4
    5d80: fbd42e23     	sw	t4, -0x44(s0)
    5d84: fff64e93     	not	t4, a2
    5d88: 01d97eb3     	and	t4, s2, t4
    5d8c: 01d54eb3     	xor	t4, a0, t4
    5d90: f5d42823     	sw	t4, -0xb0(s0)
    5d94: fff54513     	not	a0, a0
    5d98: 00a67533     	and	a0, a2, a0
    5d9c: f0a42623     	sw	a0, -0xf4(s0)
    5da0: 00cb4533     	xor	a0, s6, a2
    5da4: fca42223     	sw	a0, -0x3c(s0)
    5da8: fff8c513     	not	a0, a7
    5dac: 00aff533     	and	a0, t6, a0
    5db0: 00a2c533     	xor	a0, t0, a0
    5db4: faa42c23     	sw	a0, -0x48(s0)
    5db8: fff2c513     	not	a0, t0
    5dbc: 00a8f633     	and	a2, a7, a0
    5dc0: 011bc533     	xor	a0, s7, a7
    5dc4: fca42023     	sw	a0, -0x40(s0)
    5dc8: 0124c533     	xor	a0, s1, s2
    5dcc: fca42423     	sw	a0, -0x38(s0)
    5dd0: 01f3c533     	xor	a0, t2, t6
    5dd4: f4a42a23     	sw	a0, -0xac(s0)
    5dd8: fff5c513     	not	a0, a1
    5ddc: fb042e83     	lw	t4, -0x50(s0)
    5de0: 00aef533     	and	a0, t4, a0
    5de4: fff7c893     	not	a7, a5
    5de8: fb442283     	lw	t0, -0x4c(s0)
    5dec: 0112f8b3     	and	a7, t0, a7
    5df0: 00554533     	xor	a0, a0, t0
    5df4: faa42223     	sw	a0, -0x5c(s0)
    5df8: fff2c513     	not	a0, t0
    5dfc: fffec293     	not	t0, t4
    5e00: f8442f83     	lw	t6, -0x7c(s0)
    5e04: 005ff2b3     	and	t0, t6, t0
    5e08: 00a5f533     	and	a0, a1, a0
    5e0c: 00b2c5b3     	xor	a1, t0, a1
    5e10: f8b42e23     	sw	a1, -0x64(s0)
    5e14: fff6c593     	not	a1, a3
    5e18: fac42903     	lw	s2, -0x54(s0)
    5e1c: 00b975b3     	and	a1, s2, a1
    5e20: fff84293     	not	t0, a6
    5e24: fa042383     	lw	t2, -0x60(s0)
    5e28: 0053f2b3     	and	t0, t2, t0
    5e2c: 0075c5b3     	xor	a1, a1, t2
    5e30: fab42a23     	sw	a1, -0x4c(s0)
    5e34: fff3c593     	not	a1, t2
    5e38: fff94393     	not	t2, s2
    5e3c: 0070f3b3     	and	t2, ra, t2
    5e40: 00b6f5b3     	and	a1, a3, a1
    5e44: 00d3c6b3     	xor	a3, t2, a3
    5e48: f8d42423     	sw	a3, -0x78(s0)
    5e4c: ffffc693     	not	a3, t6
    5e50: 00d7f6b3     	and	a3, a5, a3
    5e54: 01d6c6b3     	xor	a3, a3, t4
    5e58: f8d42c23     	sw	a3, -0x68(s0)
    5e5c: fff0c693     	not	a3, ra
    5e60: 00d876b3     	and	a3, a6, a3
    5e64: 0126c6b3     	xor	a3, a3, s2
    5e68: f8d42623     	sw	a3, -0x74(s0)
    5e6c: 011fc6b3     	xor	a3, t6, a7
    5e70: f8d42223     	sw	a3, -0x7c(s0)
    5e74: 0050c6b3     	xor	a3, ra, t0
    5e78: fad42023     	sw	a3, -0x60(s0)
    5e7c: 00f54533     	xor	a0, a0, a5
    5e80: faa42823     	sw	a0, -0x50(s0)
    5e84: 0105c533     	xor	a0, a1, a6
    5e88: faa42623     	sw	a0, -0x54(s0)
    5e8c: f7842903     	lw	s2, -0x88(s0)
    5e90: fff94513     	not	a0, s2
    5e94: 00ae7533     	and	a0, t3, a0
    5e98: fff34593     	not	a1, t1
    5e9c: f9042683     	lw	a3, -0x70(s0)
    5ea0: 00b6f5b3     	and	a1, a3, a1
    5ea4: 00a6c533     	xor	a0, a3, a0
    5ea8: f6a42423     	sw	a0, -0x98(s0)
    5eac: fff6c513     	not	a0, a3
    5eb0: fffe4693     	not	a3, t3
    5eb4: 00dc76b3     	and	a3, s8, a3
    5eb8: 00a97533     	and	a0, s2, a0
    5ebc: 0126c6b3     	xor	a3, a3, s2
    5ec0: f6d42c23     	sw	a3, -0x88(s0)
    5ec4: f7c42903     	lw	s2, -0x84(s0)
    5ec8: fff94693     	not	a3, s2
    5ecc: 00da76b3     	and	a3, s4, a3
    5ed0: fff74793     	not	a5, a4
    5ed4: fa842803     	lw	a6, -0x58(s0)
    5ed8: 00f877b3     	and	a5, a6, a5
    5edc: 00d84bb3     	xor	s7, a6, a3
    5ee0: fff84693     	not	a3, a6
    5ee4: fffa4813     	not	a6, s4
    5ee8: 010af833     	and	a6, s5, a6
    5eec: 00d976b3     	and	a3, s2, a3
    5ef0: 012849b3     	xor	s3, a6, s2
    5ef4: fffc4813     	not	a6, s8
    5ef8: 01037833     	and	a6, t1, a6
    5efc: 01c84833     	xor	a6, a6, t3
    5f00: f7042823     	sw	a6, -0x90(s0)
    5f04: fffac813     	not	a6, s5
    5f08: 01077833     	and	a6, a4, a6
    5f0c: 01484833     	xor	a6, a6, s4
    5f10: f5042e23     	sw	a6, -0xa4(s0)
    5f14: 0185c5b3     	xor	a1, a1, s8
    5f18: f6b42e23     	sw	a1, -0x84(s0)
    5f1c: 0157c5b3     	xor	a1, a5, s5
    5f20: f6b42a23     	sw	a1, -0x8c(s0)
    5f24: 00654533     	xor	a0, a0, t1
    5f28: f8a42823     	sw	a0, -0x70(s0)
    5f2c: 00e6c6b3     	xor	a3, a3, a4
    5f30: fad42423     	sw	a3, -0x58(s0)
    5f34: fffd4513     	not	a0, s10
    5f38: f6442803     	lw	a6, -0x9c(s0)
    5f3c: 00a87533     	and	a0, a6, a0
    5f40: f1c42e83     	lw	t4, -0xe4(s0)
    5f44: fffec593     	not	a1, t4
    5f48: f2442683     	lw	a3, -0xdc(s0)
    5f4c: 00b6f5b3     	and	a1, a3, a1
    5f50: 00d542b3     	xor	t0, a0, a3
    5f54: fff6c513     	not	a0, a3
    5f58: fff84693     	not	a3, a6
    5f5c: f8042a03     	lw	s4, -0x80(s0)
    5f60: 00da76b3     	and	a3, s4, a3
    5f64: 00ad7533     	and	a0, s10, a0
    5f68: 01a6cc33     	xor	s8, a3, s10
    5f6c: f3c42f83     	lw	t6, -0xc4(s0)
    5f70: ffffc693     	not	a3, t6
    5f74: f9442e03     	lw	t3, -0x6c(s0)
    5f78: 00de76b3     	and	a3, t3, a3
    5f7c: f2042903     	lw	s2, -0xe0(s0)
    5f80: fff94713     	not	a4, s2
    5f84: f2c42783     	lw	a5, -0xd4(s0)
    5f88: 00e7f733     	and	a4, a5, a4
    5f8c: 00f6c333     	xor	t1, a3, a5
    5f90: fff7c693     	not	a3, a5
    5f94: fffe4793     	not	a5, t3
    5f98: f6042483     	lw	s1, -0xa0(s0)
    5f9c: 00f4f7b3     	and	a5, s1, a5
    5fa0: 00dff6b3     	and	a3, t6, a3
    5fa4: 01f7c8b3     	xor	a7, a5, t6
    5fa8: fffa4793     	not	a5, s4
    5fac: 00fef7b3     	and	a5, t4, a5
    5fb0: 00f84d33     	xor	s10, a6, a5
    5fb4: fff4c793     	not	a5, s1
    5fb8: 00f977b3     	and	a5, s2, a5
    5fbc: 00fe4db3     	xor	s11, t3, a5
    5fc0: 0145ce33     	xor	t3, a1, s4
    5fc4: 009745b3     	xor	a1, a4, s1
    5fc8: f8b42023     	sw	a1, -0x80(s0)
    5fcc: 01d54533     	xor	a0, a0, t4
    5fd0: f8a42a23     	sw	a0, -0x6c(s0)
    5fd4: 0126c833     	xor	a6, a3, s2
    5fd8: f4442483     	lw	s1, -0xbc(s0)
    5fdc: fff4c513     	not	a0, s1
    5fe0: f1442e83     	lw	t4, -0xec(s0)
    5fe4: 00aef533     	and	a0, t4, a0
    5fe8: f4042a03     	lw	s4, -0xc0(s0)
    5fec: fffa4593     	not	a1, s4
    5ff0: 00bf75b3     	and	a1, t5, a1
    5ff4: 01e54933     	xor	s2, a0, t5
    5ff8: ffff4513     	not	a0, t5
    5ffc: fffec693     	not	a3, t4
    6000: f3042f83     	lw	t6, -0xd0(s0)
    6004: 00dff6b3     	and	a3, t6, a3
    6008: 00a4f533     	and	a0, s1, a0
    600c: 0096c4b3     	xor	s1, a3, s1
    6010: f6c42f03     	lw	t5, -0x94(s0)
    6014: ffff4693     	not	a3, t5
    6018: f1842383     	lw	t2, -0xe8(s0)
    601c: 00d3f6b3     	and	a3, t2, a3
    6020: f4842a83     	lw	s5, -0xb8(s0)
    6024: fffac713     	not	a4, s5
    6028: f2842783     	lw	a5, -0xd8(s0)
    602c: 00e7f733     	and	a4, a5, a4
    6030: 00f6cb33     	xor	s6, a3, a5
    6034: fff7c693     	not	a3, a5
    6038: fff3c793     	not	a5, t2
    603c: f3842083     	lw	ra, -0xc8(s0)
    6040: 00f0f7b3     	and	a5, ra, a5
    6044: 00df76b3     	and	a3, t5, a3
    6048: 01e7cf33     	xor	t5, a5, t5
    604c: ffffc793     	not	a5, t6
    6050: 00fa77b3     	and	a5, s4, a5
    6054: 01d7c7b3     	xor	a5, a5, t4
    6058: f6f42023     	sw	a5, -0xa0(s0)
    605c: fff0c793     	not	a5, ra
    6060: 00faf7b3     	and	a5, s5, a5
    6064: 0077ceb3     	xor	t4, a5, t2
    6068: 01f5c5b3     	xor	a1, a1, t6
    606c: f6b42623     	sw	a1, -0x94(s0)
    6070: 001745b3     	xor	a1, a4, ra
    6074: f6b42223     	sw	a1, -0x9c(s0)
    6078: 00aa4733     	xor	a4, s4, a0
    607c: 00dac7b3     	xor	a5, s5, a3
    6080: f1042503     	lw	a0, -0xf0(s0)
    6084: f0c42583     	lw	a1, -0xf4(s0)
    6088: 00b54fb3     	xor	t6, a0, a1
    608c: 008c8c93     	addi	s9, s9, 0x8
    6090: f3442683     	lw	a3, -0xcc(s0)
    6094: 00c6c6b3     	xor	a3, a3, a2
    6098: ef842503     	lw	a0, -0x108(s0)
    609c: d6ac9e63     	bne	s9, a0, 0x5618 <keccak::keccak_p::h2c32d13bf019081a+0x1ac>
    60a0: ef442503     	lw	a0, -0x10c(s0)
    60a4: 05752823     	sw	s7, 0x50(a0)
    60a8: f6842583     	lw	a1, -0x98(s0)
    60ac: 04b52a23     	sw	a1, 0x54(a0)
    60b0: 05352c23     	sw	s3, 0x58(a0)
    60b4: f7842583     	lw	a1, -0x88(s0)
    60b8: 04b52e23     	sw	a1, 0x5c(a0)
    60bc: 0b652023     	sw	s6, 0xa0(a0)
    60c0: 0b252223     	sw	s2, 0xa4(a0)
    60c4: 0be52423     	sw	t5, 0xa8(a0)
    60c8: 0a952623     	sw	s1, 0xac(a0)
    60cc: f8842583     	lw	a1, -0x78(s0)
    60d0: 02b52823     	sw	a1, 0x30(a0)
    60d4: f9c42583     	lw	a1, -0x64(s0)
    60d8: 02b52a23     	sw	a1, 0x34(a0)
    60dc: f8c42583     	lw	a1, -0x74(s0)
    60e0: 02b52c23     	sw	a1, 0x38(a0)
    60e4: f9842583     	lw	a1, -0x68(s0)
    60e8: 02b52e23     	sw	a1, 0x3c(a0)
    60ec: 09152023     	sw	a7, 0x80(a0)
    60f0: 09852223     	sw	s8, 0x84(a0)
    60f4: 09b52423     	sw	s11, 0x88(a0)
    60f8: 09a52623     	sw	s10, 0x8c(a0)
    60fc: fc042583     	lw	a1, -0x40(s0)
    6100: 00b52823     	sw	a1, 0x10(a0)
    6104: fc442583     	lw	a1, -0x3c(s0)
    6108: 00b52a23     	sw	a1, 0x14(a0)
    610c: f5442583     	lw	a1, -0xac(s0)
    6110: 00b52c23     	sw	a1, 0x18(a0)
    6114: fc842583     	lw	a1, -0x38(s0)
    6118: 00b52e23     	sw	a1, 0x1c(a0)
    611c: f5c42583     	lw	a1, -0xa4(s0)
    6120: 06b52023     	sw	a1, 0x60(a0)
    6124: f7042583     	lw	a1, -0x90(s0)
    6128: 06b52223     	sw	a1, 0x64(a0)
    612c: f7442583     	lw	a1, -0x8c(s0)
    6130: 06b52423     	sw	a1, 0x68(a0)
    6134: f7c42583     	lw	a1, -0x84(s0)
    6138: 06b52623     	sw	a1, 0x6c(a0)
    613c: fbc42583     	lw	a1, -0x44(s0)
    6140: 02b52023     	sw	a1, 0x20(a0)
    6144: f5842583     	lw	a1, -0xa8(s0)
    6148: 02b52223     	sw	a1, 0x24(a0)
    614c: fb442583     	lw	a1, -0x4c(s0)
    6150: 02b52423     	sw	a1, 0x28(a0)
    6154: fa442583     	lw	a1, -0x5c(s0)
    6158: 02b52623     	sw	a1, 0x2c(a0)
    615c: fa842583     	lw	a1, -0x58(s0)
    6160: 06b52823     	sw	a1, 0x70(a0)
    6164: f9042583     	lw	a1, -0x70(s0)
    6168: 06b52a23     	sw	a1, 0x74(a0)
    616c: 06652c23     	sw	t1, 0x78(a0)
    6170: 06552e23     	sw	t0, 0x7c(a0)
    6174: 00d52023     	sw	a3, 0x0(a0)
    6178: 01f52223     	sw	t6, 0x4(a0)
    617c: fb842583     	lw	a1, -0x48(s0)
    6180: 00b52423     	sw	a1, 0x8(a0)
    6184: f5042583     	lw	a1, -0xb0(s0)
    6188: 00b52623     	sw	a1, 0xc(a0)
    618c: 0bd52823     	sw	t4, 0xb0(a0)
    6190: f6042583     	lw	a1, -0xa0(s0)
    6194: 0ab52a23     	sw	a1, 0xb4(a0)
    6198: f6442583     	lw	a1, -0x9c(s0)
    619c: 0ab52c23     	sw	a1, 0xb8(a0)
    61a0: f6c42583     	lw	a1, -0x94(s0)
    61a4: 0ab52e23     	sw	a1, 0xbc(a0)
    61a8: fa042583     	lw	a1, -0x60(s0)
    61ac: 04b52023     	sw	a1, 0x40(a0)
    61b0: f8442583     	lw	a1, -0x7c(s0)
    61b4: 04b52223     	sw	a1, 0x44(a0)
    61b8: fac42583     	lw	a1, -0x54(s0)
    61bc: 04b52423     	sw	a1, 0x48(a0)
    61c0: fb042583     	lw	a1, -0x50(s0)
    61c4: 04b52623     	sw	a1, 0x4c(a0)
    61c8: f8042583     	lw	a1, -0x80(s0)
    61cc: 08b52823     	sw	a1, 0x90(a0)
    61d0: 09c52a23     	sw	t3, 0x94(a0)
    61d4: 09052c23     	sw	a6, 0x98(a0)
    61d8: f9442583     	lw	a1, -0x6c(s0)
    61dc: 08b52e23     	sw	a1, 0x9c(a0)
    61e0: 0cf52023     	sw	a5, 0xc0(a0)
    61e4: 0ce52223     	sw	a4, 0xc4(a0)
    61e8: 10c12083     	lw	ra, 0x10c(sp)
    61ec: 10812403     	lw	s0, 0x108(sp)
    61f0: 10412483     	lw	s1, 0x104(sp)
    61f4: 10012903     	lw	s2, 0x100(sp)
    61f8: 0fc12983     	lw	s3, 0xfc(sp)
    61fc: 0f812a03     	lw	s4, 0xf8(sp)
    6200: 0f412a83     	lw	s5, 0xf4(sp)
    6204: 0f012b03     	lw	s6, 0xf0(sp)
    6208: 0ec12b83     	lw	s7, 0xec(sp)
    620c: 0e812c03     	lw	s8, 0xe8(sp)
    6210: 0e412c83     	lw	s9, 0xe4(sp)
    6214: 0e012d03     	lw	s10, 0xe0(sp)
    6218: 0dc12d83     	lw	s11, 0xdc(sp)
    621c: 11010113     	addi	sp, sp, 0x110
    6220: 00008067     	ret
    6224: 04200537     	lui	a0, 0x4200
    6228: 2f850513     	addi	a0, a0, 0x2f8
    622c: 04200637     	lui	a2, 0x4200
    6230: 33c60613     	addi	a2, a2, 0x33c
    6234: 04100593     	li	a1, 0x41
    6238: 130010ef     	jal	0x7368 <core::panicking::panic::ha1ed58f4f5473d93>

0000623c <ark_std::rand_helper::test_rng::h09418e87dd50358f>:
    623c: fe010113     	addi	sp, sp, -0x20
    6240: 00112e23     	sw	ra, 0x1c(sp)
    6244: 00812c23     	sw	s0, 0x18(sp)
    6248: 00912a23     	sw	s1, 0x14(sp)
    624c: 01212823     	sw	s2, 0x10(sp)
    6250: 01312623     	sw	s3, 0xc(sp)
    6254: 02010413     	addi	s0, sp, 0x20
    6258: 00050493     	mv	s1, a0
    625c: 04200937     	lui	s2, 0x4200
    6260: 34c90913     	addi	s2, s2, 0x34c
    6264: 00400593     	li	a1, 0x4
    6268: 00090513     	mv	a0, s2
    626c: 090000ef     	jal	0x62fc <rand_chacha::guts::read_u32le::h37eeec1a25fc2dc1>
    6270: 00050993     	mv	s3, a0
    6274: 00490513     	addi	a0, s2, 0x4
    6278: 00400593     	li	a1, 0x4
    627c: 080000ef     	jal	0x62fc <rand_chacha::guts::read_u32le::h37eeec1a25fc2dc1>
    6280: 00050913     	mv	s2, a0
    6284: 10000613     	li	a2, 0x100
    6288: 00048513     	mv	a0, s1
    628c: 00000593     	li	a1, 0x0
    6290: 4f0010ef     	jal	0x7780 <memset>
    6294: 04000513     	li	a0, 0x40
    6298: 00100593     	li	a1, 0x1
    629c: 01700613     	li	a2, 0x17
    62a0: 1c800693     	li	a3, 0x1c8
    62a4: 1204a223     	sw	zero, 0x124(s1)
    62a8: 1204a423     	sw	zero, 0x128(s1)
    62ac: 1204a623     	sw	zero, 0x12c(s1)
    62b0: 1334a823     	sw	s3, 0x130(s1)
    62b4: 00002737     	lui	a4, 0x2
    62b8: 10a4a023     	sw	a0, 0x100(s1)
    62bc: 10b4a423     	sw	a1, 0x108(s1)
    62c0: 10c4a623     	sw	a2, 0x10c(s1)
    62c4: 10d4a823     	sw	a3, 0x110(s1)
    62c8: ed270513     	addi	a0, a4, -0x12e
    62cc: 10a4aa23     	sw	a0, 0x114(s1)
    62d0: 1004ac23     	sw	zero, 0x118(s1)
    62d4: 1004ae23     	sw	zero, 0x11c(s1)
    62d8: 1204a023     	sw	zero, 0x120(s1)
    62dc: 1324aa23     	sw	s2, 0x134(s1)
    62e0: 01c12083     	lw	ra, 0x1c(sp)
    62e4: 01812403     	lw	s0, 0x18(sp)
    62e8: 01412483     	lw	s1, 0x14(sp)
    62ec: 01012903     	lw	s2, 0x10(sp)
    62f0: 00c12983     	lw	s3, 0xc(sp)
    62f4: 02010113     	addi	sp, sp, 0x20
    62f8: 00008067     	ret

000062fc <rand_chacha::guts::read_u32le::h37eeec1a25fc2dc1>:
    62fc: fd010113     	addi	sp, sp, -0x30
    6300: 02112623     	sw	ra, 0x2c(sp)
    6304: 02812423     	sw	s0, 0x28(sp)
    6308: 03010413     	addi	s0, sp, 0x30
    630c: 00400613     	li	a2, 0x4
    6310: fcb42e23     	sw	a1, -0x24(s0)
    6314: 02c59e63     	bne	a1, a2, 0x6350 <rand_chacha::guts::read_u32le::h37eeec1a25fc2dc1+0x54>
    6318: 00154583     	lbu	a1, 0x1(a0)
    631c: 00054603     	lbu	a2, 0x0(a0)
    6320: 00254683     	lbu	a3, 0x2(a0)
    6324: 00354503     	lbu	a0, 0x3(a0)
    6328: 00859593     	slli	a1, a1, 0x8
    632c: 00c5e5b3     	or	a1, a1, a2
    6330: 01069693     	slli	a3, a3, 0x10
    6334: 01851513     	slli	a0, a0, 0x18
    6338: 00a6e533     	or	a0, a3, a0
    633c: 00a5e533     	or	a0, a1, a0
    6340: 02c12083     	lw	ra, 0x2c(sp)
    6344: 02812403     	lw	s0, 0x28(sp)
    6348: 03010113     	addi	sp, sp, 0x30
    634c: 00008067     	ret
    6350: fe042023     	sw	zero, -0x20(s0)
    6354: 04200637     	lui	a2, 0x4200
    6358: 35460613     	addi	a2, a2, 0x354
    635c: 04200737     	lui	a4, 0x4200
    6360: 35870713     	addi	a4, a4, 0x358
    6364: fdc40593     	addi	a1, s0, -0x24
    6368: fe040693     	addi	a3, s0, -0x20
    636c: 00000513     	li	a0, 0x0
    6370: 605000ef     	jal	0x7174 <core::panicking::assert_failed::hfc6c5954ad41c12a>

00006374 <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754>:
    6374: ff010113     	addi	sp, sp, -0x10
    6378: 00112623     	sw	ra, 0xc(sp)
    637c: 00812423     	sw	s0, 0x8(sp)
    6380: 01010413     	addi	s0, sp, 0x10
    6384: 00058813     	mv	a6, a1
    6388: 0085a583     	lw	a1, 0x8(a1)
    638c: 00052503     	lw	a0, 0x0(a0)
    6390: 00659613     	slli	a2, a1, 0x6
    6394: 02064263     	bltz	a2, 0x63b8 <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754+0x44>
    6398: 00559593     	slli	a1, a1, 0x5
    639c: 0405ca63     	bltz	a1, 0x63f0 <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754+0x7c>
    63a0: 00080593     	mv	a1, a6
    63a4: 00c12083     	lw	ra, 0xc(sp)
    63a8: 00812403     	lw	s0, 0x8(sp)
    63ac: 01010113     	addi	sp, sp, 0x10
    63b0: 00000317     	auipc	t1, 0x0
    63b4: 0d430067     	jr	0xd4(t1) <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d>
    63b8: 00000793     	li	a5, 0x0
    63bc: 00052503     	lw	a0, 0x0(a0)
    63c0: ff740593     	addi	a1, s0, -0x9
    63c4: 04200637     	lui	a2, 0x4200
    63c8: 46060613     	addi	a2, a2, 0x460
    63cc: 00f57693     	andi	a3, a0, 0xf
    63d0: 00d606b3     	add	a3, a2, a3
    63d4: 0006c683     	lbu	a3, 0x0(a3)
    63d8: 00455513     	srli	a0, a0, 0x4
    63dc: 00178793     	addi	a5, a5, 0x1
    63e0: 00d58023     	sb	a3, 0x0(a1)
    63e4: fff58593     	addi	a1, a1, -0x1
    63e8: fe0512e3     	bnez	a0, 0x63cc <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754+0x58>
    63ec: 0380006f     	j	0x6424 <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754+0xb0>
    63f0: 00000793     	li	a5, 0x0
    63f4: 00052503     	lw	a0, 0x0(a0)
    63f8: ff740593     	addi	a1, s0, -0x9
    63fc: 04200637     	lui	a2, 0x4200
    6400: 43060613     	addi	a2, a2, 0x430
    6404: 00f57693     	andi	a3, a0, 0xf
    6408: 00d606b3     	add	a3, a2, a3
    640c: 0006c683     	lbu	a3, 0x0(a3)
    6410: 00455513     	srli	a0, a0, 0x4
    6414: 00178793     	addi	a5, a5, 0x1
    6418: 00d58023     	sb	a3, 0x0(a1)
    641c: fff58593     	addi	a1, a1, -0x1
    6420: fe0512e3     	bnez	a0, 0x6404 <<&T as core::fmt::Debug>::fmt::h77827edaa5b1f754+0x90>
    6424: ff040513     	addi	a0, s0, -0x10
    6428: 40f50533     	sub	a0, a0, a5
    642c: 00850713     	addi	a4, a0, 0x8
    6430: 04200637     	lui	a2, 0x4200
    6434: 47060613     	addi	a2, a2, 0x470
    6438: 00100593     	li	a1, 0x1
    643c: 00200693     	li	a3, 0x2
    6440: 00080513     	mv	a0, a6
    6444: 00000097     	auipc	ra, 0x0
    6448: 534080e7     	jalr	0x534(ra) <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8>
    644c: 00c12083     	lw	ra, 0xc(sp)
    6450: 00812403     	lw	s0, 0x8(sp)
    6454: 01010113     	addi	sp, sp, 0x10
    6458: 00008067     	ret

0000645c <<&T as core::fmt::Debug>::fmt::h85c5cd2e4eb17cf7>:
    645c: 00452603     	lw	a2, 0x4(a0)
    6460: 00052503     	lw	a0, 0x0(a0)
    6464: 00c62303     	lw	t1, 0xc(a2)
    6468: 00030067     	jr	t1

0000646c <<&T as core::fmt::Display>::fmt::hb631d56c33c5b083>:
    646c: 00052683     	lw	a3, 0x0(a0)
    6470: 00452603     	lw	a2, 0x4(a0)
    6474: 00058513     	mv	a0, a1
    6478: 00068593     	mv	a1, a3
    647c: 00001317     	auipc	t1, 0x1
    6480: 86830067     	jr	-0x798(t1) <core::fmt::Formatter::pad::h44850e4be722b02c>

00006484 <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d>:
    6484: fc010113     	addi	sp, sp, -0x40
    6488: 02112e23     	sw	ra, 0x3c(sp)
    648c: 02812c23     	sw	s0, 0x38(sp)
    6490: 02912a23     	sw	s1, 0x34(sp)
    6494: 03212823     	sw	s2, 0x30(sp)
    6498: 03312623     	sw	s3, 0x2c(sp)
    649c: 03412423     	sw	s4, 0x28(sp)
    64a0: 03512223     	sw	s5, 0x24(sp)
    64a4: 03612023     	sw	s6, 0x20(sp)
    64a8: 01712e23     	sw	s7, 0x1c(sp)
    64ac: 01812c23     	sw	s8, 0x18(sp)
    64b0: 01912a23     	sw	s9, 0x14(sp)
    64b4: 01a12823     	sw	s10, 0x10(sp)
    64b8: 04010413     	addi	s0, sp, 0x40
    64bc: 00058493     	mv	s1, a1
    64c0: 00052b83     	lw	s7, 0x0(a0)
    64c4: 3e800513     	li	a0, 0x3e8
    64c8: 04200b37     	lui	s6, 0x4200
    64cc: 368b0b13     	addi	s6, s6, 0x368
    64d0: 00a00a93     	li	s5, 0xa
    64d4: 1aabe663     	bltu	s7, a0, 0x6680 <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x1fc>
    64d8: fcf40c13     	addi	s8, s0, -0x31
    64dc: 00002537     	lui	a0, 0x2
    64e0: 009895b7     	lui	a1, 0x989
    64e4: 71050993     	addi	s3, a0, 0x710
    64e8: 67f58c93     	addi	s9, a1, 0x67f
    64ec: 000b8913     	mv	s2, s7
    64f0: 00090a13     	mv	s4, s2
    64f4: ffca8a93     	addi	s5, s5, -0x4
    64f8: 00090513     	mv	a0, s2
    64fc: 00098593     	mv	a1, s3
    6500: 00001097     	auipc	ra, 0x1
    6504: 278080e7     	jalr	0x278(ra) <__udivsi3>
    6508: 00050913     	mv	s2, a0
    650c: 00851513     	slli	a0, a0, 0x8
    6510: 00491593     	slli	a1, s2, 0x4
    6514: 00b91613     	slli	a2, s2, 0xb
    6518: 40a585b3     	sub	a1, a1, a0
    651c: 00d91513     	slli	a0, s2, 0xd
    6520: 00a60533     	add	a0, a2, a0
    6524: 00a58533     	add	a0, a1, a0
    6528: 40aa0d33     	sub	s10, s4, a0
    652c: 010d1513     	slli	a0, s10, 0x10
    6530: 01055513     	srli	a0, a0, 0x10
    6534: 06400593     	li	a1, 0x64
    6538: 00001097     	auipc	ra, 0x1
    653c: 240080e7     	jalr	0x240(ra) <__udivsi3>
    6540: 00551593     	slli	a1, a0, 0x5
    6544: 00251613     	slli	a2, a0, 0x2
    6548: 40b60633     	sub	a2, a2, a1
    654c: 00751593     	slli	a1, a0, 0x7
    6550: 00151513     	slli	a0, a0, 0x1
    6554: 00ab0533     	add	a0, s6, a0
    6558: 00b605b3     	add	a1, a2, a1
    655c: 40bd05b3     	sub	a1, s10, a1
    6560: 01159593     	slli	a1, a1, 0x11
    6564: 0105d593     	srli	a1, a1, 0x10
    6568: 00bb05b3     	add	a1, s6, a1
    656c: 00054603     	lbu	a2, 0x0(a0)
    6570: 00154503     	lbu	a0, 0x1(a0)
    6574: 0005c683     	lbu	a3, 0x0(a1)
    6578: 0015c583     	lbu	a1, 0x1(a1)
    657c: fecc0ea3     	sb	a2, -0x3(s8)
    6580: feac0f23     	sb	a0, -0x2(s8)
    6584: fedc0fa3     	sb	a3, -0x1(s8)
    6588: 00bc0023     	sb	a1, 0x0(s8)
    658c: ffcc0c13     	addi	s8, s8, -0x4
    6590: f74ce0e3     	bltu	s9, s4, 0x64f0 <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x6c>
    6594: 00900513     	li	a0, 0x9
    6598: 07257263     	bgeu	a0, s2, 0x65fc <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x178>
    659c: ffea8993     	addi	s3, s5, -0x2
    65a0: 01091513     	slli	a0, s2, 0x10
    65a4: 01055513     	srli	a0, a0, 0x10
    65a8: 06400593     	li	a1, 0x64
    65ac: 00001097     	auipc	ra, 0x1
    65b0: 1cc080e7     	jalr	0x1cc(ra) <__udivsi3>
    65b4: 00551593     	slli	a1, a0, 0x5
    65b8: 00251613     	slli	a2, a0, 0x2
    65bc: 00751693     	slli	a3, a0, 0x7
    65c0: 40b60633     	sub	a2, a2, a1
    65c4: fc640593     	addi	a1, s0, -0x3a
    65c8: 40d906b3     	sub	a3, s2, a3
    65cc: 40c686b3     	sub	a3, a3, a2
    65d0: 01169693     	slli	a3, a3, 0x11
    65d4: 0106d693     	srli	a3, a3, 0x10
    65d8: 00db06b3     	add	a3, s6, a3
    65dc: 0006c603     	lbu	a2, 0x0(a3)
    65e0: 0016c683     	lbu	a3, 0x1(a3)
    65e4: 01358733     	add	a4, a1, s3
    65e8: 015585b3     	add	a1, a1, s5
    65ec: 00c70023     	sb	a2, 0x0(a4)
    65f0: fed58fa3     	sb	a3, -0x1(a1)
    65f4: 00050913     	mv	s2, a0
    65f8: 00098a93     	mv	s5, s3
    65fc: 000b8463     	beqz	s7, 0x6604 <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x180>
    6600: 02090063     	beqz	s2, 0x6620 <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x19c>
    6604: 00191913     	slli	s2, s2, 0x1
    6608: 012b0933     	add	s2, s6, s2
    660c: 00194503     	lbu	a0, 0x1(s2)
    6610: fffa8a93     	addi	s5, s5, -0x1
    6614: fc640593     	addi	a1, s0, -0x3a
    6618: 015585b3     	add	a1, a1, s5
    661c: 00a58023     	sb	a0, 0x0(a1)
    6620: 00a00513     	li	a0, 0xa
    6624: fc640713     	addi	a4, s0, -0x3a
    6628: 415507b3     	sub	a5, a0, s5
    662c: 01570733     	add	a4, a4, s5
    6630: 00100593     	li	a1, 0x1
    6634: 00100613     	li	a2, 0x1
    6638: 00048513     	mv	a0, s1
    663c: 00000693     	li	a3, 0x0
    6640: 00000097     	auipc	ra, 0x0
    6644: 338080e7     	jalr	0x338(ra) <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8>
    6648: 03c12083     	lw	ra, 0x3c(sp)
    664c: 03812403     	lw	s0, 0x38(sp)
    6650: 03412483     	lw	s1, 0x34(sp)
    6654: 03012903     	lw	s2, 0x30(sp)
    6658: 02c12983     	lw	s3, 0x2c(sp)
    665c: 02812a03     	lw	s4, 0x28(sp)
    6660: 02412a83     	lw	s5, 0x24(sp)
    6664: 02012b03     	lw	s6, 0x20(sp)
    6668: 01c12b83     	lw	s7, 0x1c(sp)
    666c: 01812c03     	lw	s8, 0x18(sp)
    6670: 01412c83     	lw	s9, 0x14(sp)
    6674: 01012d03     	lw	s10, 0x10(sp)
    6678: 04010113     	addi	sp, sp, 0x40
    667c: 00008067     	ret
    6680: 000b8913     	mv	s2, s7
    6684: 00900513     	li	a0, 0x9
    6688: f1756ae3     	bltu	a0, s7, 0x659c <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x118>
    668c: f71ff06f     	j	0x65fc <core::fmt::num::imp::<impl core::fmt::Display for usize>::fmt::hd3e729a9cd54254d+0x178>

00006690 <core::fmt::write::heccac97d5faae40b>:
    6690: fc010113     	addi	sp, sp, -0x40
    6694: 02112e23     	sw	ra, 0x3c(sp)
    6698: 02812c23     	sw	s0, 0x38(sp)
    669c: 02912a23     	sw	s1, 0x34(sp)
    66a0: 03212823     	sw	s2, 0x30(sp)
    66a4: 03312623     	sw	s3, 0x2c(sp)
    66a8: 03412423     	sw	s4, 0x28(sp)
    66ac: 03512223     	sw	s5, 0x24(sp)
    66b0: 03612023     	sw	s6, 0x20(sp)
    66b4: 01712e23     	sw	s7, 0x1c(sp)
    66b8: 01812c23     	sw	s8, 0x18(sp)
    66bc: 04010413     	addi	s0, sp, 0x40
    66c0: 00060493     	mv	s1, a2
    66c4: e0000637     	lui	a2, 0xe0000
    66c8: 0104aa03     	lw	s4, 0x10(s1)
    66cc: 02060613     	addi	a2, a2, 0x20
    66d0: fca42423     	sw	a0, -0x38(s0)
    66d4: fcb42623     	sw	a1, -0x34(s0)
    66d8: fcc42823     	sw	a2, -0x30(s0)
    66dc: fc042a23     	sw	zero, -0x2c(s0)
    66e0: 100a0463     	beqz	s4, 0x67e8 <core::fmt::write::heccac97d5faae40b+0x158>
    66e4: 0144a503     	lw	a0, 0x14(s1)
    66e8: 16050a63     	beqz	a0, 0x685c <core::fmt::write::heccac97d5faae40b+0x1cc>
    66ec: 00351593     	slli	a1, a0, 0x3
    66f0: 00551613     	slli	a2, a0, 0x5
    66f4: 0004ab83     	lw	s7, 0x0(s1)
    66f8: 0084a983     	lw	s3, 0x8(s1)
    66fc: fff50513     	addi	a0, a0, -0x1
    6700: 00aa0a13     	addi	s4, s4, 0xa
    6704: 00200a93     	li	s5, 0x2
    6708: 40b60b33     	sub	s6, a2, a1
    670c: 00351513     	slli	a0, a0, 0x3
    6710: 00355513     	srli	a0, a0, 0x3
    6714: 00150913     	addi	s2, a0, 0x1
    6718: 004b8b93     	addi	s7, s7, 0x4
    671c: 00100c13     	li	s8, 0x1
    6720: 000ba603     	lw	a2, 0x0(s7)
    6724: 00060e63     	beqz	a2, 0x6740 <core::fmt::write::heccac97d5faae40b+0xb0>
    6728: fcc42683     	lw	a3, -0x34(s0)
    672c: fc842503     	lw	a0, -0x38(s0)
    6730: ffcba583     	lw	a1, -0x4(s7)
    6734: 00c6a683     	lw	a3, 0xc(a3)
    6738: 000680e7     	jalr	a3
    673c: 14051a63     	bnez	a0, 0x6890 <core::fmt::write::heccac97d5faae40b+0x200>
    6740: ffea5503     	lhu	a0, -0x2(s4)
    6744: 02050c63     	beqz	a0, 0x677c <core::fmt::write::heccac97d5faae40b+0xec>
    6748: 05851463     	bne	a0, s8, 0x6790 <core::fmt::write::heccac97d5faae40b+0x100>
    674c: 002a2503     	lw	a0, 0x2(s4)
    6750: 00351513     	slli	a0, a0, 0x3
    6754: 00a98533     	add	a0, s3, a0
    6758: 00455583     	lhu	a1, 0x4(a0)
    675c: ff6a5503     	lhu	a0, -0xa(s4)
    6760: 03550463     	beq	a0, s5, 0x6788 <core::fmt::write::heccac97d5faae40b+0xf8>
    6764: 03851e63     	bne	a0, s8, 0x67a0 <core::fmt::write::heccac97d5faae40b+0x110>
    6768: ffaa2503     	lw	a0, -0x6(s4)
    676c: 00351513     	slli	a0, a0, 0x3
    6770: 00a98533     	add	a0, s3, a0
    6774: 00455603     	lhu	a2, 0x4(a0)
    6778: 02c0006f     	j	0x67a4 <core::fmt::write::heccac97d5faae40b+0x114>
    677c: 000a5583     	lhu	a1, 0x0(s4)
    6780: ff6a5503     	lhu	a0, -0xa(s4)
    6784: ff5510e3     	bne	a0, s5, 0x6764 <core::fmt::write::heccac97d5faae40b+0xd4>
    6788: 00000613     	li	a2, 0x0
    678c: 0180006f     	j	0x67a4 <core::fmt::write::heccac97d5faae40b+0x114>
    6790: 00000593     	li	a1, 0x0
    6794: ff6a5503     	lhu	a0, -0xa(s4)
    6798: fd5516e3     	bne	a0, s5, 0x6764 <core::fmt::write::heccac97d5faae40b+0xd4>
    679c: fedff06f     	j	0x6788 <core::fmt::write::heccac97d5faae40b+0xf8>
    67a0: ff8a5603     	lhu	a2, -0x8(s4)
    67a4: 006a2503     	lw	a0, 0x6(s4)
    67a8: 00aa2683     	lw	a3, 0xa(s4)
    67ac: 00351513     	slli	a0, a0, 0x3
    67b0: 00a98733     	add	a4, s3, a0
    67b4: 00072503     	lw	a0, 0x0(a4)
    67b8: 00472703     	lw	a4, 0x4(a4)
    67bc: fcd42823     	sw	a3, -0x30(s0)
    67c0: fcb41a23     	sh	a1, -0x2c(s0)
    67c4: fcc41b23     	sh	a2, -0x2a(s0)
    67c8: fc840593     	addi	a1, s0, -0x38
    67cc: 000700e7     	jalr	a4
    67d0: 0c051063     	bnez	a0, 0x6890 <core::fmt::write::heccac97d5faae40b+0x200>
    67d4: 008b8b93     	addi	s7, s7, 0x8
    67d8: fe8b0b13     	addi	s6, s6, -0x18
    67dc: 018a0a13     	addi	s4, s4, 0x18
    67e0: f40b10e3     	bnez	s6, 0x6720 <core::fmt::write::heccac97d5faae40b+0x90>
    67e4: 06c0006f     	j	0x6850 <core::fmt::write::heccac97d5faae40b+0x1c0>
    67e8: 00c4a503     	lw	a0, 0xc(s1)
    67ec: 06050863     	beqz	a0, 0x685c <core::fmt::write::heccac97d5faae40b+0x1cc>
    67f0: 0004aa83     	lw	s5, 0x0(s1)
    67f4: 0084a983     	lw	s3, 0x8(s1)
    67f8: 00351513     	slli	a0, a0, 0x3
    67fc: ff850593     	addi	a1, a0, -0x8
    6800: 0035d593     	srli	a1, a1, 0x3
    6804: 00158913     	addi	s2, a1, 0x1
    6808: 00a98a33     	add	s4, s3, a0
    680c: 004a8a93     	addi	s5, s5, 0x4
    6810: 000aa603     	lw	a2, 0x0(s5)
    6814: 00060e63     	beqz	a2, 0x6830 <core::fmt::write::heccac97d5faae40b+0x1a0>
    6818: fcc42683     	lw	a3, -0x34(s0)
    681c: fc842503     	lw	a0, -0x38(s0)
    6820: ffcaa583     	lw	a1, -0x4(s5)
    6824: 00c6a683     	lw	a3, 0xc(a3)
    6828: 000680e7     	jalr	a3
    682c: 06051263     	bnez	a0, 0x6890 <core::fmt::write::heccac97d5faae40b+0x200>
    6830: 0009a503     	lw	a0, 0x0(s3)
    6834: 0049a603     	lw	a2, 0x4(s3)
    6838: fc840593     	addi	a1, s0, -0x38
    683c: 000600e7     	jalr	a2
    6840: 04051863     	bnez	a0, 0x6890 <core::fmt::write::heccac97d5faae40b+0x200>
    6844: 00898993     	addi	s3, s3, 0x8
    6848: 008a8a93     	addi	s5, s5, 0x8
    684c: fd4992e3     	bne	s3, s4, 0x6810 <core::fmt::write::heccac97d5faae40b+0x180>
    6850: 0044a503     	lw	a0, 0x4(s1)
    6854: 00a96a63     	bltu	s2, a0, 0x6868 <core::fmt::write::heccac97d5faae40b+0x1d8>
    6858: 0400006f     	j	0x6898 <core::fmt::write::heccac97d5faae40b+0x208>
    685c: 00000913     	li	s2, 0x0
    6860: 0044a503     	lw	a0, 0x4(s1)
    6864: 02050a63     	beqz	a0, 0x6898 <core::fmt::write::heccac97d5faae40b+0x208>
    6868: 0004a583     	lw	a1, 0x0(s1)
    686c: 00391913     	slli	s2, s2, 0x3
    6870: fc842503     	lw	a0, -0x38(s0)
    6874: fcc42683     	lw	a3, -0x34(s0)
    6878: 01258933     	add	s2, a1, s2
    687c: 00092583     	lw	a1, 0x0(s2)
    6880: 00492603     	lw	a2, 0x4(s2)
    6884: 00c6a683     	lw	a3, 0xc(a3)
    6888: 000680e7     	jalr	a3
    688c: 00050663     	beqz	a0, 0x6898 <core::fmt::write::heccac97d5faae40b+0x208>
    6890: 00100513     	li	a0, 0x1
    6894: 0080006f     	j	0x689c <core::fmt::write::heccac97d5faae40b+0x20c>
    6898: 00000513     	li	a0, 0x0
    689c: 03c12083     	lw	ra, 0x3c(sp)
    68a0: 03812403     	lw	s0, 0x38(sp)
    68a4: 03412483     	lw	s1, 0x34(sp)
    68a8: 03012903     	lw	s2, 0x30(sp)
    68ac: 02c12983     	lw	s3, 0x2c(sp)
    68b0: 02812a03     	lw	s4, 0x28(sp)
    68b4: 02412a83     	lw	s5, 0x24(sp)
    68b8: 02012b03     	lw	s6, 0x20(sp)
    68bc: 01c12b83     	lw	s7, 0x1c(sp)
    68c0: 01812c03     	lw	s8, 0x18(sp)
    68c4: 04010113     	addi	sp, sp, 0x40
    68c8: 00008067     	ret

000068cc <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6>:
    68cc: fe010113     	addi	sp, sp, -0x20
    68d0: 00112e23     	sw	ra, 0x1c(sp)
    68d4: 00812c23     	sw	s0, 0x18(sp)
    68d8: 00912a23     	sw	s1, 0x14(sp)
    68dc: 01212823     	sw	s2, 0x10(sp)
    68e0: 01312623     	sw	s3, 0xc(sp)
    68e4: 01412423     	sw	s4, 0x8(sp)
    68e8: 02010413     	addi	s0, sp, 0x20
    68ec: 00070493     	mv	s1, a4
    68f0: 00068913     	mv	s2, a3
    68f4: 00058993     	mv	s3, a1
    68f8: 001105b7     	lui	a1, 0x110
    68fc: 02b60463     	beq	a2, a1, 0x6924 <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6+0x58>
    6900: 0109a683     	lw	a3, 0x10(s3)
    6904: 00050a13     	mv	s4, a0
    6908: 00060593     	mv	a1, a2
    690c: 000680e7     	jalr	a3
    6910: 00050593     	mv	a1, a0
    6914: 000a0513     	mv	a0, s4
    6918: 00058663     	beqz	a1, 0x6924 <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6+0x58>
    691c: 00100513     	li	a0, 0x1
    6920: 0380006f     	j	0x6958 <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6+0x8c>
    6924: 02090863     	beqz	s2, 0x6954 <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6+0x88>
    6928: 00c9a303     	lw	t1, 0xc(s3)
    692c: 00090593     	mv	a1, s2
    6930: 00048613     	mv	a2, s1
    6934: 01c12083     	lw	ra, 0x1c(sp)
    6938: 01812403     	lw	s0, 0x18(sp)
    693c: 01412483     	lw	s1, 0x14(sp)
    6940: 01012903     	lw	s2, 0x10(sp)
    6944: 00c12983     	lw	s3, 0xc(sp)
    6948: 00812a03     	lw	s4, 0x8(sp)
    694c: 02010113     	addi	sp, sp, 0x20
    6950: 00030067     	jr	t1
    6954: 00000513     	li	a0, 0x0
    6958: 01c12083     	lw	ra, 0x1c(sp)
    695c: 01812403     	lw	s0, 0x18(sp)
    6960: 01412483     	lw	s1, 0x14(sp)
    6964: 01012903     	lw	s2, 0x10(sp)
    6968: 00c12983     	lw	s3, 0xc(sp)
    696c: 00812a03     	lw	s4, 0x8(sp)
    6970: 02010113     	addi	sp, sp, 0x20
    6974: 00008067     	ret

00006978 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8>:
    6978: fc010113     	addi	sp, sp, -0x40
    697c: 02112e23     	sw	ra, 0x3c(sp)
    6980: 02812c23     	sw	s0, 0x38(sp)
    6984: 02912a23     	sw	s1, 0x34(sp)
    6988: 03212823     	sw	s2, 0x30(sp)
    698c: 03312623     	sw	s3, 0x2c(sp)
    6990: 03412423     	sw	s4, 0x28(sp)
    6994: 03512223     	sw	s5, 0x24(sp)
    6998: 03612023     	sw	s6, 0x20(sp)
    699c: 01712e23     	sw	s7, 0x1c(sp)
    69a0: 01812c23     	sw	s8, 0x18(sp)
    69a4: 01912a23     	sw	s9, 0x14(sp)
    69a8: 01a12823     	sw	s10, 0x10(sp)
    69ac: 01b12623     	sw	s11, 0xc(sp)
    69b0: 04010413     	addi	s0, sp, 0x40
    69b4: 00078493     	mv	s1, a5
    69b8: 00070913     	mv	s2, a4
    69bc: 00068a13     	mv	s4, a3
    69c0: 00060a93     	mv	s5, a2
    69c4: 00050993     	mv	s3, a0
    69c8: 08058c63     	beqz	a1, 0x6a60 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0xe8>
    69cc: 0089ab83     	lw	s7, 0x8(s3)
    69d0: 00200537     	lui	a0, 0x200
    69d4: 00abf533     	and	a0, s7, a0
    69d8: 00110b37     	lui	s6, 0x110
    69dc: 00050463     	beqz	a0, 0x69e4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x6c>
    69e0: 02b00b13     	li	s6, 0x2b
    69e4: 01555513     	srli	a0, a0, 0x15
    69e8: 00950cb3     	add	s9, a0, s1
    69ec: 008b9513     	slli	a0, s7, 0x8
    69f0: 08055263     	bgez	a0, 0x6a74 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0xfc>
    69f4: 01000513     	li	a0, 0x10
    69f8: 0eaa7e63     	bgeu	s4, a0, 0x6af4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x17c>
    69fc: 00000513     	li	a0, 0x0
    6a00: 020a0263     	beqz	s4, 0x6a24 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0xac>
    6a04: 014a85b3     	add	a1, s5, s4
    6a08: 000a8613     	mv	a2, s5
    6a0c: 00060683     	lb	a3, 0x0(a2)
    6a10: 00160613     	addi	a2, a2, 0x1
    6a14: fc06a693     	slti	a3, a3, -0x40
    6a18: 0016c693     	xori	a3, a3, 0x1
    6a1c: 00d50533     	add	a0, a0, a3
    6a20: feb616e3     	bne	a2, a1, 0x6a0c <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x94>
    6a24: 01950cb3     	add	s9, a0, s9
    6a28: 00c9dd83     	lhu	s11, 0xc(s3)
    6a2c: 05bcfa63     	bgeu	s9, s11, 0x6a80 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x108>
    6a30: 007b9513     	slli	a0, s7, 0x7
    6a34: 0e054063     	bltz	a0, 0x6b14 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x19c>
    6a38: 419d8633     	sub	a2, s11, s9
    6a3c: 001b9513     	slli	a0, s7, 0x1
    6a40: 01e55513     	srli	a0, a0, 0x1e
    6a44: 00100593     	li	a1, 0x1
    6a48: 00bb9b93     	slli	s7, s7, 0xb
    6a4c: fd242423     	sw	s2, -0x38(s0)
    6a50: 14a5c663     	blt	a1, a0, 0x6b9c <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x224>
    6a54: 18051663     	bnez	a0, 0x6be0 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x268>
    6a58: 00000d13     	li	s10, 0x0
    6a5c: 1880006f     	j	0x6be4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x26c>
    6a60: 0089ab83     	lw	s7, 0x8(s3)
    6a64: 00148c93     	addi	s9, s1, 0x1
    6a68: 02d00b13     	li	s6, 0x2d
    6a6c: 008b9513     	slli	a0, s7, 0x8
    6a70: f80542e3     	bltz	a0, 0x69f4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x7c>
    6a74: 00000a93     	li	s5, 0x0
    6a78: 00c9dd83     	lhu	s11, 0xc(s3)
    6a7c: fbbceae3     	bltu	s9, s11, 0x6a30 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0xb8>
    6a80: 0009ab83     	lw	s7, 0x0(s3)
    6a84: 0049a983     	lw	s3, 0x4(s3)
    6a88: 000b8513     	mv	a0, s7
    6a8c: 00098593     	mv	a1, s3
    6a90: 000b0613     	mv	a2, s6
    6a94: 000a8693     	mv	a3, s5
    6a98: 000a0713     	mv	a4, s4
    6a9c: 00000097     	auipc	ra, 0x0
    6aa0: e30080e7     	jalr	-0x1d0(ra) <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6>
    6aa4: 18051063     	bnez	a0, 0x6c24 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2ac>
    6aa8: 00c9a303     	lw	t1, 0xc(s3)
    6aac: 000b8513     	mv	a0, s7
    6ab0: 00090593     	mv	a1, s2
    6ab4: 00048613     	mv	a2, s1
    6ab8: 03c12083     	lw	ra, 0x3c(sp)
    6abc: 03812403     	lw	s0, 0x38(sp)
    6ac0: 03412483     	lw	s1, 0x34(sp)
    6ac4: 03012903     	lw	s2, 0x30(sp)
    6ac8: 02c12983     	lw	s3, 0x2c(sp)
    6acc: 02812a03     	lw	s4, 0x28(sp)
    6ad0: 02412a83     	lw	s5, 0x24(sp)
    6ad4: 02012b03     	lw	s6, 0x20(sp)
    6ad8: 01c12b83     	lw	s7, 0x1c(sp)
    6adc: 01812c03     	lw	s8, 0x18(sp)
    6ae0: 01412c83     	lw	s9, 0x14(sp)
    6ae4: 01012d03     	lw	s10, 0x10(sp)
    6ae8: 00c12d83     	lw	s11, 0xc(sp)
    6aec: 04010113     	addi	sp, sp, 0x40
    6af0: 00030067     	jr	t1
    6af4: 000a8513     	mv	a0, s5
    6af8: 000a0593     	mv	a1, s4
    6afc: 00000097     	auipc	ra, 0x0
    6b00: 480080e7     	jalr	0x480(ra) <core::str::count::do_count_chars::h7f3e012b2bb2993f>
    6b04: 01950cb3     	add	s9, a0, s9
    6b08: 00c9dd83     	lhu	s11, 0xc(s3)
    6b0c: f7bcfae3     	bgeu	s9, s11, 0x6a80 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x108>
    6b10: f21ff06f     	j	0x6a30 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0xb8>
    6b14: 0009ab83     	lw	s7, 0x0(s3)
    6b18: 0049ac03     	lw	s8, 0x4(s3)
    6b1c: 0089ad03     	lw	s10, 0x8(s3)
    6b20: 00c9a503     	lw	a0, 0xc(s3)
    6b24: fca42223     	sw	a0, -0x3c(s0)
    6b28: 9fe00537     	lui	a0, 0x9fe00
    6b2c: 200005b7     	lui	a1, 0x20000
    6b30: 00ad7533     	and	a0, s10, a0
    6b34: 03058593     	addi	a1, a1, 0x30
    6b38: 00b56533     	or	a0, a0, a1
    6b3c: 00a9a423     	sw	a0, 0x8(s3)
    6b40: 000b8513     	mv	a0, s7
    6b44: 000c0593     	mv	a1, s8
    6b48: 000b0613     	mv	a2, s6
    6b4c: 000a8693     	mv	a3, s5
    6b50: 000a0713     	mv	a4, s4
    6b54: 00000097     	auipc	ra, 0x0
    6b58: d78080e7     	jalr	-0x288(ra) <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6>
    6b5c: 00100a13     	li	s4, 0x1
    6b60: 0c051463     	bnez	a0, 0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6b64: 00000a93     	li	s5, 0x0
    6b68: 419d8533     	sub	a0, s11, s9
    6b6c: 00010b37     	lui	s6, 0x10
    6b70: fffb0b13     	addi	s6, s6, -0x1
    6b74: 01657cb3     	and	s9, a0, s6
    6b78: 016af533     	and	a0, s5, s6
    6b7c: 03957c63     	bgeu	a0, s9, 0x6bb4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x23c>
    6b80: 010c2603     	lw	a2, 0x10(s8)
    6b84: 001a8a93     	addi	s5, s5, 0x1
    6b88: 03000593     	li	a1, 0x30
    6b8c: 000b8513     	mv	a0, s7
    6b90: 000600e7     	jalr	a2
    6b94: fe0502e3     	beqz	a0, 0x6b78 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x200>
    6b98: 0900006f     	j	0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6b9c: 00200593     	li	a1, 0x2
    6ba0: 00060d13     	mv	s10, a2
    6ba4: 04b51063     	bne	a0, a1, 0x6be4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x26c>
    6ba8: 01061513     	slli	a0, a2, 0x10
    6bac: 01155d13     	srli	s10, a0, 0x11
    6bb0: 0340006f     	j	0x6be4 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x26c>
    6bb4: 00cc2683     	lw	a3, 0xc(s8)
    6bb8: 000b8513     	mv	a0, s7
    6bbc: 00090593     	mv	a1, s2
    6bc0: 00048613     	mv	a2, s1
    6bc4: 000680e7     	jalr	a3
    6bc8: 06051063     	bnez	a0, 0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6bcc: 00000a13     	li	s4, 0x0
    6bd0: 01a9a423     	sw	s10, 0x8(s3)
    6bd4: fc442503     	lw	a0, -0x3c(s0)
    6bd8: 00a9a623     	sw	a0, 0xc(s3)
    6bdc: 04c0006f     	j	0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6be0: 00060d13     	mv	s10, a2
    6be4: fcc42223     	sw	a2, -0x3c(s0)
    6be8: 00000d93     	li	s11, 0x0
    6bec: 00bbdb93     	srli	s7, s7, 0xb
    6bf0: 0009ac03     	lw	s8, 0x0(s3)
    6bf4: 0049a983     	lw	s3, 0x4(s3)
    6bf8: 00010937     	lui	s2, 0x10
    6bfc: fff90913     	addi	s2, s2, -0x1
    6c00: 012d7cb3     	and	s9, s10, s2
    6c04: 012df533     	and	a0, s11, s2
    6c08: 07957063     	bgeu	a0, s9, 0x6c68 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2f0>
    6c0c: 0109a603     	lw	a2, 0x10(s3)
    6c10: 001d8d93     	addi	s11, s11, 0x1
    6c14: 000c0513     	mv	a0, s8
    6c18: 000b8593     	mv	a1, s7
    6c1c: 000600e7     	jalr	a2
    6c20: fe0502e3     	beqz	a0, 0x6c04 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x28c>
    6c24: 00100a13     	li	s4, 0x1
    6c28: 000a0513     	mv	a0, s4
    6c2c: 03c12083     	lw	ra, 0x3c(sp)
    6c30: 03812403     	lw	s0, 0x38(sp)
    6c34: 03412483     	lw	s1, 0x34(sp)
    6c38: 03012903     	lw	s2, 0x30(sp)
    6c3c: 02c12983     	lw	s3, 0x2c(sp)
    6c40: 02812a03     	lw	s4, 0x28(sp)
    6c44: 02412a83     	lw	s5, 0x24(sp)
    6c48: 02012b03     	lw	s6, 0x20(sp)
    6c4c: 01c12b83     	lw	s7, 0x1c(sp)
    6c50: 01812c03     	lw	s8, 0x18(sp)
    6c54: 01412c83     	lw	s9, 0x14(sp)
    6c58: 01012d03     	lw	s10, 0x10(sp)
    6c5c: 00c12d83     	lw	s11, 0xc(sp)
    6c60: 04010113     	addi	sp, sp, 0x40
    6c64: 00008067     	ret
    6c68: 000c0513     	mv	a0, s8
    6c6c: 00098593     	mv	a1, s3
    6c70: 000b0613     	mv	a2, s6
    6c74: 000a8693     	mv	a3, s5
    6c78: 000a0713     	mv	a4, s4
    6c7c: 00000097     	auipc	ra, 0x0
    6c80: c50080e7     	jalr	-0x3b0(ra) <core::fmt::Formatter::pad_integral::write_prefix::hab84cde72d48c8e6>
    6c84: 00100a13     	li	s4, 0x1
    6c88: fa0510e3     	bnez	a0, 0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6c8c: 00c9a683     	lw	a3, 0xc(s3)
    6c90: 000c0513     	mv	a0, s8
    6c94: fc842583     	lw	a1, -0x38(s0)
    6c98: 00048613     	mv	a2, s1
    6c9c: 000680e7     	jalr	a3
    6ca0: f80514e3     	bnez	a0, 0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6ca4: 00000493     	li	s1, 0x0
    6ca8: fc442503     	lw	a0, -0x3c(s0)
    6cac: 41a50533     	sub	a0, a0, s10
    6cb0: 00010937     	lui	s2, 0x10
    6cb4: fff90913     	addi	s2, s2, -0x1
    6cb8: 01257ab3     	and	s5, a0, s2
    6cbc: 0124f533     	and	a0, s1, s2
    6cc0: 01553a33     	sltu	s4, a0, s5
    6cc4: f75572e3     	bgeu	a0, s5, 0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>
    6cc8: 0109a603     	lw	a2, 0x10(s3)
    6ccc: 00148493     	addi	s1, s1, 0x1
    6cd0: 000c0513     	mv	a0, s8
    6cd4: 000b8593     	mv	a1, s7
    6cd8: 000600e7     	jalr	a2
    6cdc: fe0500e3     	beqz	a0, 0x6cbc <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x344>
    6ce0: f49ff06f     	j	0x6c28 <core::fmt::Formatter::pad_integral::hd7a5e3934241cae8+0x2b0>

00006ce4 <core::fmt::Formatter::pad::h44850e4be722b02c>:
    6ce4: fd010113     	addi	sp, sp, -0x30
    6ce8: 02112623     	sw	ra, 0x2c(sp)
    6cec: 02812423     	sw	s0, 0x28(sp)
    6cf0: 02912223     	sw	s1, 0x24(sp)
    6cf4: 03212023     	sw	s2, 0x20(sp)
    6cf8: 01312e23     	sw	s3, 0x1c(sp)
    6cfc: 01412c23     	sw	s4, 0x18(sp)
    6d00: 01512a23     	sw	s5, 0x14(sp)
    6d04: 01612823     	sw	s6, 0x10(sp)
    6d08: 01712623     	sw	s7, 0xc(sp)
    6d0c: 01812423     	sw	s8, 0x8(sp)
    6d10: 01912223     	sw	s9, 0x4(sp)
    6d14: 01a12023     	sw	s10, 0x0(sp)
    6d18: 03010413     	addi	s0, sp, 0x30
    6d1c: 00060913     	mv	s2, a2
    6d20: 00852983     	lw	s3, 0x8(a0)
    6d24: 18000637     	lui	a2, 0x18000
    6d28: 00c9f633     	and	a2, s3, a2
    6d2c: 00058493     	mv	s1, a1
    6d30: 0e060263     	beqz	a2, 0x6e14 <core::fmt::Formatter::pad::h44850e4be722b02c+0x130>
    6d34: 00399593     	slli	a1, s3, 0x3
    6d38: 0605c263     	bltz	a1, 0x6d9c <core::fmt::Formatter::pad::h44850e4be722b02c+0xb8>
    6d3c: 01000593     	li	a1, 0x10
    6d40: 12b97063     	bgeu	s2, a1, 0x6e60 <core::fmt::Formatter::pad::h44850e4be722b02c+0x17c>
    6d44: 00000593     	li	a1, 0x0
    6d48: 02090263     	beqz	s2, 0x6d6c <core::fmt::Formatter::pad::h44850e4be722b02c+0x88>
    6d4c: 01248633     	add	a2, s1, s2
    6d50: 00048693     	mv	a3, s1
    6d54: 00068703     	lb	a4, 0x0(a3)
    6d58: 00168693     	addi	a3, a3, 0x1
    6d5c: fc072713     	slti	a4, a4, -0x40
    6d60: 00174713     	xori	a4, a4, 0x1
    6d64: 00e585b3     	add	a1, a1, a4
    6d68: fec696e3     	bne	a3, a2, 0x6d54 <core::fmt::Formatter::pad::h44850e4be722b02c+0x70>
    6d6c: 00c55603     	lhu	a2, 0xc(a0)
    6d70: 0ac5f263     	bgeu	a1, a2, 0x6e14 <core::fmt::Formatter::pad::h44850e4be722b02c+0x130>
    6d74: 00000b13     	li	s6, 0x0
    6d78: 40b60ab3     	sub	s5, a2, a1
    6d7c: 00199593     	slli	a1, s3, 0x1
    6d80: 01e5d593     	srli	a1, a1, 0x1e
    6d84: 00100613     	li	a2, 0x1
    6d88: 00b99993     	slli	s3, s3, 0xb
    6d8c: 0eb64e63     	blt	a2, a1, 0x6e88 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1a4>
    6d90: 10058463     	beqz	a1, 0x6e98 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1b4>
    6d94: 000a8b13     	mv	s6, s5
    6d98: 1000006f     	j	0x6e98 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1b4>
    6d9c: 00e55583     	lhu	a1, 0xe(a0)
    6da0: 18058863     	beqz	a1, 0x6f30 <core::fmt::Formatter::pad::h44850e4be722b02c+0x24c>
    6da4: 012486b3     	add	a3, s1, s2
    6da8: 0e000713     	li	a4, 0xe0
    6dac: 0f000793     	li	a5, 0xf0
    6db0: 00048813     	mv	a6, s1
    6db4: 00058613     	mv	a2, a1
    6db8: 00000913     	li	s2, 0x0
    6dbc: 01c0006f     	j	0x6dd8 <core::fmt::Formatter::pad::h44850e4be722b02c+0xf4>
    6dc0: 00180893     	addi	a7, a6, 0x1
    6dc4: 41280833     	sub	a6, a6, s2
    6dc8: fff60613     	addi	a2, a2, -0x1
    6dcc: 41088933     	sub	s2, a7, a6
    6dd0: 00088813     	mv	a6, a7
    6dd4: 02060a63     	beqz	a2, 0x6e08 <core::fmt::Formatter::pad::h44850e4be722b02c+0x124>
    6dd8: 02d80863     	beq	a6, a3, 0x6e08 <core::fmt::Formatter::pad::h44850e4be722b02c+0x124>
    6ddc: 00080883     	lb	a7, 0x0(a6)
    6de0: fe08d0e3     	bgez	a7, 0x6dc0 <core::fmt::Formatter::pad::h44850e4be722b02c+0xdc>
    6de4: 0ff8f893     	zext.b	a7, a7
    6de8: 00e8e863     	bltu	a7, a4, 0x6df8 <core::fmt::Formatter::pad::h44850e4be722b02c+0x114>
    6dec: 00f8ea63     	bltu	a7, a5, 0x6e00 <core::fmt::Formatter::pad::h44850e4be722b02c+0x11c>
    6df0: 00480893     	addi	a7, a6, 0x4
    6df4: fd1ff06f     	j	0x6dc4 <core::fmt::Formatter::pad::h44850e4be722b02c+0xe0>
    6df8: 00280893     	addi	a7, a6, 0x2
    6dfc: fc9ff06f     	j	0x6dc4 <core::fmt::Formatter::pad::h44850e4be722b02c+0xe0>
    6e00: 00380893     	addi	a7, a6, 0x3
    6e04: fc1ff06f     	j	0x6dc4 <core::fmt::Formatter::pad::h44850e4be722b02c+0xe0>
    6e08: 40c585b3     	sub	a1, a1, a2
    6e0c: 00c55603     	lhu	a2, 0xc(a0)
    6e10: f6c5e2e3     	bltu	a1, a2, 0x6d74 <core::fmt::Formatter::pad::h44850e4be722b02c+0x90>
    6e14: 00452583     	lw	a1, 0x4(a0)
    6e18: 00052503     	lw	a0, 0x0(a0)
    6e1c: 00c5a303     	lw	t1, 0xc(a1)
    6e20: 00048593     	mv	a1, s1
    6e24: 00090613     	mv	a2, s2
    6e28: 02c12083     	lw	ra, 0x2c(sp)
    6e2c: 02812403     	lw	s0, 0x28(sp)
    6e30: 02412483     	lw	s1, 0x24(sp)
    6e34: 02012903     	lw	s2, 0x20(sp)
    6e38: 01c12983     	lw	s3, 0x1c(sp)
    6e3c: 01812a03     	lw	s4, 0x18(sp)
    6e40: 01412a83     	lw	s5, 0x14(sp)
    6e44: 01012b03     	lw	s6, 0x10(sp)
    6e48: 00c12b83     	lw	s7, 0xc(sp)
    6e4c: 00812c03     	lw	s8, 0x8(sp)
    6e50: 00412c83     	lw	s9, 0x4(sp)
    6e54: 00012d03     	lw	s10, 0x0(sp)
    6e58: 03010113     	addi	sp, sp, 0x30
    6e5c: 00030067     	jr	t1
    6e60: 00050a13     	mv	s4, a0
    6e64: 00048513     	mv	a0, s1
    6e68: 00090593     	mv	a1, s2
    6e6c: 00000097     	auipc	ra, 0x0
    6e70: 110080e7     	jalr	0x110(ra) <core::str::count::do_count_chars::h7f3e012b2bb2993f>
    6e74: 00050593     	mv	a1, a0
    6e78: 000a0513     	mv	a0, s4
    6e7c: 00ca5603     	lhu	a2, 0xc(s4)
    6e80: f8c5fae3     	bgeu	a1, a2, 0x6e14 <core::fmt::Formatter::pad::h44850e4be722b02c+0x130>
    6e84: ef1ff06f     	j	0x6d74 <core::fmt::Formatter::pad::h44850e4be722b02c+0x90>
    6e88: 00200613     	li	a2, 0x2
    6e8c: 00c59663     	bne	a1, a2, 0x6e98 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1b4>
    6e90: 010a9593     	slli	a1, s5, 0x10
    6e94: 0115db13     	srli	s6, a1, 0x11
    6e98: 00000c13     	li	s8, 0x0
    6e9c: 00b9d993     	srli	s3, s3, 0xb
    6ea0: 00052a03     	lw	s4, 0x0(a0)
    6ea4: 00452b83     	lw	s7, 0x4(a0)
    6ea8: 00010cb7     	lui	s9, 0x10
    6eac: fffc8c93     	addi	s9, s9, -0x1
    6eb0: 019b7d33     	and	s10, s6, s9
    6eb4: 019c7533     	and	a0, s8, s9
    6eb8: 03a57063     	bgeu	a0, s10, 0x6ed8 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1f4>
    6ebc: 010ba603     	lw	a2, 0x10(s7)
    6ec0: 001c0c13     	addi	s8, s8, 0x1
    6ec4: 000a0513     	mv	a0, s4
    6ec8: 00098593     	mv	a1, s3
    6ecc: 000600e7     	jalr	a2
    6ed0: fe0502e3     	beqz	a0, 0x6eb4 <core::fmt::Formatter::pad::h44850e4be722b02c+0x1d0>
    6ed4: 01c0006f     	j	0x6ef0 <core::fmt::Formatter::pad::h44850e4be722b02c+0x20c>
    6ed8: 00cba683     	lw	a3, 0xc(s7)
    6edc: 000a0513     	mv	a0, s4
    6ee0: 00048593     	mv	a1, s1
    6ee4: 00090613     	mv	a2, s2
    6ee8: 000680e7     	jalr	a3
    6eec: 04050a63     	beqz	a0, 0x6f40 <core::fmt::Formatter::pad::h44850e4be722b02c+0x25c>
    6ef0: 00100493     	li	s1, 0x1
    6ef4: 00048513     	mv	a0, s1
    6ef8: 02c12083     	lw	ra, 0x2c(sp)
    6efc: 02812403     	lw	s0, 0x28(sp)
    6f00: 02412483     	lw	s1, 0x24(sp)
    6f04: 02012903     	lw	s2, 0x20(sp)
    6f08: 01c12983     	lw	s3, 0x1c(sp)
    6f0c: 01812a03     	lw	s4, 0x18(sp)
    6f10: 01412a83     	lw	s5, 0x14(sp)
    6f14: 01012b03     	lw	s6, 0x10(sp)
    6f18: 00c12b83     	lw	s7, 0xc(sp)
    6f1c: 00812c03     	lw	s8, 0x8(sp)
    6f20: 00412c83     	lw	s9, 0x4(sp)
    6f24: 00012d03     	lw	s10, 0x0(sp)
    6f28: 03010113     	addi	sp, sp, 0x30
    6f2c: 00008067     	ret
    6f30: 00000913     	li	s2, 0x0
    6f34: 00c55603     	lhu	a2, 0xc(a0)
    6f38: e2c5eee3     	bltu	a1, a2, 0x6d74 <core::fmt::Formatter::pad::h44850e4be722b02c+0x90>
    6f3c: ed9ff06f     	j	0x6e14 <core::fmt::Formatter::pad::h44850e4be722b02c+0x130>
    6f40: 00000913     	li	s2, 0x0
    6f44: 416a8533     	sub	a0, s5, s6
    6f48: 00010ab7     	lui	s5, 0x10
    6f4c: fffa8a93     	addi	s5, s5, -0x1
    6f50: 01557b33     	and	s6, a0, s5
    6f54: 01597533     	and	a0, s2, s5
    6f58: 016534b3     	sltu	s1, a0, s6
    6f5c: f9657ce3     	bgeu	a0, s6, 0x6ef4 <core::fmt::Formatter::pad::h44850e4be722b02c+0x210>
    6f60: 010ba603     	lw	a2, 0x10(s7)
    6f64: 00190913     	addi	s2, s2, 0x1
    6f68: 000a0513     	mv	a0, s4
    6f6c: 00098593     	mv	a1, s3
    6f70: 000600e7     	jalr	a2
    6f74: fe0500e3     	beqz	a0, 0x6f54 <core::fmt::Formatter::pad::h44850e4be722b02c+0x270>
    6f78: f7dff06f     	j	0x6ef4 <core::fmt::Formatter::pad::h44850e4be722b02c+0x210>

00006f7c <core::str::count::do_count_chars::h7f3e012b2bb2993f>:
    6f7c: 00050613     	mv	a2, a0
    6f80: 00350513     	addi	a0, a0, 0x3
    6f84: ffc57513     	andi	a0, a0, -0x4
    6f88: 40c50833     	sub	a6, a0, a2
    6f8c: 0305f663     	bgeu	a1, a6, 0x6fb8 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x3c>
    6f90: 00000513     	li	a0, 0x0
    6f94: 02058063     	beqz	a1, 0x6fb4 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x38>
    6f98: 00b605b3     	add	a1, a2, a1
    6f9c: 00060683     	lb	a3, 0x0(a2)
    6fa0: 00160613     	addi	a2, a2, 0x1
    6fa4: fc06a693     	slti	a3, a3, -0x40
    6fa8: 0016c693     	xori	a3, a3, 0x1
    6fac: 00d50533     	add	a0, a0, a3
    6fb0: feb616e3     	bne	a2, a1, 0x6f9c <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x20>
    6fb4: 00008067     	ret
    6fb8: 41058733     	sub	a4, a1, a6
    6fbc: 00275693     	srli	a3, a4, 0x2
    6fc0: fc0688e3     	beqz	a3, 0x6f90 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x14>
    6fc4: 01060833     	add	a6, a2, a6
    6fc8: 00377593     	andi	a1, a4, 0x3
    6fcc: 00c51663     	bne	a0, a2, 0x6fd8 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x5c>
    6fd0: 00000513     	li	a0, 0x0
    6fd4: 0200006f     	j	0x6ff4 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x78>
    6fd8: 00000513     	li	a0, 0x0
    6fdc: 00060783     	lb	a5, 0x0(a2)
    6fe0: 00160613     	addi	a2, a2, 0x1
    6fe4: fc07a793     	slti	a5, a5, -0x40
    6fe8: 0017c793     	xori	a5, a5, 0x1
    6fec: 00f50533     	add	a0, a0, a5
    6ff0: ff0616e3     	bne	a2, a6, 0x6fdc <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x60>
    6ff4: 00000793     	li	a5, 0x0
    6ff8: 02058463     	beqz	a1, 0x7020 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0xa4>
    6ffc: ffc77613     	andi	a2, a4, -0x4
    7000: 00c80633     	add	a2, a6, a2
    7004: 00060703     	lb	a4, 0x0(a2)
    7008: fff58593     	addi	a1, a1, -0x1
    700c: fc072713     	slti	a4, a4, -0x40
    7010: 00174713     	xori	a4, a4, 0x1
    7014: 00e787b3     	add	a5, a5, a4
    7018: 00160613     	addi	a2, a2, 0x1
    701c: fe0594e3     	bnez	a1, 0x7004 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x88>
    7020: 010105b7     	lui	a1, 0x1010
    7024: 00ff0737     	lui	a4, 0xff0
    7028: 10158613     	addi	a2, a1, 0x101
    702c: 0ff70593     	addi	a1, a4, 0xff
    7030: 00a78533     	add	a0, a5, a0
    7034: 0340006f     	j	0x7068 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0xec>
    7038: 01070833     	add	a6, a4, a6
    703c: 40f686b3     	sub	a3, a3, a5
    7040: 0037f293     	andi	t0, a5, 0x3
    7044: 00b8f333     	and	t1, a7, a1
    7048: 0088d893     	srli	a7, a7, 0x8
    704c: 00b8f8b3     	and	a7, a7, a1
    7050: 006888b3     	add	a7, a7, t1
    7054: 01089313     	slli	t1, a7, 0x10
    7058: 011308b3     	add	a7, t1, a7
    705c: 0108d893     	srli	a7, a7, 0x10
    7060: 00a88533     	add	a0, a7, a0
    7064: 0a029863     	bnez	t0, 0x7114 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x198>
    7068: f40686e3     	beqz	a3, 0x6fb4 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x38>
    706c: 00080713     	mv	a4, a6
    7070: 0c000813     	li	a6, 0xc0
    7074: 00068793     	mv	a5, a3
    7078: 0106e463     	bltu	a3, a6, 0x7080 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x104>
    707c: 0c000793     	li	a5, 0xc0
    7080: 00279813     	slli	a6, a5, 0x2
    7084: 00000893     	li	a7, 0x0
    7088: 3f087293     	andi	t0, a6, 0x3f0
    708c: fa0286e3     	beqz	t0, 0x7038 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0xbc>
    7090: 005702b3     	add	t0, a4, t0
    7094: 00070313     	mv	t1, a4
    7098: 00032383     	lw	t2, 0x0(t1)
    709c: 00432e03     	lw	t3, 0x4(t1)
    70a0: 00832e83     	lw	t4, 0x8(t1)
    70a4: 00c32f03     	lw	t5, 0xc(t1)
    70a8: fff3cf93     	not	t6, t2
    70ac: 0063d393     	srli	t2, t2, 0x6
    70b0: 007fdf93     	srli	t6, t6, 0x7
    70b4: 007fe3b3     	or	t2, t6, t2
    70b8: fffe4f93     	not	t6, t3
    70bc: 006e5e13     	srli	t3, t3, 0x6
    70c0: 007fdf93     	srli	t6, t6, 0x7
    70c4: 01cfee33     	or	t3, t6, t3
    70c8: fffecf93     	not	t6, t4
    70cc: 006ede93     	srli	t4, t4, 0x6
    70d0: 007fdf93     	srli	t6, t6, 0x7
    70d4: 01dfeeb3     	or	t4, t6, t4
    70d8: ffff4f93     	not	t6, t5
    70dc: 006f5f13     	srli	t5, t5, 0x6
    70e0: 007fdf93     	srli	t6, t6, 0x7
    70e4: 01efef33     	or	t5, t6, t5
    70e8: 00c3f3b3     	and	t2, t2, a2
    70ec: 011388b3     	add	a7, t2, a7
    70f0: 00ce73b3     	and	t2, t3, a2
    70f4: 00cefe33     	and	t3, t4, a2
    70f8: 00cf7eb3     	and	t4, t5, a2
    70fc: 007e03b3     	add	t2, t3, t2
    7100: 011388b3     	add	a7, t2, a7
    7104: 01030313     	addi	t1, t1, 0x10
    7108: 011e88b3     	add	a7, t4, a7
    710c: f85316e3     	bne	t1, t0, 0x7098 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x11c>
    7110: f29ff06f     	j	0x7038 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0xbc>
    7114: 00000693     	li	a3, 0x0
    7118: 0fc7f793     	andi	a5, a5, 0xfc
    711c: 00279793     	slli	a5, a5, 0x2
    7120: 00f707b3     	add	a5, a4, a5
    7124: 00229713     	slli	a4, t0, 0x2
    7128: 0007a803     	lw	a6, 0x0(a5)
    712c: 00478793     	addi	a5, a5, 0x4
    7130: fff84893     	not	a7, a6
    7134: 00685813     	srli	a6, a6, 0x6
    7138: 0078d893     	srli	a7, a7, 0x7
    713c: 0108e833     	or	a6, a7, a6
    7140: 00c87833     	and	a6, a6, a2
    7144: ffc70713     	addi	a4, a4, -0x4
    7148: 00d806b3     	add	a3, a6, a3
    714c: fc071ee3     	bnez	a4, 0x7128 <core::str::count::do_count_chars::h7f3e012b2bb2993f+0x1ac>
    7150: 00b6f633     	and	a2, a3, a1
    7154: 0086d693     	srli	a3, a3, 0x8
    7158: 00b6f5b3     	and	a1, a3, a1
    715c: 00c585b3     	add	a1, a1, a2
    7160: 01059613     	slli	a2, a1, 0x10
    7164: 00b605b3     	add	a1, a2, a1
    7168: 0105d593     	srli	a1, a1, 0x10
    716c: 00a58533     	add	a0, a1, a0
    7170: 00008067     	ret

00007174 <core::panicking::assert_failed::hfc6c5954ad41c12a>:
    7174: ff010113     	addi	sp, sp, -0x10
    7178: 00112623     	sw	ra, 0xc(sp)
    717c: 00812423     	sw	s0, 0x8(sp)
    7180: 01010413     	addi	s0, sp, 0x10
    7184: 00070813     	mv	a6, a4
    7188: 00068793     	mv	a5, a3
    718c: feb42823     	sw	a1, -0x10(s0)
    7190: fec42a23     	sw	a2, -0xc(s0)
    7194: 04200637     	lui	a2, 0x4200
    7198: 47460613     	addi	a2, a2, 0x474
    719c: ff040593     	addi	a1, s0, -0x10
    71a0: ff440693     	addi	a3, s0, -0xc
    71a4: 00060713     	mv	a4, a2
    71a8: 00000097     	auipc	ra, 0x0
    71ac: 074080e7     	jalr	0x74(ra) <core::panicking::assert_failed_inner::he7ea224d76ea60aa>

000071b0 <core::panicking::panic_bounds_check::hf0fbe51e842a70af>:
    71b0: fc010113     	addi	sp, sp, -0x40
    71b4: 02112e23     	sw	ra, 0x3c(sp)
    71b8: 02812c23     	sw	s0, 0x38(sp)
    71bc: 04010413     	addi	s0, sp, 0x40
    71c0: fca42423     	sw	a0, -0x38(s0)
    71c4: fcb42623     	sw	a1, -0x34(s0)
    71c8: fcc40513     	addi	a0, s0, -0x34
    71cc: 000065b7     	lui	a1, 0x6
    71d0: 48458593     	addi	a1, a1, 0x484
    71d4: fc840693     	addi	a3, s0, -0x38
    71d8: 04200737     	lui	a4, 0x4200
    71dc: 49870713     	addi	a4, a4, 0x498
    71e0: 00200793     	li	a5, 0x2
    71e4: fe042023     	sw	zero, -0x20(s0)
    71e8: fea42423     	sw	a0, -0x18(s0)
    71ec: feb42623     	sw	a1, -0x14(s0)
    71f0: fed42823     	sw	a3, -0x10(s0)
    71f4: feb42a23     	sw	a1, -0xc(s0)
    71f8: fe840513     	addi	a0, s0, -0x18
    71fc: fce42823     	sw	a4, -0x30(s0)
    7200: fcf42a23     	sw	a5, -0x2c(s0)
    7204: fca42c23     	sw	a0, -0x28(s0)
    7208: fcf42e23     	sw	a5, -0x24(s0)
    720c: fd040513     	addi	a0, s0, -0x30
    7210: 00060593     	mv	a1, a2
    7214: 00000097     	auipc	ra, 0x0
    7218: 19c080e7     	jalr	0x19c(ra) <core::panicking::panic_fmt::h224b92ba1adb8ba8>

0000721c <core::panicking::assert_failed_inner::he7ea224d76ea60aa>:
    721c: f8010113     	addi	sp, sp, -0x80
    7220: 06112e23     	sw	ra, 0x7c(sp)
    7224: 06812c23     	sw	s0, 0x78(sp)
    7228: 06912a23     	sw	s1, 0x74(sp)
    722c: 07212823     	sw	s2, 0x70(sp)
    7230: 08010413     	addi	s0, sp, 0x80
    7234: 00080493     	mv	s1, a6
    7238: f8b42423     	sw	a1, -0x78(s0)
    723c: f8c42623     	sw	a2, -0x74(s0)
    7240: 00251513     	slli	a0, a0, 0x2
    7244: 042005b7     	lui	a1, 0x4200
    7248: 51858593     	addi	a1, a1, 0x518
    724c: 04200637     	lui	a2, 0x4200
    7250: 52460613     	addi	a2, a2, 0x524
    7254: 00a585b3     	add	a1, a1, a0
    7258: 00a60533     	add	a0, a2, a0
    725c: 0007a603     	lw	a2, 0x0(a5)
    7260: 0005a583     	lw	a1, 0x0(a1)
    7264: 00052503     	lw	a0, 0x0(a0)
    7268: f8d42823     	sw	a3, -0x70(s0)
    726c: f8e42a23     	sw	a4, -0x6c(s0)
    7270: f8b42c23     	sw	a1, -0x68(s0)
    7274: f8a42e23     	sw	a0, -0x64(s0)
    7278: 06061063     	bnez	a2, 0x72d8 <core::panicking::assert_failed_inner::he7ea224d76ea60aa+0xbc>
    727c: f9840513     	addi	a0, s0, -0x68
    7280: 000065b7     	lui	a1, 0x6
    7284: 46c58593     	addi	a1, a1, 0x46c
    7288: f8840613     	addi	a2, s0, -0x78
    728c: 000066b7     	lui	a3, 0x6
    7290: 45c68693     	addi	a3, a3, 0x45c
    7294: f9040713     	addi	a4, s0, -0x70
    7298: 042007b7     	lui	a5, 0x4200
    729c: 4d478793     	addi	a5, a5, 0x4d4
    72a0: 00300813     	li	a6, 0x3
    72a4: fe042423     	sw	zero, -0x18(s0)
    72a8: faa42c23     	sw	a0, -0x48(s0)
    72ac: fab42e23     	sw	a1, -0x44(s0)
    72b0: fcc42023     	sw	a2, -0x40(s0)
    72b4: fcd42223     	sw	a3, -0x3c(s0)
    72b8: fce42423     	sw	a4, -0x38(s0)
    72bc: fcd42623     	sw	a3, -0x34(s0)
    72c0: fb840513     	addi	a0, s0, -0x48
    72c4: fcf42c23     	sw	a5, -0x28(s0)
    72c8: fd042e23     	sw	a6, -0x24(s0)
    72cc: fea42023     	sw	a0, -0x20(s0)
    72d0: ff042223     	sw	a6, -0x1c(s0)
    72d4: 0840006f     	j	0x7358 <core::panicking::assert_failed_inner::he7ea224d76ea60aa+0x13c>
    72d8: fa040513     	addi	a0, s0, -0x60
    72dc: 01800613     	li	a2, 0x18
    72e0: fa040913     	addi	s2, s0, -0x60
    72e4: 00078593     	mv	a1, a5
    72e8: 00000097     	auipc	ra, 0x0
    72ec: 488080e7     	jalr	0x488(ra) <memcpy>
    72f0: f9840513     	addi	a0, s0, -0x68
    72f4: 000065b7     	lui	a1, 0x6
    72f8: 46c58593     	addi	a1, a1, 0x46c
    72fc: 00007637     	lui	a2, 0x7
    7300: 3dc60613     	addi	a2, a2, 0x3dc
    7304: f8840693     	addi	a3, s0, -0x78
    7308: 00006737     	lui	a4, 0x6
    730c: 45c70713     	addi	a4, a4, 0x45c
    7310: f9040793     	addi	a5, s0, -0x70
    7314: 04200837     	lui	a6, 0x4200
    7318: 4f880813     	addi	a6, a6, 0x4f8
    731c: faa42c23     	sw	a0, -0x48(s0)
    7320: fab42e23     	sw	a1, -0x44(s0)
    7324: fd242023     	sw	s2, -0x40(s0)
    7328: fcc42223     	sw	a2, -0x3c(s0)
    732c: 00400513     	li	a0, 0x4
    7330: fe042423     	sw	zero, -0x18(s0)
    7334: fcd42423     	sw	a3, -0x38(s0)
    7338: fce42623     	sw	a4, -0x34(s0)
    733c: fcf42823     	sw	a5, -0x30(s0)
    7340: fce42a23     	sw	a4, -0x2c(s0)
    7344: fb840593     	addi	a1, s0, -0x48
    7348: fd042c23     	sw	a6, -0x28(s0)
    734c: fca42e23     	sw	a0, -0x24(s0)
    7350: feb42023     	sw	a1, -0x20(s0)
    7354: fea42223     	sw	a0, -0x1c(s0)
    7358: fd840513     	addi	a0, s0, -0x28
    735c: 00048593     	mv	a1, s1
    7360: 00000097     	auipc	ra, 0x0
    7364: 050080e7     	jalr	0x50(ra) <core::panicking::panic_fmt::h224b92ba1adb8ba8>

00007368 <core::panicking::panic::ha1ed58f4f5473d93>:
    7368: fd010113     	addi	sp, sp, -0x30
    736c: 02112623     	sw	ra, 0x2c(sp)
    7370: 02812423     	sw	s0, 0x28(sp)
    7374: 03010413     	addi	s0, sp, 0x30
    7378: fea42823     	sw	a0, -0x10(s0)
    737c: feb42a23     	sw	a1, -0xc(s0)
    7380: ff040513     	addi	a0, s0, -0x10
    7384: 00100593     	li	a1, 0x1
    7388: fe042423     	sw	zero, -0x18(s0)
    738c: 00400693     	li	a3, 0x4
    7390: fca42c23     	sw	a0, -0x28(s0)
    7394: fcb42e23     	sw	a1, -0x24(s0)
    7398: fed42023     	sw	a3, -0x20(s0)
    739c: fe042223     	sw	zero, -0x1c(s0)
    73a0: fd840513     	addi	a0, s0, -0x28
    73a4: 00060593     	mv	a1, a2
    73a8: 00000097     	auipc	ra, 0x0
    73ac: 008080e7     	jalr	0x8(ra) <core::panicking::panic_fmt::h224b92ba1adb8ba8>

000073b0 <core::panicking::panic_fmt::h224b92ba1adb8ba8>:
    73b0: fe010113     	addi	sp, sp, -0x20
    73b4: 00112e23     	sw	ra, 0x1c(sp)
    73b8: 00812c23     	sw	s0, 0x18(sp)
    73bc: 02010413     	addi	s0, sp, 0x20
    73c0: 00100613     	li	a2, 0x1
    73c4: fea42623     	sw	a0, -0x14(s0)
    73c8: feb42823     	sw	a1, -0x10(s0)
    73cc: fec41a23     	sh	a2, -0xc(s0)
    73d0: fec40513     	addi	a0, s0, -0x14
    73d4: ffff9097     	auipc	ra, 0xffff9
    73d8: cb8080e7     	jalr	-0x348(ra) <_RNvCs6Gf8pSYpf6Z_7___rustc17rust_begin_unwind>

000073dc <<core::fmt::Arguments as core::fmt::Display>::fmt::h66710fea98750e90>:
    73dc: 0005a603     	lw	a2, 0x0(a1)
    73e0: 0045a583     	lw	a1, 0x4(a1)
    73e4: 00050693     	mv	a3, a0
    73e8: 00060513     	mv	a0, a2
    73ec: 00068613     	mv	a2, a3
    73f0: fffff317     	auipc	t1, 0xfffff
    73f4: 2a030067     	jr	0x2a0(t1) <core::fmt::write::heccac97d5faae40b>

000073f8 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f>:
    73f8: 00050613     	mv	a2, a0
    73fc: 00b57863     	bgeu	a0, a1, 0x740c <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x14>
    7400: 00000513     	li	a0, 0x0
    7404: 00060593     	mv	a1, a2
    7408: 00008067     	ret
    740c: 01065693     	srli	a3, a2, 0x10
    7410: 00060513     	mv	a0, a2
    7414: 08b6f863     	bgeu	a3, a1, 0x74a4 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0xac>
    7418: 00855793     	srli	a5, a0, 0x8
    741c: 08b7fa63     	bgeu	a5, a1, 0x74b0 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0xb8>
    7420: 00455813     	srli	a6, a0, 0x4
    7424: 00b86463     	bltu	a6, a1, 0x742c <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x34>
    7428: 00080513     	mv	a0, a6
    742c: 00255713     	srli	a4, a0, 0x2
    7430: 00b83833     	sltu	a6, a6, a1
    7434: 00b7b7b3     	sltu	a5, a5, a1
    7438: 00b6b6b3     	sltu	a3, a3, a1
    743c: 00b738b3     	sltu	a7, a4, a1
    7440: 00184813     	xori	a6, a6, 0x1
    7444: 0017c793     	xori	a5, a5, 0x1
    7448: 0016c293     	xori	t0, a3, 0x1
    744c: 0018c693     	xori	a3, a7, 0x1
    7450: 00429293     	slli	t0, t0, 0x4
    7454: 00379793     	slli	a5, a5, 0x3
    7458: 0057e7b3     	or	a5, a5, t0
    745c: 00281813     	slli	a6, a6, 0x2
    7460: 0107e7b3     	or	a5, a5, a6
    7464: 00b76463     	bltu	a4, a1, 0x746c <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x74>
    7468: 00070513     	mv	a0, a4
    746c: 00169693     	slli	a3, a3, 0x1
    7470: 00155513     	srli	a0, a0, 0x1
    7474: 00b53533     	sltu	a0, a0, a1
    7478: 00154513     	xori	a0, a0, 0x1
    747c: 00a6e533     	or	a0, a3, a0
    7480: 00a7e6b3     	or	a3, a5, a0
    7484: 00d59733     	sll	a4, a1, a3
    7488: 40e60633     	sub	a2, a2, a4
    748c: 00100513     	li	a0, 0x1
    7490: 00d51533     	sll	a0, a0, a3
    7494: 08b66e63     	bltu	a2, a1, 0x7530 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x138>
    7498: 02074463     	bltz	a4, 0x74c0 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0xc8>
    749c: 00050793     	mv	a5, a0
    74a0: 0540006f     	j	0x74f4 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0xfc>
    74a4: 00068513     	mv	a0, a3
    74a8: 0086d793     	srli	a5, a3, 0x8
    74ac: f6b7eae3     	bltu	a5, a1, 0x7420 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x28>
    74b0: 00078513     	mv	a0, a5
    74b4: 0047d813     	srli	a6, a5, 0x4
    74b8: f6b878e3     	bgeu	a6, a1, 0x7428 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x30>
    74bc: f71ff06f     	j	0x742c <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x34>
    74c0: 00175713     	srli	a4, a4, 0x1
    74c4: fff68693     	addi	a3, a3, -0x1
    74c8: 00100793     	li	a5, 0x1
    74cc: 00d797b3     	sll	a5, a5, a3
    74d0: 40e60833     	sub	a6, a2, a4
    74d4: 00082893     	slti	a7, a6, 0x0
    74d8: fff88893     	addi	a7, a7, -0x1
    74dc: 00f8f8b3     	and	a7, a7, a5
    74e0: 00085463     	bgez	a6, 0x74e8 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0xf0>
    74e4: 00060813     	mv	a6, a2
    74e8: 00a8e533     	or	a0, a7, a0
    74ec: 00080613     	mv	a2, a6
    74f0: 04b86063     	bltu	a6, a1, 0x7530 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x138>
    74f4: fff78793     	addi	a5, a5, -0x1
    74f8: 02068663     	beqz	a3, 0x7524 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x12c>
    74fc: 00068593     	mv	a1, a3
    7500: 00c0006f     	j	0x750c <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x114>
    7504: fff58593     	addi	a1, a1, -0x1
    7508: 00058e63     	beqz	a1, 0x7524 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x12c>
    750c: 00161613     	slli	a2, a2, 0x1
    7510: 40e60833     	sub	a6, a2, a4
    7514: 00180813     	addi	a6, a6, 0x1
    7518: fe0846e3     	bltz	a6, 0x7504 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x10c>
    751c: 00080613     	mv	a2, a6
    7520: fe5ff06f     	j	0x7504 <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f+0x10c>
    7524: 00f677b3     	and	a5, a2, a5
    7528: 00a7e533     	or	a0, a5, a0
    752c: 00d65633     	srl	a2, a2, a3
    7530: 00060593     	mv	a1, a2
    7534: 00008067     	ret

00007538 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a>:
    7538: ff010113     	addi	sp, sp, -0x10
    753c: 01000693     	li	a3, 0x10
    7540: 08d66063     	bltu	a2, a3, 0x75c0 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x88>
    7544: 40a006b3     	neg	a3, a0
    7548: 0036f693     	andi	a3, a3, 0x3
    754c: 00d507b3     	add	a5, a0, a3
    7550: 02f57463     	bgeu	a0, a5, 0x7578 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x40>
    7554: 00068713     	mv	a4, a3
    7558: 00050813     	mv	a6, a0
    755c: 00058893     	mv	a7, a1
    7560: 0008c283     	lbu	t0, 0x0(a7)
    7564: fff70713     	addi	a4, a4, -0x1
    7568: 00580023     	sb	t0, 0x0(a6)
    756c: 00180813     	addi	a6, a6, 0x1
    7570: 00188893     	addi	a7, a7, 0x1
    7574: fe0716e3     	bnez	a4, 0x7560 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x28>
    7578: 00d585b3     	add	a1, a1, a3
    757c: 40d60633     	sub	a2, a2, a3
    7580: ffc67713     	andi	a4, a2, -0x4
    7584: 0035f893     	andi	a7, a1, 0x3
    7588: 00e786b3     	add	a3, a5, a4
    758c: 06089063     	bnez	a7, 0x75ec <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xb4>
    7590: 00d7fe63     	bgeu	a5, a3, 0x75ac <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x74>
    7594: 00058813     	mv	a6, a1
    7598: 00082883     	lw	a7, 0x0(a6)
    759c: 0117a023     	sw	a7, 0x0(a5)
    75a0: 00478793     	addi	a5, a5, 0x4
    75a4: 00480813     	addi	a6, a6, 0x4
    75a8: fed7e8e3     	bltu	a5, a3, 0x7598 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x60>
    75ac: 00e585b3     	add	a1, a1, a4
    75b0: 00367613     	andi	a2, a2, 0x3
    75b4: 00c68733     	add	a4, a3, a2
    75b8: 00e6ea63     	bltu	a3, a4, 0x75cc <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    75bc: 0280006f     	j	0x75e4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>
    75c0: 00050693     	mv	a3, a0
    75c4: 00c50733     	add	a4, a0, a2
    75c8: 00e57e63     	bgeu	a0, a4, 0x75e4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>
    75cc: 0005c703     	lbu	a4, 0x0(a1)
    75d0: fff60613     	addi	a2, a2, -0x1
    75d4: 00e68023     	sb	a4, 0x0(a3)
    75d8: 00168693     	addi	a3, a3, 0x1
    75dc: 00158593     	addi	a1, a1, 0x1
    75e0: fe0616e3     	bnez	a2, 0x75cc <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    75e4: 01010113     	addi	sp, sp, 0x10
    75e8: 00008067     	ret
    75ec: 00000813     	li	a6, 0x0
    75f0: 00400293     	li	t0, 0x4
    75f4: 00012623     	sw	zero, 0xc(sp)
    75f8: 41128333     	sub	t1, t0, a7
    75fc: 00c10293     	addi	t0, sp, 0xc
    7600: 00137393     	andi	t2, t1, 0x1
    7604: 0112e2b3     	or	t0, t0, a7
    7608: 04039e63     	bnez	t2, 0x7664 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x12c>
    760c: 00237313     	andi	t1, t1, 0x2
    7610: 06031463     	bnez	t1, 0x7678 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x140>
    7614: 00c12e83     	lw	t4, 0xc(sp)
    7618: 00389813     	slli	a6, a7, 0x3
    761c: 00478293     	addi	t0, a5, 0x4
    7620: 41158f33     	sub	t5, a1, a7
    7624: 06d2fc63     	bgeu	t0, a3, 0x769c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x164>
    7628: 410002b3     	neg	t0, a6
    762c: 0182fe13     	andi	t3, t0, 0x18
    7630: 004f2283     	lw	t0, 0x4(t5)
    7634: 004f0393     	addi	t2, t5, 0x4
    7638: 010edeb3     	srl	t4, t4, a6
    763c: 00478313     	addi	t1, a5, 0x4
    7640: 01c29f33     	sll	t5, t0, t3
    7644: 01df6eb3     	or	t4, t5, t4
    7648: 00878f93     	addi	t6, a5, 0x8
    764c: 01d7a023     	sw	t4, 0x0(a5)
    7650: 00030793     	mv	a5, t1
    7654: 00038f13     	mv	t5, t2
    7658: 00028e93     	mv	t4, t0
    765c: fcdfeae3     	bltu	t6, a3, 0x7630 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xf8>
    7660: 0480006f     	j	0x76a8 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x170>
    7664: 0005c803     	lbu	a6, 0x0(a1)
    7668: 01028023     	sb	a6, 0x0(t0)
    766c: 00100813     	li	a6, 0x1
    7670: 00237313     	andi	t1, t1, 0x2
    7674: fa0300e3     	beqz	t1, 0x7614 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xdc>
    7678: 01058333     	add	t1, a1, a6
    767c: 00031303     	lh	t1, 0x0(t1)
    7680: 01028833     	add	a6, t0, a6
    7684: 00681023     	sh	t1, 0x0(a6)
    7688: 00c12e83     	lw	t4, 0xc(sp)
    768c: 00389813     	slli	a6, a7, 0x3
    7690: 00478293     	addi	t0, a5, 0x4
    7694: 41158f33     	sub	t5, a1, a7
    7698: f8d2e8e3     	bltu	t0, a3, 0x7628 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xf0>
    769c: 000e8293     	mv	t0, t4
    76a0: 000f0393     	mv	t2, t5
    76a4: 00078313     	mv	t1, a5
    76a8: 00010423     	sb	zero, 0x8(sp)
    76ac: 00100793     	li	a5, 0x1
    76b0: 00010323     	sb	zero, 0x6(sp)
    76b4: 00f89c63     	bne	a7, a5, 0x76cc <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x194>
    76b8: 00000893     	li	a7, 0x0
    76bc: 00000793     	li	a5, 0x0
    76c0: 00000e13     	li	t3, 0x0
    76c4: 00810e93     	addi	t4, sp, 0x8
    76c8: 01c0006f     	j	0x76e4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1ac>
    76cc: 0043c883     	lbu	a7, 0x4(t2)
    76d0: 0053c783     	lbu	a5, 0x5(t2)
    76d4: 00200e13     	li	t3, 0x2
    76d8: 01110423     	sb	a7, 0x8(sp)
    76dc: 00879793     	slli	a5, a5, 0x8
    76e0: 00610e93     	addi	t4, sp, 0x6
    76e4: 0015ff13     	andi	t5, a1, 0x1
    76e8: 000f1663     	bnez	t5, 0x76f4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1bc>
    76ec: 00000393     	li	t2, 0x0
    76f0: 01c0006f     	j	0x770c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1d4>
    76f4: 01c383b3     	add	t2, t2, t3
    76f8: 0043c883     	lbu	a7, 0x4(t2)
    76fc: 011e8023     	sb	a7, 0x0(t4)
    7700: 00614383     	lbu	t2, 0x6(sp)
    7704: 00814883     	lbu	a7, 0x8(sp)
    7708: 01039393     	slli	t2, t2, 0x10
    770c: 0113e8b3     	or	a7, t2, a7
    7710: 0102d2b3     	srl	t0, t0, a6
    7714: 41000833     	neg	a6, a6
    7718: 0117e7b3     	or	a5, a5, a7
    771c: 01887813     	andi	a6, a6, 0x18
    7720: 010797b3     	sll	a5, a5, a6
    7724: 0057e7b3     	or	a5, a5, t0
    7728: 00f32023     	sw	a5, 0x0(t1)
    772c: 00e585b3     	add	a1, a1, a4
    7730: 00367613     	andi	a2, a2, 0x3
    7734: 00c68733     	add	a4, a3, a2
    7738: e8e6eae3     	bltu	a3, a4, 0x75cc <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    773c: ea9ff06f     	j	0x75e4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>

00007740 <memcmp>:
    7740: 02060063     	beqz	a2, 0x7760 <memcmp+0x20>
    7744: 00054683     	lbu	a3, 0x0(a0)
    7748: 0005c703     	lbu	a4, 0x0(a1)
    774c: 00e69e63     	bne	a3, a4, 0x7768 <memcmp+0x28>
    7750: fff60613     	addi	a2, a2, -0x1
    7754: 00158593     	addi	a1, a1, 0x1
    7758: 00150513     	addi	a0, a0, 0x1
    775c: fe0614e3     	bnez	a2, 0x7744 <memcmp+0x4>
    7760: 00000513     	li	a0, 0x0
    7764: 00008067     	ret
    7768: 40e68533     	sub	a0, a3, a4
    776c: 00008067     	ret

00007770 <memcpy>:
    7770: 00000317     	auipc	t1, 0x0
    7774: dc830067     	jr	-0x238(t1) <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a>

00007778 <__udivsi3>:
    7778: 00000317     	auipc	t1, 0x0
    777c: c8030067     	jr	-0x380(t1) <compiler_builtins::int::specialized_div_rem::u32_div_rem::h5ee01a13f63f9b7f>

00007780 <memset>:
    7780: 01000693     	li	a3, 0x10
    7784: 08d66263     	bltu	a2, a3, 0x7808 <memset+0x88>
    7788: 40a006b3     	neg	a3, a0
    778c: 0036f693     	andi	a3, a3, 0x3
    7790: 00d50733     	add	a4, a0, a3
    7794: 00e57e63     	bgeu	a0, a4, 0x77b0 <memset+0x30>
    7798: 00068793     	mv	a5, a3
    779c: 00050813     	mv	a6, a0
    77a0: 00b80023     	sb	a1, 0x0(a6)
    77a4: fff78793     	addi	a5, a5, -0x1
    77a8: 00180813     	addi	a6, a6, 0x1
    77ac: fe079ae3     	bnez	a5, 0x77a0 <memset+0x20>
    77b0: 40d60633     	sub	a2, a2, a3
    77b4: ffc67693     	andi	a3, a2, -0x4
    77b8: 00d706b3     	add	a3, a4, a3
    77bc: 02d77663     	bgeu	a4, a3, 0x77e8 <memset+0x68>
    77c0: 0ff5f793     	zext.b	a5, a1
    77c4: 01859813     	slli	a6, a1, 0x18
    77c8: 00879893     	slli	a7, a5, 0x8
    77cc: 0117e8b3     	or	a7, a5, a7
    77d0: 01079793     	slli	a5, a5, 0x10
    77d4: 0107e7b3     	or	a5, a5, a6
    77d8: 00f8e7b3     	or	a5, a7, a5
    77dc: 00f72023     	sw	a5, 0x0(a4)
    77e0: 00470713     	addi	a4, a4, 0x4
    77e4: fed76ce3     	bltu	a4, a3, 0x77dc <memset+0x5c>
    77e8: 00367613     	andi	a2, a2, 0x3
    77ec: 00c68733     	add	a4, a3, a2
    77f0: 00e6fa63     	bgeu	a3, a4, 0x7804 <memset+0x84>
    77f4: 00b68023     	sb	a1, 0x0(a3)
    77f8: fff60613     	addi	a2, a2, -0x1
    77fc: 00168693     	addi	a3, a3, 0x1
    7800: fe061ae3     	bnez	a2, 0x77f4 <memset+0x74>
    7804: 00008067     	ret
    7808: 00050693     	mv	a3, a0
    780c: 00c50733     	add	a4, a0, a2
    7810: fee562e3     	bltu	a0, a4, 0x77f4 <memset+0x74>
    7814: ff1ff06f     	j	0x7804 <memset+0x84>
