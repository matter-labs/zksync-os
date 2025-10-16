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
      34: 004000ef     	jal	0x38 <test_program::main::h84ef4c6db5efb25d>

00000038 <test_program::main::h84ef4c6db5efb25d>:
      38: ff010113     	addi	sp, sp, -0x10
      3c: 00112623     	sw	ra, 0xc(sp)
      40: 00812423     	sw	s0, 0x8(sp)
      44: 01010413     	addi	s0, sp, 0x10
      48: 004000ef     	jal	0x4c <test_program::workload::h078c841317438149>

0000004c <test_program::workload::h078c841317438149>:
      4c: ff010113     	addi	sp, sp, -0x10
      50: 00112623     	sw	ra, 0xc(sp)
      54: 00812423     	sw	s0, 0x8(sp)
      58: 01010413     	addi	s0, sp, 0x10
      5c: 04200537     	lui	a0, 0x4200
      60: 00050513     	mv	a0, a0
      64: 04200637     	lui	a2, 0x4200
      68: 09860613     	addi	a2, a2, 0x98
      6c: 40a60633     	sub	a2, a2, a0
      70: 000015b7     	lui	a1, 0x1
      74: 4c858593     	addi	a1, a1, 0x4c8
      78: 3b0010ef     	jal	0x1428 <memcpy>
      7c: 14c000ef     	jal	0x1c8 <crypto::sha3::delegated::tests::bad_keccak_f1600_test::h4df1a538571d9301>
      80: 04200537     	lui	a0, 0x4200
      84: 00050513     	mv	a0, a0
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

000001c8 <crypto::sha3::delegated::tests::bad_keccak_f1600_test::h4df1a538571d9301>:
     1c8: d0010113     	addi	sp, sp, -0x300
     1cc: 2e112e23     	sw	ra, 0x2fc(sp)
     1d0: 2e812c23     	sw	s0, 0x2f8(sp)
     1d4: 2e912a23     	sw	s1, 0x2f4(sp)
     1d8: 2f212823     	sw	s2, 0x2f0(sp)
     1dc: 2f312623     	sw	s3, 0x2ec(sp)
     1e0: 2f412423     	sw	s4, 0x2e8(sp)
     1e4: 2f512223     	sw	s5, 0x2e4(sp)
     1e8: 2f612023     	sw	s6, 0x2e0(sp)
     1ec: 2d712e23     	sw	s7, 0x2dc(sp)
     1f0: 2d812c23     	sw	s8, 0x2d8(sp)
     1f4: 2d912a23     	sw	s9, 0x2d4(sp)
     1f8: 2da12823     	sw	s10, 0x2d0(sp)
     1fc: 2db12623     	sw	s11, 0x2cc(sp)
     200: 30010413     	addi	s0, sp, 0x300
     204: f0017113     	andi	sp, sp, -0x100
     208: 2d5c9937     	lui	s2, 0x2d5c9
     20c: f96ed9b7     	lui	s3, 0xf96ed
     210: 6a333a37     	lui	s4, 0x6a333
     214: 7057bbb7     	lui	s7, 0x7057b
     218: 093d9d37     	lui	s10, 0x93d9
     21c: 70d77db7     	lui	s11, 0x70d77
     220: 8a20ec37     	lui	s8, 0x8a20e
     224: 5569d0b7     	lui	ra, 0x5569d
     228: 4f9c5ab7     	lui	s5, 0x4f9c5
     22c: e5e7fb37     	lui	s6, 0xe5e7f
     230: f957cfb7     	lui	t6, 0xf957c
     234: da660cb7     	lui	s9, 0xda660
     238: 857742b7     	lui	t0, 0x85774
     23c: 1275beb7     	lui	t4, 0x1275b
     240: c3d814b7     	lui	s1, 0xc3d81
     244: 1f1ba5b7     	lui	a1, 0x1f1ba
     248: f79a86b7     	lui	a3, 0xf79a8
     24c: e4fed837     	lui	a6, 0xe4fed
     250: ee98b3b7     	lui	t2, 0xee98b
     254: 68ce6637     	lui	a2, 0x68ce6
     258: b9ce77b7     	lui	a5, 0xb9ce7
     25c: deea6337     	lui	t1, 0xdeea6
     260: ba8f9f37     	lui	t5, 0xba8f9
     264: 33c44737     	lui	a4, 0x33c44
     268: 6eafb8b7     	lui	a7, 0x6eafb
     26c: e0065e37     	lui	t3, 0xe0065
     270: 54d90913     	addi	s2, s2, 0x54d
     274: b3c98993     	addi	s3, s3, -0x4c4
     278: 03312c23     	sw	s3, 0x38(sp)
     27c: 2719e9b7     	lui	s3, 0x2719e
     280: 56db8b93     	addi	s7, s7, 0x56d
     284: 03212e23     	sw	s2, 0x3c(sp)
     288: 7cf8b937     	lui	s2, 0x7cf8b
     28c: cd0a0513     	addi	a0, s4, -0x330
     290: 05712023     	sw	s7, 0x40(sp)
     294: 09831a37     	lui	s4, 0x9831
     298: 04a12223     	sw	a0, 0x44(sp)
     29c: fd545bb7     	lui	s7, 0xfd545
     2a0: d12d0513     	addi	a0, s10, -0x2ee
     2a4: b6cd8d13     	addi	s10, s11, -0x494
     2a8: 05a12423     	sw	s10, 0x48(sp)
     2ac: bf174db7     	lui	s11, 0xbf174
     2b0: 09408093     	addi	ra, ra, 0x94
     2b4: 04a12623     	sw	a0, 0x4c(sp)
     2b8: 97ddbd37     	lui	s10, 0x97ddb
     2bc: 9b2c0513     	addi	a0, s8, -0x64e
     2c0: 04112823     	sw	ra, 0x50(sp)
     2c4: d8995c37     	lui	s8, 0xd8995
     2c8: 04a12a23     	sw	a0, 0x54(sp)
     2cc: 48ead0b7     	lui	ra, 0x48ead
     2d0: f99a8513     	addi	a0, s5, -0x67
     2d4: 156b0a93     	addi	s5, s6, 0x156
     2d8: 05512c23     	sw	s5, 0x58(sp)
     2dc: 5d0beb37     	lui	s6, 0x5d0be
     2e0: b38c8c93     	addi	s9, s9, -0x4c8
     2e4: 04a12e23     	sw	a0, 0x5c(sp)
     2e8: e3b8dab7     	lui	s5, 0xe3b8d
     2ec: 9a2f8513     	addi	a0, t6, -0x65e
     2f0: 07912023     	sw	s9, 0x60(sp)
     2f4: 55b7bfb7     	lui	t6, 0x55b7b
     2f8: 06a12223     	sw	a0, 0x64(sp)
     2fc: 91a02cb7     	lui	s9, 0x91a02
     300: dae28513     	addi	a0, t0, -0x252
     304: f0de8293     	addi	t0, t4, -0xf3
     308: 06512423     	sw	t0, 0x68(sp)
     30c: 649e4eb7     	lui	t4, 0x649e4
     310: 0f748493     	addi	s1, s1, 0xf7
     314: 06a12623     	sw	a0, 0x6c(sp)
     318: 900e32b7     	lui	t0, 0x900e3
     31c: faf4f537     	lui	a0, 0xfaf4f
     320: 24750513     	addi	a0, a0, 0x247
     324: 06912823     	sw	s1, 0x70(sp)
     328: 06a12a23     	sw	a0, 0x74(sp)
     32c: e7bae537     	lui	a0, 0xe7bae
     330: ee658593     	addi	a1, a1, -0x11a
     334: 75968693     	addi	a3, a3, 0x759
     338: c0f80813     	addi	a6, a6, -0x3f1
     33c: 42538393     	addi	t2, t2, 0x425
     340: 06d12c23     	sw	a3, 0x78(sp)
     344: 06b12e23     	sw	a1, 0x7c(sp)
     348: 08712023     	sw	t2, 0x80(sp)
     34c: 09012223     	sw	a6, 0x84(sp)
     350: 202aa5b7     	lui	a1, 0x202aa
     354: 1b660613     	addi	a2, a2, 0x1b6
     358: 8a178693     	addi	a3, a5, -0x75f
     35c: 6c430793     	addi	a5, t1, 0x6c4
     360: 74ff0813     	addi	a6, t5, 0x74f
     364: 08d12423     	sw	a3, 0x88(sp)
     368: 08c12623     	sw	a2, 0x8c(sp)
     36c: 09012823     	sw	a6, 0x90(sp)
     370: 08f12a23     	sw	a5, 0x94(sp)
     374: faa3d637     	lui	a2, 0xfaa3d
     378: d8370693     	addi	a3, a4, -0x27d
     37c: 1f588713     	addi	a4, a7, 0x1f5
     380: 404e0793     	addi	a5, t3, 0x404
     384: bd998813     	addi	a6, s3, -0x427
     388: 08e12c23     	sw	a4, 0x98(sp)
     38c: 08d12e23     	sw	a3, 0x9c(sp)
     390: 0b012023     	sw	a6, 0xa0(sp)
     394: 0af12223     	sw	a5, 0xa4(sp)
     398: 5b3406b7     	lui	a3, 0x5b340
     39c: 9f090713     	addi	a4, s2, -0x610
     3a0: 265a0793     	addi	a5, s4, 0x265
     3a4: 9a6b8813     	addi	a6, s7, -0x65a
     3a8: 743d8893     	addi	a7, s11, 0x743
     3ac: 0af12423     	sw	a5, 0xa8(sp)
     3b0: 0ae12623     	sw	a4, 0xac(sp)
     3b4: 0b112823     	sw	a7, 0xb0(sp)
     3b8: 0b012a23     	sw	a6, 0xb4(sp)
     3bc: 4e1c4737     	lui	a4, 0x4e1c4
     3c0: d33d0793     	addi	a5, s10, -0x2cd
     3c4: b40c0813     	addi	a6, s8, -0x4c0
     3c8: 5fc08893     	addi	a7, ra, 0x5fc
     3cc: 774b0313     	addi	t1, s6, 0x774
     3d0: 0b012c23     	sw	a6, 0xb8(sp)
     3d4: 0af12e23     	sw	a5, 0xbc(sp)
     3d8: 0c612023     	sw	t1, 0xc0(sp)
     3dc: 0d112223     	sw	a7, 0xc4(sp)
     3e0: 609f57b7     	lui	a5, 0x609f5
     3e4: 8eea8813     	addi	a6, s5, -0x712
     3e8: 03cf8893     	addi	a7, t6, 0x3c
     3ec: 26ec8313     	addi	t1, s9, 0x26e
     3f0: 2e9e8393     	addi	t2, t4, 0x2e9
     3f4: 0d112423     	sw	a7, 0xc8(sp)
     3f8: 0d012623     	sw	a6, 0xcc(sp)
     3fc: 0c712823     	sw	t2, 0xd0(sp)
     400: 0c612a23     	sw	t1, 0xd4(sp)
     404: a44c1837     	lui	a6, 0xa44c1
     408: 12928893     	addi	a7, t0, 0x129
     40c: d7b50513     	addi	a0, a0, -0x285
     410: ec558593     	addi	a1, a1, -0x13b
     414: ce860613     	addi	a2, a2, -0x318
     418: 0ca12c23     	sw	a0, 0xd8(sp)
     41c: 0d112e23     	sw	a7, 0xdc(sp)
     420: 0ec12023     	sw	a2, 0xe0(sp)
     424: 0eb12223     	sw	a1, 0xe4(sp)
     428: 00100513     	li	a0, 0x1
     42c: 24668593     	addi	a1, a3, 0x246
     430: db670613     	addi	a2, a4, -0x24a
     434: e6278693     	addi	a3, a5, -0x19e
     438: 05980713     	addi	a4, a6, 0x59
     43c: 0ea12c23     	sw	a0, 0xf8(sp)
     440: 0e012e23     	sw	zero, 0xfc(sp)
     444: 0ec12423     	sw	a2, 0xe8(sp)
     448: 0eb12623     	sw	a1, 0xec(sp)
     44c: 0ee12823     	sw	a4, 0xf0(sp)
     450: 0ed12a23     	sw	a3, 0xf4(sp)
     454: 1c810513     	addi	a0, sp, 0x1c8
     458: 03000613     	li	a2, 0x30
     45c: 00000593     	li	a1, 0x0
     460: 7d1000ef     	jal	0x1430 <memset>
     464: f1259937     	lui	s2, 0xf1259
     468: 40e1e9b7     	lui	s3, 0x40e1e
     46c: 84d5da37     	lui	s4, 0x84d5d
     470: 33c04bb7     	lui	s7, 0x33c04
     474: d5982d37     	lui	s10, 0xd5982
     478: a65abdb7     	lui	s11, 0xa65ab
     47c: bd154c37     	lui	s8, 0xbd154
     480: 6f8050b7     	lui	ra, 0x6f805
     484: 8b285ab7     	lui	s5, 0x8b285
     488: 6253db37     	lui	s6, 0x6253d
     48c: ff97afb7     	lui	t6, 0xff97a
     490: 7f8e7cb7     	lui	s9, 0x7f8e7
     494: 90fee2b7     	lui	t0, 0x90fee
     498: a4464eb7     	lui	t4, 0xa4464
     49c: d61934b7     	lui	s1, 0xd6193
     4a0: ad30a5b7     	lui	a1, 0xad30a
     4a4: 1b1906b7     	lui	a3, 0x1b190
     4a8: 30936837     	lui	a6, 0x30936
     4ac: d09003b7     	lui	t2, 0xd0900
     4b0: eb5ab637     	lui	a2, 0xeb5ab
     4b4: 2317d7b7     	lui	a5, 0x2317d
     4b8: a9a6e337     	lui	t1, 0xa9a6e
     4bc: 0d712f37     	lui	t5, 0xd712
     4c0: 81a58737     	lui	a4, 0x81a58
     4c4: dbcf58b7     	lui	a7, 0xdbcf5
     4c8: 43b83e37     	lui	t3, 0x43b83
     4cc: f7990913     	addi	s2, s2, -0x87
     4d0: de798993     	addi	s3, s3, -0x219
     4d4: 11312023     	sw	s3, 0x100(sp)
     4d8: 0347d9b7     	lui	s3, 0x347d
     4dc: 78ab8b93     	addi	s7, s7, 0x78a
     4e0: 11212223     	sw	s2, 0x104(sp)
     4e4: 01f23937     	lui	s2, 0x1f23
     4e8: cf9a0513     	addi	a0, s4, -0x307
     4ec: 11712423     	sw	s7, 0x108(sp)
     4f0: 11a55a37     	lui	s4, 0x11a55
     4f4: 10a12623     	sw	a0, 0x10c(sp)
     4f8: 05e56bb7     	lui	s7, 0x5e56
     4fc: 61ed0513     	addi	a0, s10, 0x61e
     500: 9eed8d13     	addi	s10, s11, -0x612
     504: 11a12823     	sw	s10, 0x110(sp)
     508: 21d9bdb7     	lui	s11, 0x21d9b
     50c: 94d08093     	addi	ra, ra, -0x6b3
     510: 10a12a23     	sw	a0, 0x114(sp)
     514: 64bf0d37     	lui	s10, 0x64bf0
     518: 730c0513     	addi	a0, s8, 0x730
     51c: 10112c23     	sw	ra, 0x118(sp)
     520: 8cc97c37     	lui	s8, 0x8cc97
     524: 10a12e23     	sw	a0, 0x11c(sp)
     528: 613670b7     	lui	ra, 0x61367
     52c: e05a8513     	addi	a0, s5, -0x1fb
     530: 057b0a93     	addi	s5, s6, 0x57
     534: 13512023     	sw	s5, 0x120(sp)
     538: 7bc46b37     	lui	s6, 0x7bc46
     53c: fd4c8c93     	addi	s9, s9, -0x2c
     540: 12a12223     	sw	a0, 0x124(sp)
     544: b87c6ab7     	lui	s5, 0xb87c6
     548: 42df8513     	addi	a0, t6, 0x42d
     54c: 13912423     	sw	s9, 0x128(sp)
     550: 4fd01fb7     	lui	t6, 0x4fd01
     554: 12a12623     	sw	a0, 0x12c(sp)
     558: 8c3efcb7     	lui	s9, 0x8c3ef
     55c: 5a028513     	addi	a0, t0, 0x5a0
     560: 7c4e8293     	addi	t0, t4, 0x7c4
     564: 12512823     	sw	t0, 0x130(sp)
     568: 1ccf3eb7     	lui	t4, 0x1ccf3
     56c: e7648493     	addi	s1, s1, -0x18a
     570: 12a12a23     	sw	a0, 0x134(sp)
     574: 940c82b7     	lui	t0, 0x940c8
     578: 8c5be537     	lui	a0, 0x8c5be
     57c: a0c50513     	addi	a0, a0, -0x5f4
     580: 12912c23     	sw	s1, 0x138(sp)
     584: 12a12e23     	sw	a0, 0x13c(sp)
     588: ae3a2537     	lui	a0, 0xae3a2
     58c: 6f758593     	addi	a1, a1, 0x6f7
     590: 59c68693     	addi	a3, a3, 0x59c
     594: ab780813     	addi	a6, a6, -0x549
     598: c6438393     	addi	t2, t2, -0x39c
     59c: 14d12023     	sw	a3, 0x140(sp)
     5a0: 14b12223     	sw	a1, 0x144(sp)
     5a4: 14712423     	sw	t2, 0x148(sp)
     5a8: 15012623     	sw	a6, 0x14c(sp)
     5ac: 184205b7     	lui	a1, 0x18420
     5b0: 93f60613     	addi	a2, a2, -0x6c1
     5b4: 63578693     	addi	a3, a5, 0x635
     5b8: 62630793     	addi	a5, t1, 0x626
     5bc: 103f0813     	addi	a6, t5, 0x103
     5c0: 14d12823     	sw	a3, 0x150(sp)
     5c4: 14c12a23     	sw	a2, 0x154(sp)
     5c8: 15012c23     	sw	a6, 0x158(sp)
     5cc: 14f12e23     	sw	a5, 0x15c(sp)
     5d0: a2c51637     	lui	a2, 0xa2c51
     5d4: c1670693     	addi	a3, a4, -0x3ea
     5d8: 55f88713     	addi	a4, a7, 0x55f
     5dc: 1cde0793     	addi	a5, t3, 0x1cd
     5e0: 82698813     	addi	a6, s3, -0x7da
     5e4: 16e12023     	sw	a4, 0x160(sp)
     5e8: 16d12223     	sw	a3, 0x164(sp)
     5ec: 17012423     	sw	a6, 0x168(sp)
     5f0: 16f12623     	sw	a5, 0x16c(sp)
     5f4: 16f536b7     	lui	a3, 0x16f53
     5f8: f1a90713     	addi	a4, s2, -0xe6
     5fc: 69fa0793     	addi	a5, s4, 0x69f
     600: 35ab8813     	addi	a6, s7, 0x35a
     604: e61d8893     	addi	a7, s11, -0x19f
     608: 16f12823     	sw	a5, 0x170(sp)
     60c: 16e12a23     	sw	a4, 0x174(sp)
     610: 17112c23     	sw	a7, 0x178(sp)
     614: 17012e23     	sw	a6, 0x17c(sp)
     618: e7046737     	lui	a4, 0xe7046
     61c: ef2d0793     	addi	a5, s10, -0x10e
     620: 0f2c0813     	addi	a6, s8, 0xf2
     624: 09508893     	addi	a7, ra, 0x95
     628: 611b0313     	addi	t1, s6, 0x611
     62c: 19012023     	sw	a6, 0x180(sp)
     630: 18f12223     	sw	a5, 0x184(sp)
     634: 18612423     	sw	t1, 0x188(sp)
     638: 19112623     	sw	a7, 0x18c(sp)
     63c: 75f647b7     	lui	a5, 0x75f64
     640: a55a8813     	addi	a6, s5, -0x5ab
     644: ecbf8893     	addi	a7, t6, -0x135
     648: 88ac8313     	addi	t1, s9, -0x776
     64c: 2c8e8393     	addi	t2, t4, 0x2c8
     650: 19112823     	sw	a7, 0x190(sp)
     654: 19012a23     	sw	a6, 0x194(sp)
     658: 18712c23     	sw	t2, 0x198(sp)
     65c: 18612e23     	sw	t1, 0x19c(sp)
     660: 7f30a837     	lui	a6, 0x7f30a
     664: 92228893     	addi	a7, t0, -0x6de
     668: 61450513     	addi	a0, a0, 0x614
     66c: 92458593     	addi	a1, a1, -0x6dc
     670: 9e460613     	addi	a2, a2, -0x61c
     674: 1aa12023     	sw	a0, 0x1a0(sp)
     678: 1b112223     	sw	a7, 0x1a4(sp)
     67c: 1ac12423     	sw	a2, 0x1a8(sp)
     680: 1ab12623     	sw	a1, 0x1ac(sp)
     684: eaf20537     	lui	a0, 0xeaf20
     688: 52668593     	addi	a1, a3, 0x526
     68c: 5c270613     	addi	a2, a4, 0x5c2
     690: 4e978693     	addi	a3, a5, 0x4e9
     694: 13b80713     	addi	a4, a6, 0x13b
     698: 1ac12823     	sw	a2, 0x1b0(sp)
     69c: 1ab12a23     	sw	a1, 0x1b4(sp)
     6a0: 1ae12c23     	sw	a4, 0x1b8(sp)
     6a4: 1ad12e23     	sw	a3, 0x1bc(sp)
     6a8: 5ceca5b7     	lui	a1, 0x5ceca
     6ac: f7b50513     	addi	a0, a0, -0x85
     6b0: 24958593     	addi	a1, a1, 0x249
     6b4: 1cb12023     	sw	a1, 0x1c0(sp)
     6b8: 1ca12223     	sw	a0, 0x1c4(sp)
     6bc: 10010513     	addi	a0, sp, 0x100
     6c0: 070000ef     	jal	0x730 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>
     6c4: 10010513     	addi	a0, sp, 0x100
     6c8: 03810593     	addi	a1, sp, 0x38
     6cc: 0c800613     	li	a2, 0xc8
     6d0: 529000ef     	jal	0x13f8 <memcmp>
     6d4: 04051263     	bnez	a0, 0x718 <crypto::sha3::delegated::tests::bad_keccak_f1600_test::h4df1a538571d9301+0x550>
     6d8: d0040113     	addi	sp, s0, -0x300
     6dc: 2fc12083     	lw	ra, 0x2fc(sp)
     6e0: 2f812403     	lw	s0, 0x2f8(sp)
     6e4: 2f412483     	lw	s1, 0x2f4(sp)
     6e8: 2f012903     	lw	s2, 0x2f0(sp)
     6ec: 2ec12983     	lw	s3, 0x2ec(sp)
     6f0: 2e812a03     	lw	s4, 0x2e8(sp)
     6f4: 2e412a83     	lw	s5, 0x2e4(sp)
     6f8: 2e012b03     	lw	s6, 0x2e0(sp)
     6fc: 2dc12b83     	lw	s7, 0x2dc(sp)
     700: 2d812c03     	lw	s8, 0x2d8(sp)
     704: 2d412c83     	lw	s9, 0x2d4(sp)
     708: 2d012d03     	lw	s10, 0x2d0(sp)
     70c: 2cc12d83     	lw	s11, 0x2cc(sp)
     710: 30010113     	addi	sp, sp, 0x300
     714: 00008067     	ret
     718: 04200537     	lui	a0, 0x4200
     71c: 05750513     	addi	a0, a0, 0x57
     720: 04200637     	lui	a2, 0x4200
     724: 08860613     	addi	a2, a2, 0x88
     728: 02f00593     	li	a1, 0x2f
     72c: 251000ef     	jal	0x117c <core::panicking::panic::ha1ed58f4f5473d93>

00000730 <crypto::sha3::delegated::precompile::keccak_f1600::hd97d6f81616d3337>:
     730: ff010113     	addi	sp, sp, -0x10
     734: 00112623     	sw	ra, 0xc(sp)
     738: 00812423     	sw	s0, 0x8(sp)
     73c: 01010413     	addi	s0, sp, 0x10
     740: 00050593     	mv	a1, a0
     744: 00000533     	add	a0, zero, zero
     748: 7cb01073     	csrw	0x7cb, zero
     74c: 7cb01073     	csrw	0x7cb, zero
     750: 7cb01073     	csrw	0x7cb, zero
     754: 7cb01073     	csrw	0x7cb, zero
     758: 7cb01073     	csrw	0x7cb, zero
     75c: 7cb01073     	csrw	0x7cb, zero
     760: 7cb01073     	csrw	0x7cb, zero
     764: 7cb01073     	csrw	0x7cb, zero
     768: 7cb01073     	csrw	0x7cb, zero
     76c: 7cb01073     	csrw	0x7cb, zero
     770: 7cb01073     	csrw	0x7cb, zero
     774: 7cb01073     	csrw	0x7cb, zero
     778: 7cb01073     	csrw	0x7cb, zero
     77c: 7cb01073     	csrw	0x7cb, zero
     780: 7cb01073     	csrw	0x7cb, zero
     784: 7cb01073     	csrw	0x7cb, zero
     788: 7cb01073     	csrw	0x7cb, zero
     78c: 7cb01073     	csrw	0x7cb, zero
     790: 7cb01073     	csrw	0x7cb, zero
     794: 7cb01073     	csrw	0x7cb, zero
     798: 7cb01073     	csrw	0x7cb, zero
     79c: 7cb01073     	csrw	0x7cb, zero
     7a0: 7cb01073     	csrw	0x7cb, zero
     7a4: 7cb01073     	csrw	0x7cb, zero
     7a8: 7cb01073     	csrw	0x7cb, zero
     7ac: 7cb01073     	csrw	0x7cb, zero
     7b0: 7cb01073     	csrw	0x7cb, zero
     7b4: 7cb01073     	csrw	0x7cb, zero
     7b8: 7cb01073     	csrw	0x7cb, zero
     7bc: 7cb01073     	csrw	0x7cb, zero
     7c0: 7cb01073     	csrw	0x7cb, zero
     7c4: 7cb01073     	csrw	0x7cb, zero
     7c8: 7cb01073     	csrw	0x7cb, zero
     7cc: 7cb01073     	csrw	0x7cb, zero
     7d0: 7cb01073     	csrw	0x7cb, zero
     7d4: 7cb01073     	csrw	0x7cb, zero
     7d8: 7cb01073     	csrw	0x7cb, zero
     7dc: 7cb01073     	csrw	0x7cb, zero
     7e0: 7cb01073     	csrw	0x7cb, zero
     7e4: 7cb01073     	csrw	0x7cb, zero
     7e8: 7cb01073     	csrw	0x7cb, zero
     7ec: 7cb01073     	csrw	0x7cb, zero
     7f0: 7cb01073     	csrw	0x7cb, zero
     7f4: 7cb01073     	csrw	0x7cb, zero
     7f8: 7cb01073     	csrw	0x7cb, zero
     7fc: 7cb01073     	csrw	0x7cb, zero
     800: 7cb01073     	csrw	0x7cb, zero
     804: 7cb01073     	csrw	0x7cb, zero
     808: 7cb01073     	csrw	0x7cb, zero
     80c: 7cb01073     	csrw	0x7cb, zero
     810: 7cb01073     	csrw	0x7cb, zero
     814: 7cb01073     	csrw	0x7cb, zero
     818: 7cb01073     	csrw	0x7cb, zero
     81c: 7cb01073     	csrw	0x7cb, zero
     820: 7cb01073     	csrw	0x7cb, zero
     824: 7cb01073     	csrw	0x7cb, zero
     828: 7cb01073     	csrw	0x7cb, zero
     82c: 7cb01073     	csrw	0x7cb, zero
     830: 7cb01073     	csrw	0x7cb, zero
     834: 7cb01073     	csrw	0x7cb, zero
     838: 7cb01073     	csrw	0x7cb, zero
     83c: 7cb01073     	csrw	0x7cb, zero
     840: 7cb01073     	csrw	0x7cb, zero
     844: 7cb01073     	csrw	0x7cb, zero
     848: 7cb01073     	csrw	0x7cb, zero
     84c: 7cb01073     	csrw	0x7cb, zero
     850: 7cb01073     	csrw	0x7cb, zero
     854: 7cb01073     	csrw	0x7cb, zero
     858: 7cb01073     	csrw	0x7cb, zero
     85c: 7cb01073     	csrw	0x7cb, zero
     860: 7cb01073     	csrw	0x7cb, zero
     864: 7cb01073     	csrw	0x7cb, zero
     868: 7cb01073     	csrw	0x7cb, zero
     86c: 7cb01073     	csrw	0x7cb, zero
     870: 7cb01073     	csrw	0x7cb, zero
     874: 7cb01073     	csrw	0x7cb, zero
     878: 7cb01073     	csrw	0x7cb, zero
     87c: 7cb01073     	csrw	0x7cb, zero
     880: 7cb01073     	csrw	0x7cb, zero
     884: 7cb01073     	csrw	0x7cb, zero
     888: 7cb01073     	csrw	0x7cb, zero
     88c: 7cb01073     	csrw	0x7cb, zero
     890: 7cb01073     	csrw	0x7cb, zero
     894: 7cb01073     	csrw	0x7cb, zero
     898: 7cb01073     	csrw	0x7cb, zero
     89c: 7cb01073     	csrw	0x7cb, zero
     8a0: 7cb01073     	csrw	0x7cb, zero
     8a4: 7cb01073     	csrw	0x7cb, zero
     8a8: 7cb01073     	csrw	0x7cb, zero
     8ac: 7cb01073     	csrw	0x7cb, zero
     8b0: 7cb01073     	csrw	0x7cb, zero
     8b4: 7cb01073     	csrw	0x7cb, zero
     8b8: 7cb01073     	csrw	0x7cb, zero
     8bc: 7cb01073     	csrw	0x7cb, zero
     8c0: 7cb01073     	csrw	0x7cb, zero
     8c4: 7cb01073     	csrw	0x7cb, zero
     8c8: 7cb01073     	csrw	0x7cb, zero
     8cc: 7cb01073     	csrw	0x7cb, zero
     8d0: 7cb01073     	csrw	0x7cb, zero
     8d4: 7cb01073     	csrw	0x7cb, zero
     8d8: 7cb01073     	csrw	0x7cb, zero
     8dc: 7cb01073     	csrw	0x7cb, zero
     8e0: 7cb01073     	csrw	0x7cb, zero
     8e4: 7cb01073     	csrw	0x7cb, zero
     8e8: 7cb01073     	csrw	0x7cb, zero
     8ec: 7cb01073     	csrw	0x7cb, zero
     8f0: 7cb01073     	csrw	0x7cb, zero
     8f4: 7cb01073     	csrw	0x7cb, zero
     8f8: 7cb01073     	csrw	0x7cb, zero
     8fc: 7cb01073     	csrw	0x7cb, zero
     900: 7cb01073     	csrw	0x7cb, zero
     904: 7cb01073     	csrw	0x7cb, zero
     908: 7cb01073     	csrw	0x7cb, zero
     90c: 7cb01073     	csrw	0x7cb, zero
     910: 7cb01073     	csrw	0x7cb, zero
     914: 7cb01073     	csrw	0x7cb, zero
     918: 7cb01073     	csrw	0x7cb, zero
     91c: 7cb01073     	csrw	0x7cb, zero
     920: 7cb01073     	csrw	0x7cb, zero
     924: 7cb01073     	csrw	0x7cb, zero
     928: 7cb01073     	csrw	0x7cb, zero
     92c: 7cb01073     	csrw	0x7cb, zero
     930: 7cb01073     	csrw	0x7cb, zero
     934: 7cb01073     	csrw	0x7cb, zero
     938: 7cb01073     	csrw	0x7cb, zero
     93c: 7cb01073     	csrw	0x7cb, zero
     940: 7cb01073     	csrw	0x7cb, zero
     944: 7cb01073     	csrw	0x7cb, zero
     948: 7cb01073     	csrw	0x7cb, zero
     94c: 7cb01073     	csrw	0x7cb, zero
     950: 7cb01073     	csrw	0x7cb, zero
     954: 7cb01073     	csrw	0x7cb, zero
     958: 7cb01073     	csrw	0x7cb, zero
     95c: 7cb01073     	csrw	0x7cb, zero
     960: 7cb01073     	csrw	0x7cb, zero
     964: 7cb01073     	csrw	0x7cb, zero
     968: 7cb01073     	csrw	0x7cb, zero
     96c: 7cb01073     	csrw	0x7cb, zero
     970: 7cb01073     	csrw	0x7cb, zero
     974: 7cb01073     	csrw	0x7cb, zero
     978: 7cb01073     	csrw	0x7cb, zero
     97c: 7cb01073     	csrw	0x7cb, zero
     980: 7cb01073     	csrw	0x7cb, zero
     984: 7cb01073     	csrw	0x7cb, zero
     988: 7cb01073     	csrw	0x7cb, zero
     98c: 7cb01073     	csrw	0x7cb, zero
     990: 7cb01073     	csrw	0x7cb, zero
     994: 7cb01073     	csrw	0x7cb, zero
     998: 7cb01073     	csrw	0x7cb, zero
     99c: 7cb01073     	csrw	0x7cb, zero
     9a0: 7cb01073     	csrw	0x7cb, zero
     9a4: 7cb01073     	csrw	0x7cb, zero
     9a8: 7cb01073     	csrw	0x7cb, zero
     9ac: 7cb01073     	csrw	0x7cb, zero
     9b0: 7cb01073     	csrw	0x7cb, zero
     9b4: 7cb01073     	csrw	0x7cb, zero
     9b8: 7cb01073     	csrw	0x7cb, zero
     9bc: 7cb01073     	csrw	0x7cb, zero
     9c0: 7cb01073     	csrw	0x7cb, zero
     9c4: 7cb01073     	csrw	0x7cb, zero
     9c8: 7cb01073     	csrw	0x7cb, zero
     9cc: 7cb01073     	csrw	0x7cb, zero
     9d0: 7cb01073     	csrw	0x7cb, zero
     9d4: 7cb01073     	csrw	0x7cb, zero
     9d8: 7cb01073     	csrw	0x7cb, zero
     9dc: 7cb01073     	csrw	0x7cb, zero
     9e0: 7cb01073     	csrw	0x7cb, zero
     9e4: 7cb01073     	csrw	0x7cb, zero
     9e8: 7cb01073     	csrw	0x7cb, zero
     9ec: 7cb01073     	csrw	0x7cb, zero
     9f0: 7cb01073     	csrw	0x7cb, zero
     9f4: 7cb01073     	csrw	0x7cb, zero
     9f8: 7cb01073     	csrw	0x7cb, zero
     9fc: 7cb01073     	csrw	0x7cb, zero
     a00: 7cb01073     	csrw	0x7cb, zero
     a04: 7cb01073     	csrw	0x7cb, zero
     a08: 7cb01073     	csrw	0x7cb, zero
     a0c: 7cb01073     	csrw	0x7cb, zero
     a10: 7cb01073     	csrw	0x7cb, zero
     a14: 7cb01073     	csrw	0x7cb, zero
     a18: 7cb01073     	csrw	0x7cb, zero
     a1c: 7cb01073     	csrw	0x7cb, zero
     a20: 7cb01073     	csrw	0x7cb, zero
     a24: 7cb01073     	csrw	0x7cb, zero
     a28: 7cb01073     	csrw	0x7cb, zero
     a2c: 7cb01073     	csrw	0x7cb, zero
     a30: 7cb01073     	csrw	0x7cb, zero
     a34: 7cb01073     	csrw	0x7cb, zero
     a38: 7cb01073     	csrw	0x7cb, zero
     a3c: 7cb01073     	csrw	0x7cb, zero
     a40: 7cb01073     	csrw	0x7cb, zero
     a44: 7cb01073     	csrw	0x7cb, zero
     a48: 7cb01073     	csrw	0x7cb, zero
     a4c: 7cb01073     	csrw	0x7cb, zero
     a50: 7cb01073     	csrw	0x7cb, zero
     a54: 7cb01073     	csrw	0x7cb, zero
     a58: 7cb01073     	csrw	0x7cb, zero
     a5c: 7cb01073     	csrw	0x7cb, zero
     a60: 7cb01073     	csrw	0x7cb, zero
     a64: 7cb01073     	csrw	0x7cb, zero
     a68: 7cb01073     	csrw	0x7cb, zero
     a6c: 7cb01073     	csrw	0x7cb, zero
     a70: 7cb01073     	csrw	0x7cb, zero
     a74: 7cb01073     	csrw	0x7cb, zero
     a78: 7cb01073     	csrw	0x7cb, zero
     a7c: 7cb01073     	csrw	0x7cb, zero
     a80: 7cb01073     	csrw	0x7cb, zero
     a84: 7cb01073     	csrw	0x7cb, zero
     a88: 7cb01073     	csrw	0x7cb, zero
     a8c: 7cb01073     	csrw	0x7cb, zero
     a90: 7cb01073     	csrw	0x7cb, zero
     a94: 7cb01073     	csrw	0x7cb, zero
     a98: 7cb01073     	csrw	0x7cb, zero
     a9c: 7cb01073     	csrw	0x7cb, zero
     aa0: 7cb01073     	csrw	0x7cb, zero
     aa4: 7cb01073     	csrw	0x7cb, zero
     aa8: 7cb01073     	csrw	0x7cb, zero
     aac: 7cb01073     	csrw	0x7cb, zero
     ab0: 7cb01073     	csrw	0x7cb, zero
     ab4: 7cb01073     	csrw	0x7cb, zero
     ab8: 7cb01073     	csrw	0x7cb, zero
     abc: 7cb01073     	csrw	0x7cb, zero
     ac0: 7cb01073     	csrw	0x7cb, zero
     ac4: 7cb01073     	csrw	0x7cb, zero
     ac8: 7cb01073     	csrw	0x7cb, zero
     acc: 7cb01073     	csrw	0x7cb, zero
     ad0: 7cb01073     	csrw	0x7cb, zero
     ad4: 7cb01073     	csrw	0x7cb, zero
     ad8: 7cb01073     	csrw	0x7cb, zero
     adc: 7cb01073     	csrw	0x7cb, zero
     ae0: 7cb01073     	csrw	0x7cb, zero
     ae4: 7cb01073     	csrw	0x7cb, zero
     ae8: 7cb01073     	csrw	0x7cb, zero
     aec: 7cb01073     	csrw	0x7cb, zero
     af0: 7cb01073     	csrw	0x7cb, zero
     af4: 7cb01073     	csrw	0x7cb, zero
     af8: 7cb01073     	csrw	0x7cb, zero
     afc: 7cb01073     	csrw	0x7cb, zero
     b00: 7cb01073     	csrw	0x7cb, zero
     b04: 7cb01073     	csrw	0x7cb, zero
     b08: 7cb01073     	csrw	0x7cb, zero
     b0c: 7cb01073     	csrw	0x7cb, zero
     b10: 7cb01073     	csrw	0x7cb, zero
     b14: 7cb01073     	csrw	0x7cb, zero
     b18: 7cb01073     	csrw	0x7cb, zero
     b1c: 7cb01073     	csrw	0x7cb, zero
     b20: 7cb01073     	csrw	0x7cb, zero
     b24: 7cb01073     	csrw	0x7cb, zero
     b28: 7cb01073     	csrw	0x7cb, zero
     b2c: 7cb01073     	csrw	0x7cb, zero
     b30: 7cb01073     	csrw	0x7cb, zero
     b34: 7cb01073     	csrw	0x7cb, zero
     b38: 7cb01073     	csrw	0x7cb, zero
     b3c: 7cb01073     	csrw	0x7cb, zero
     b40: 7cb01073     	csrw	0x7cb, zero
     b44: 7cb01073     	csrw	0x7cb, zero
     b48: 7cb01073     	csrw	0x7cb, zero
     b4c: 7cb01073     	csrw	0x7cb, zero
     b50: 7cb01073     	csrw	0x7cb, zero
     b54: 7cb01073     	csrw	0x7cb, zero
     b58: 7cb01073     	csrw	0x7cb, zero
     b5c: 7cb01073     	csrw	0x7cb, zero
     b60: 7cb01073     	csrw	0x7cb, zero
     b64: 7cb01073     	csrw	0x7cb, zero
     b68: 7cb01073     	csrw	0x7cb, zero
     b6c: 7cb01073     	csrw	0x7cb, zero
     b70: 7cb01073     	csrw	0x7cb, zero
     b74: 7cb01073     	csrw	0x7cb, zero
     b78: 7cb01073     	csrw	0x7cb, zero
     b7c: 7cb01073     	csrw	0x7cb, zero
     b80: 7cb01073     	csrw	0x7cb, zero
     b84: 7cb01073     	csrw	0x7cb, zero
     b88: 7cb01073     	csrw	0x7cb, zero
     b8c: 7cb01073     	csrw	0x7cb, zero
     b90: 7cb01073     	csrw	0x7cb, zero
     b94: 7cb01073     	csrw	0x7cb, zero
     b98: 7cb01073     	csrw	0x7cb, zero
     b9c: 7cb01073     	csrw	0x7cb, zero
     ba0: 7cb01073     	csrw	0x7cb, zero
     ba4: 7cb01073     	csrw	0x7cb, zero
     ba8: 7cb01073     	csrw	0x7cb, zero
     bac: 7cb01073     	csrw	0x7cb, zero
     bb0: 7cb01073     	csrw	0x7cb, zero
     bb4: 7cb01073     	csrw	0x7cb, zero
     bb8: 7cb01073     	csrw	0x7cb, zero
     bbc: 7cb01073     	csrw	0x7cb, zero
     bc0: 7cb01073     	csrw	0x7cb, zero
     bc4: 7cb01073     	csrw	0x7cb, zero
     bc8: 7cb01073     	csrw	0x7cb, zero
     bcc: 7cb01073     	csrw	0x7cb, zero
     bd0: 7cb01073     	csrw	0x7cb, zero
     bd4: 7cb01073     	csrw	0x7cb, zero
     bd8: 7cb01073     	csrw	0x7cb, zero
     bdc: 7cb01073     	csrw	0x7cb, zero
     be0: 7cb01073     	csrw	0x7cb, zero
     be4: 7cb01073     	csrw	0x7cb, zero
     be8: 7cb01073     	csrw	0x7cb, zero
     bec: 7cb01073     	csrw	0x7cb, zero
     bf0: 7cb01073     	csrw	0x7cb, zero
     bf4: 7cb01073     	csrw	0x7cb, zero
     bf8: 7cb01073     	csrw	0x7cb, zero
     bfc: 7cb01073     	csrw	0x7cb, zero
     c00: 7cb01073     	csrw	0x7cb, zero
     c04: 7cb01073     	csrw	0x7cb, zero
     c08: 7cb01073     	csrw	0x7cb, zero
     c0c: 7cb01073     	csrw	0x7cb, zero
     c10: 7cb01073     	csrw	0x7cb, zero
     c14: 7cb01073     	csrw	0x7cb, zero
     c18: 7cb01073     	csrw	0x7cb, zero
     c1c: 7cb01073     	csrw	0x7cb, zero
     c20: 7cb01073     	csrw	0x7cb, zero
     c24: 7cb01073     	csrw	0x7cb, zero
     c28: 7cb01073     	csrw	0x7cb, zero
     c2c: 7cb01073     	csrw	0x7cb, zero
     c30: 7cb01073     	csrw	0x7cb, zero
     c34: 7cb01073     	csrw	0x7cb, zero
     c38: 7cb01073     	csrw	0x7cb, zero
     c3c: 7cb01073     	csrw	0x7cb, zero
     c40: 7cb01073     	csrw	0x7cb, zero
     c44: 7cb01073     	csrw	0x7cb, zero
     c48: 7cb01073     	csrw	0x7cb, zero
     c4c: 7cb01073     	csrw	0x7cb, zero
     c50: 7cb01073     	csrw	0x7cb, zero
     c54: 7cb01073     	csrw	0x7cb, zero
     c58: 7cb01073     	csrw	0x7cb, zero
     c5c: 7cb01073     	csrw	0x7cb, zero
     c60: 7cb01073     	csrw	0x7cb, zero
     c64: 7cb01073     	csrw	0x7cb, zero
     c68: 7cb01073     	csrw	0x7cb, zero
     c6c: 7cb01073     	csrw	0x7cb, zero
     c70: 7cb01073     	csrw	0x7cb, zero
     c74: 7cb01073     	csrw	0x7cb, zero
     c78: 7cb01073     	csrw	0x7cb, zero
     c7c: 7cb01073     	csrw	0x7cb, zero
     c80: 7cb01073     	csrw	0x7cb, zero
     c84: 7cb01073     	csrw	0x7cb, zero
     c88: 7cb01073     	csrw	0x7cb, zero
     c8c: 7cb01073     	csrw	0x7cb, zero
     c90: 7cb01073     	csrw	0x7cb, zero
     c94: 7cb01073     	csrw	0x7cb, zero
     c98: 7cb01073     	csrw	0x7cb, zero
     c9c: 7cb01073     	csrw	0x7cb, zero
     ca0: 7cb01073     	csrw	0x7cb, zero
     ca4: 7cb01073     	csrw	0x7cb, zero
     ca8: 7cb01073     	csrw	0x7cb, zero
     cac: 7cb01073     	csrw	0x7cb, zero
     cb0: 7cb01073     	csrw	0x7cb, zero
     cb4: 7cb01073     	csrw	0x7cb, zero
     cb8: 7cb01073     	csrw	0x7cb, zero
     cbc: 7cb01073     	csrw	0x7cb, zero
     cc0: 7cb01073     	csrw	0x7cb, zero
     cc4: 7cb01073     	csrw	0x7cb, zero
     cc8: 7cb01073     	csrw	0x7cb, zero
     ccc: 7cb01073     	csrw	0x7cb, zero
     cd0: 7cb01073     	csrw	0x7cb, zero
     cd4: 7cb01073     	csrw	0x7cb, zero
     cd8: 7cb01073     	csrw	0x7cb, zero
     cdc: 7cb01073     	csrw	0x7cb, zero
     ce0: 7cb01073     	csrw	0x7cb, zero
     ce4: 7cb01073     	csrw	0x7cb, zero
     ce8: 7cb01073     	csrw	0x7cb, zero
     cec: 7cb01073     	csrw	0x7cb, zero
     cf0: 7cb01073     	csrw	0x7cb, zero
     cf4: 7cb01073     	csrw	0x7cb, zero
     cf8: 7cb01073     	csrw	0x7cb, zero
     cfc: 7cb01073     	csrw	0x7cb, zero
     d00: 7cb01073     	csrw	0x7cb, zero
     d04: 7cb01073     	csrw	0x7cb, zero
     d08: 7cb01073     	csrw	0x7cb, zero
     d0c: 7cb01073     	csrw	0x7cb, zero
     d10: 7cb01073     	csrw	0x7cb, zero
     d14: 7cb01073     	csrw	0x7cb, zero
     d18: 7cb01073     	csrw	0x7cb, zero
     d1c: 7cb01073     	csrw	0x7cb, zero
     d20: 7cb01073     	csrw	0x7cb, zero
     d24: 7cb01073     	csrw	0x7cb, zero
     d28: 7cb01073     	csrw	0x7cb, zero
     d2c: 7cb01073     	csrw	0x7cb, zero
     d30: 7cb01073     	csrw	0x7cb, zero
     d34: 7cb01073     	csrw	0x7cb, zero
     d38: 7cb01073     	csrw	0x7cb, zero
     d3c: 7cb01073     	csrw	0x7cb, zero
     d40: 7cb01073     	csrw	0x7cb, zero
     d44: 7cb01073     	csrw	0x7cb, zero
     d48: 7cb01073     	csrw	0x7cb, zero
     d4c: 7cb01073     	csrw	0x7cb, zero
     d50: 7cb01073     	csrw	0x7cb, zero
     d54: 7cb01073     	csrw	0x7cb, zero
     d58: 7cb01073     	csrw	0x7cb, zero
     d5c: 7cb01073     	csrw	0x7cb, zero
     d60: 7cb01073     	csrw	0x7cb, zero
     d64: 7cb01073     	csrw	0x7cb, zero
     d68: 7cb01073     	csrw	0x7cb, zero
     d6c: 7cb01073     	csrw	0x7cb, zero
     d70: 7cb01073     	csrw	0x7cb, zero
     d74: 7cb01073     	csrw	0x7cb, zero
     d78: 7cb01073     	csrw	0x7cb, zero
     d7c: 7cb01073     	csrw	0x7cb, zero
     d80: 7cb01073     	csrw	0x7cb, zero
     d84: 7cb01073     	csrw	0x7cb, zero
     d88: 7cb01073     	csrw	0x7cb, zero
     d8c: 7cb01073     	csrw	0x7cb, zero
     d90: 7cb01073     	csrw	0x7cb, zero
     d94: 7cb01073     	csrw	0x7cb, zero
     d98: 7cb01073     	csrw	0x7cb, zero
     d9c: 7cb01073     	csrw	0x7cb, zero
     da0: 7cb01073     	csrw	0x7cb, zero
     da4: 7cb01073     	csrw	0x7cb, zero
     da8: 7cb01073     	csrw	0x7cb, zero
     dac: 7cb01073     	csrw	0x7cb, zero
     db0: 7cb01073     	csrw	0x7cb, zero
     db4: 7cb01073     	csrw	0x7cb, zero
     db8: 7cb01073     	csrw	0x7cb, zero
     dbc: 7cb01073     	csrw	0x7cb, zero
     dc0: 7cb01073     	csrw	0x7cb, zero
     dc4: 7cb01073     	csrw	0x7cb, zero
     dc8: 7cb01073     	csrw	0x7cb, zero
     dcc: 7cb01073     	csrw	0x7cb, zero
     dd0: 7cb01073     	csrw	0x7cb, zero
     dd4: 7cb01073     	csrw	0x7cb, zero
     dd8: 7cb01073     	csrw	0x7cb, zero
     ddc: 7cb01073     	csrw	0x7cb, zero
     de0: 7cb01073     	csrw	0x7cb, zero
     de4: 7cb01073     	csrw	0x7cb, zero
     de8: 7cb01073     	csrw	0x7cb, zero
     dec: 7cb01073     	csrw	0x7cb, zero
     df0: 7cb01073     	csrw	0x7cb, zero
     df4: 7cb01073     	csrw	0x7cb, zero
     df8: 7cb01073     	csrw	0x7cb, zero
     dfc: 7cb01073     	csrw	0x7cb, zero
     e00: 7cb01073     	csrw	0x7cb, zero
     e04: 7cb01073     	csrw	0x7cb, zero
     e08: 7cb01073     	csrw	0x7cb, zero
     e0c: 7cb01073     	csrw	0x7cb, zero
     e10: 7cb01073     	csrw	0x7cb, zero
     e14: 7cb01073     	csrw	0x7cb, zero
     e18: 7cb01073     	csrw	0x7cb, zero
     e1c: 7cb01073     	csrw	0x7cb, zero
     e20: 7cb01073     	csrw	0x7cb, zero
     e24: 7cb01073     	csrw	0x7cb, zero
     e28: 7cb01073     	csrw	0x7cb, zero
     e2c: 7cb01073     	csrw	0x7cb, zero
     e30: 7cb01073     	csrw	0x7cb, zero
     e34: 7cb01073     	csrw	0x7cb, zero
     e38: 7cb01073     	csrw	0x7cb, zero
     e3c: 7cb01073     	csrw	0x7cb, zero
     e40: 7cb01073     	csrw	0x7cb, zero
     e44: 7cb01073     	csrw	0x7cb, zero
     e48: 7cb01073     	csrw	0x7cb, zero
     e4c: 7cb01073     	csrw	0x7cb, zero
     e50: 7cb01073     	csrw	0x7cb, zero
     e54: 7cb01073     	csrw	0x7cb, zero
     e58: 7cb01073     	csrw	0x7cb, zero
     e5c: 7cb01073     	csrw	0x7cb, zero
     e60: 7cb01073     	csrw	0x7cb, zero
     e64: 7cb01073     	csrw	0x7cb, zero
     e68: 7cb01073     	csrw	0x7cb, zero
     e6c: 7cb01073     	csrw	0x7cb, zero
     e70: 7cb01073     	csrw	0x7cb, zero
     e74: 7cb01073     	csrw	0x7cb, zero
     e78: 7cb01073     	csrw	0x7cb, zero
     e7c: 7cb01073     	csrw	0x7cb, zero
     e80: 7cb01073     	csrw	0x7cb, zero
     e84: 7cb01073     	csrw	0x7cb, zero
     e88: 7cb01073     	csrw	0x7cb, zero
     e8c: 7cb01073     	csrw	0x7cb, zero
     e90: 7cb01073     	csrw	0x7cb, zero
     e94: 7cb01073     	csrw	0x7cb, zero
     e98: 7cb01073     	csrw	0x7cb, zero
     e9c: 7cb01073     	csrw	0x7cb, zero
     ea0: 7cb01073     	csrw	0x7cb, zero
     ea4: 7cb01073     	csrw	0x7cb, zero
     ea8: 7cb01073     	csrw	0x7cb, zero
     eac: 7cb01073     	csrw	0x7cb, zero
     eb0: 7cb01073     	csrw	0x7cb, zero
     eb4: 7cb01073     	csrw	0x7cb, zero
     eb8: 7cb01073     	csrw	0x7cb, zero
     ebc: 7cb01073     	csrw	0x7cb, zero
     ec0: 7cb01073     	csrw	0x7cb, zero
     ec4: 7cb01073     	csrw	0x7cb, zero
     ec8: 7cb01073     	csrw	0x7cb, zero
     ecc: 7cb01073     	csrw	0x7cb, zero
     ed0: 7cb01073     	csrw	0x7cb, zero
     ed4: 7cb01073     	csrw	0x7cb, zero
     ed8: 7cb01073     	csrw	0x7cb, zero
     edc: 7cb01073     	csrw	0x7cb, zero
     ee0: 7cb01073     	csrw	0x7cb, zero
     ee4: 7cb01073     	csrw	0x7cb, zero
     ee8: 7cb01073     	csrw	0x7cb, zero
     eec: 7cb01073     	csrw	0x7cb, zero
     ef0: 7cb01073     	csrw	0x7cb, zero
     ef4: 7cb01073     	csrw	0x7cb, zero
     ef8: 7cb01073     	csrw	0x7cb, zero
     efc: 7cb01073     	csrw	0x7cb, zero
     f00: 7cb01073     	csrw	0x7cb, zero
     f04: 7cb01073     	csrw	0x7cb, zero
     f08: 7cb01073     	csrw	0x7cb, zero
     f0c: 7cb01073     	csrw	0x7cb, zero
     f10: 7cb01073     	csrw	0x7cb, zero
     f14: 7cb01073     	csrw	0x7cb, zero
     f18: 7cb01073     	csrw	0x7cb, zero
     f1c: 7cb01073     	csrw	0x7cb, zero
     f20: 7cb01073     	csrw	0x7cb, zero
     f24: 7cb01073     	csrw	0x7cb, zero
     f28: 7cb01073     	csrw	0x7cb, zero
     f2c: 7cb01073     	csrw	0x7cb, zero
     f30: 7cb01073     	csrw	0x7cb, zero
     f34: 7cb01073     	csrw	0x7cb, zero
     f38: 7cb01073     	csrw	0x7cb, zero
     f3c: 7cb01073     	csrw	0x7cb, zero
     f40: 7cb01073     	csrw	0x7cb, zero
     f44: 7cb01073     	csrw	0x7cb, zero
     f48: 7cb01073     	csrw	0x7cb, zero
     f4c: 7cb01073     	csrw	0x7cb, zero
     f50: 7cb01073     	csrw	0x7cb, zero
     f54: 7cb01073     	csrw	0x7cb, zero
     f58: 7cb01073     	csrw	0x7cb, zero
     f5c: 7cb01073     	csrw	0x7cb, zero
     f60: 7cb01073     	csrw	0x7cb, zero
     f64: 7cb01073     	csrw	0x7cb, zero
     f68: 7cb01073     	csrw	0x7cb, zero
     f6c: 7cb01073     	csrw	0x7cb, zero
     f70: 7cb01073     	csrw	0x7cb, zero
     f74: 7cb01073     	csrw	0x7cb, zero
     f78: 7cb01073     	csrw	0x7cb, zero
     f7c: 7cb01073     	csrw	0x7cb, zero
     f80: 7cb01073     	csrw	0x7cb, zero
     f84: 7cb01073     	csrw	0x7cb, zero
     f88: 7cb01073     	csrw	0x7cb, zero
     f8c: 7cb01073     	csrw	0x7cb, zero
     f90: 7cb01073     	csrw	0x7cb, zero
     f94: 7cb01073     	csrw	0x7cb, zero
     f98: 7cb01073     	csrw	0x7cb, zero
     f9c: 7cb01073     	csrw	0x7cb, zero
     fa0: 7cb01073     	csrw	0x7cb, zero
     fa4: 7cb01073     	csrw	0x7cb, zero
     fa8: 7cb01073     	csrw	0x7cb, zero
     fac: 7cb01073     	csrw	0x7cb, zero
     fb0: 7cb01073     	csrw	0x7cb, zero
     fb4: 7cb01073     	csrw	0x7cb, zero
     fb8: 7cb01073     	csrw	0x7cb, zero
     fbc: 7cb01073     	csrw	0x7cb, zero
     fc0: 7cb01073     	csrw	0x7cb, zero
     fc4: 7cb01073     	csrw	0x7cb, zero
     fc8: 7cb01073     	csrw	0x7cb, zero
     fcc: 7cb01073     	csrw	0x7cb, zero
     fd0: 7cb01073     	csrw	0x7cb, zero
     fd4: 7cb01073     	csrw	0x7cb, zero
     fd8: 7cb01073     	csrw	0x7cb, zero
     fdc: 7cb01073     	csrw	0x7cb, zero
     fe0: 7cb01073     	csrw	0x7cb, zero
     fe4: 7cb01073     	csrw	0x7cb, zero
     fe8: 7cb01073     	csrw	0x7cb, zero
     fec: 7cb01073     	csrw	0x7cb, zero
     ff0: 7cb01073     	csrw	0x7cb, zero
     ff4: 7cb01073     	csrw	0x7cb, zero
     ff8: 7cb01073     	csrw	0x7cb, zero
     ffc: 7cb01073     	csrw	0x7cb, zero
    1000: 7cb01073     	csrw	0x7cb, zero
    1004: 7cb01073     	csrw	0x7cb, zero
    1008: 7cb01073     	csrw	0x7cb, zero
    100c: 7cb01073     	csrw	0x7cb, zero
    1010: 7cb01073     	csrw	0x7cb, zero
    1014: 7cb01073     	csrw	0x7cb, zero
    1018: 7cb01073     	csrw	0x7cb, zero
    101c: 7cb01073     	csrw	0x7cb, zero
    1020: 7cb01073     	csrw	0x7cb, zero
    1024: 7cb01073     	csrw	0x7cb, zero
    1028: 7cb01073     	csrw	0x7cb, zero
    102c: 7cb01073     	csrw	0x7cb, zero
    1030: 7cb01073     	csrw	0x7cb, zero
    1034: 7cb01073     	csrw	0x7cb, zero
    1038: 7cb01073     	csrw	0x7cb, zero
    103c: 7cb01073     	csrw	0x7cb, zero
    1040: 7cb01073     	csrw	0x7cb, zero
    1044: 7cb01073     	csrw	0x7cb, zero
    1048: 7cb01073     	csrw	0x7cb, zero
    104c: 7cb01073     	csrw	0x7cb, zero
    1050: 7cb01073     	csrw	0x7cb, zero
    1054: 7cb01073     	csrw	0x7cb, zero
    1058: 7cb01073     	csrw	0x7cb, zero
    105c: 7cb01073     	csrw	0x7cb, zero
    1060: 7cb01073     	csrw	0x7cb, zero
    1064: 7cb01073     	csrw	0x7cb, zero
    1068: 7cb01073     	csrw	0x7cb, zero
    106c: 7cb01073     	csrw	0x7cb, zero
    1070: 7cb01073     	csrw	0x7cb, zero
    1074: 7cb01073     	csrw	0x7cb, zero
    1078: 7cb01073     	csrw	0x7cb, zero
    107c: 7cb01073     	csrw	0x7cb, zero
    1080: 7cb01073     	csrw	0x7cb, zero
    1084: 7cb01073     	csrw	0x7cb, zero
    1088: 7cb01073     	csrw	0x7cb, zero
    108c: 7cb01073     	csrw	0x7cb, zero
    1090: 7cb01073     	csrw	0x7cb, zero
    1094: 7cb01073     	csrw	0x7cb, zero
    1098: 7cb01073     	csrw	0x7cb, zero
    109c: 7cb01073     	csrw	0x7cb, zero
    10a0: 7cb01073     	csrw	0x7cb, zero
    10a4: 7cb01073     	csrw	0x7cb, zero
    10a8: 7cb01073     	csrw	0x7cb, zero
    10ac: 7cb01073     	csrw	0x7cb, zero
    10b0: 7cb01073     	csrw	0x7cb, zero
    10b4: 7cb01073     	csrw	0x7cb, zero
    10b8: 7cb01073     	csrw	0x7cb, zero
    10bc: 7cb01073     	csrw	0x7cb, zero
    10c0: 7cb01073     	csrw	0x7cb, zero
    10c4: 7cb01073     	csrw	0x7cb, zero
    10c8: 7cb01073     	csrw	0x7cb, zero
    10cc: 7cb01073     	csrw	0x7cb, zero
    10d0: 7cb01073     	csrw	0x7cb, zero
    10d4: 7cb01073     	csrw	0x7cb, zero
    10d8: 7cb01073     	csrw	0x7cb, zero
    10dc: 7cb01073     	csrw	0x7cb, zero
    10e0: 7cb01073     	csrw	0x7cb, zero
    10e4: 7cb01073     	csrw	0x7cb, zero
    10e8: 7cb01073     	csrw	0x7cb, zero
    10ec: 7cb01073     	csrw	0x7cb, zero
    10f0: 7cb01073     	csrw	0x7cb, zero
    10f4: 7cb01073     	csrw	0x7cb, zero
    10f8: 7cb01073     	csrw	0x7cb, zero
    10fc: 7cb01073     	csrw	0x7cb, zero
    1100: 7cb01073     	csrw	0x7cb, zero
    1104: 7cb01073     	csrw	0x7cb, zero
    1108: 7cb01073     	csrw	0x7cb, zero
    110c: 7cb01073     	csrw	0x7cb, zero
    1110: 7cb01073     	csrw	0x7cb, zero
    1114: 7cb01073     	csrw	0x7cb, zero
    1118: 7cb01073     	csrw	0x7cb, zero
    111c: 7cb01073     	csrw	0x7cb, zero
    1120: 7cb01073     	csrw	0x7cb, zero
    1124: 7cb01073     	csrw	0x7cb, zero
    1128: 7cb01073     	csrw	0x7cb, zero
    112c: 7cb01073     	csrw	0x7cb, zero
    1130: 7cb01073     	csrw	0x7cb, zero
    1134: 7cb01073     	csrw	0x7cb, zero
    1138: 7cb01073     	csrw	0x7cb, zero
    113c: 7cb01073     	csrw	0x7cb, zero
    1140: 7cb01073     	csrw	0x7cb, zero
    1144: 7cb01073     	csrw	0x7cb, zero
    1148: 7cb01073     	csrw	0x7cb, zero
    114c: 7cb01073     	csrw	0x7cb, zero
    1150: 7cb01073     	csrw	0x7cb, zero
    1154: 7cb01073     	csrw	0x7cb, zero
    1158: 7cb01073     	csrw	0x7cb, zero
    115c: 7cb01073     	csrw	0x7cb, zero
    1160: 7cb01073     	csrw	0x7cb, zero
    1164: 7cb01073     	csrw	0x7cb, zero
    1168: 7cb01073     	csrw	0x7cb, zero
    116c: 00c12083     	lw	ra, 0xc(sp)
    1170: 00812403     	lw	s0, 0x8(sp)
    1174: 01010113     	addi	sp, sp, 0x10
    1178: 00008067     	ret

0000117c <core::panicking::panic::ha1ed58f4f5473d93>:
    117c: fd010113     	addi	sp, sp, -0x30
    1180: 02112623     	sw	ra, 0x2c(sp)
    1184: 02812423     	sw	s0, 0x28(sp)
    1188: 03010413     	addi	s0, sp, 0x30
    118c: fea42823     	sw	a0, -0x10(s0)
    1190: feb42a23     	sw	a1, -0xc(s0)
    1194: ff040513     	addi	a0, s0, -0x10
    1198: 00100593     	li	a1, 0x1
    119c: fe042423     	sw	zero, -0x18(s0)
    11a0: 00400693     	li	a3, 0x4
    11a4: fca42c23     	sw	a0, -0x28(s0)
    11a8: fcb42e23     	sw	a1, -0x24(s0)
    11ac: fed42023     	sw	a3, -0x20(s0)
    11b0: fe042223     	sw	zero, -0x1c(s0)
    11b4: fd840513     	addi	a0, s0, -0x28
    11b8: 00060593     	mv	a1, a2
    11bc: 00000097     	auipc	ra, 0x0
    11c0: 008080e7     	jalr	0x8(ra) <core::panicking::panic_fmt::h224b92ba1adb8ba8>

000011c4 <core::panicking::panic_fmt::h224b92ba1adb8ba8>:
    11c4: fe010113     	addi	sp, sp, -0x20
    11c8: 00112e23     	sw	ra, 0x1c(sp)
    11cc: 00812c23     	sw	s0, 0x18(sp)
    11d0: 02010413     	addi	s0, sp, 0x20
    11d4: 00100613     	li	a2, 0x1
    11d8: fea42623     	sw	a0, -0x14(s0)
    11dc: feb42823     	sw	a1, -0x10(s0)
    11e0: fec41a23     	sh	a2, -0xc(s0)
    11e4: fec40513     	addi	a0, s0, -0x14
    11e8: fffff097     	auipc	ra, 0xfffff
    11ec: ea4080e7     	jalr	-0x15c(ra) <_RNvCs6Gf8pSYpf6Z_7___rustc17rust_begin_unwind>

000011f0 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a>:
    11f0: ff010113     	addi	sp, sp, -0x10
    11f4: 01000693     	li	a3, 0x10
    11f8: 08d66063     	bltu	a2, a3, 0x1278 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x88>
    11fc: 40a006b3     	neg	a3, a0
    1200: 0036f693     	andi	a3, a3, 0x3
    1204: 00d507b3     	add	a5, a0, a3
    1208: 02f57463     	bgeu	a0, a5, 0x1230 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x40>
    120c: 00068713     	mv	a4, a3
    1210: 00050813     	mv	a6, a0
    1214: 00058893     	mv	a7, a1
    1218: 0008c283     	lbu	t0, 0x0(a7)
    121c: fff70713     	addi	a4, a4, -0x1
    1220: 00580023     	sb	t0, 0x0(a6)
    1224: 00180813     	addi	a6, a6, 0x1
    1228: 00188893     	addi	a7, a7, 0x1
    122c: fe0716e3     	bnez	a4, 0x1218 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x28>
    1230: 00d585b3     	add	a1, a1, a3
    1234: 40d60633     	sub	a2, a2, a3
    1238: ffc67713     	andi	a4, a2, -0x4
    123c: 0035f893     	andi	a7, a1, 0x3
    1240: 00e786b3     	add	a3, a5, a4
    1244: 06089063     	bnez	a7, 0x12a4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xb4>
    1248: 00d7fe63     	bgeu	a5, a3, 0x1264 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x74>
    124c: 00058813     	mv	a6, a1
    1250: 00082883     	lw	a7, 0x0(a6)
    1254: 0117a023     	sw	a7, 0x0(a5)
    1258: 00478793     	addi	a5, a5, 0x4
    125c: 00480813     	addi	a6, a6, 0x4
    1260: fed7e8e3     	bltu	a5, a3, 0x1250 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x60>
    1264: 00e585b3     	add	a1, a1, a4
    1268: 00367613     	andi	a2, a2, 0x3
    126c: 00c68733     	add	a4, a3, a2
    1270: 00e6ea63     	bltu	a3, a4, 0x1284 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    1274: 0280006f     	j	0x129c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>
    1278: 00050693     	mv	a3, a0
    127c: 00c50733     	add	a4, a0, a2
    1280: 00e57e63     	bgeu	a0, a4, 0x129c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>
    1284: 0005c703     	lbu	a4, 0x0(a1)
    1288: fff60613     	addi	a2, a2, -0x1
    128c: 00e68023     	sb	a4, 0x0(a3)
    1290: 00168693     	addi	a3, a3, 0x1
    1294: 00158593     	addi	a1, a1, 0x1
    1298: fe0616e3     	bnez	a2, 0x1284 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    129c: 01010113     	addi	sp, sp, 0x10
    12a0: 00008067     	ret
    12a4: 00000813     	li	a6, 0x0
    12a8: 00400293     	li	t0, 0x4
    12ac: 00012623     	sw	zero, 0xc(sp)
    12b0: 41128333     	sub	t1, t0, a7
    12b4: 00c10293     	addi	t0, sp, 0xc
    12b8: 00137393     	andi	t2, t1, 0x1
    12bc: 0112e2b3     	or	t0, t0, a7
    12c0: 04039e63     	bnez	t2, 0x131c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x12c>
    12c4: 00237313     	andi	t1, t1, 0x2
    12c8: 06031463     	bnez	t1, 0x1330 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x140>
    12cc: 00c12e83     	lw	t4, 0xc(sp)
    12d0: 00389813     	slli	a6, a7, 0x3
    12d4: 00478293     	addi	t0, a5, 0x4
    12d8: 41158f33     	sub	t5, a1, a7
    12dc: 06d2fc63     	bgeu	t0, a3, 0x1354 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x164>
    12e0: 410002b3     	neg	t0, a6
    12e4: 0182fe13     	andi	t3, t0, 0x18
    12e8: 004f2283     	lw	t0, 0x4(t5)
    12ec: 004f0393     	addi	t2, t5, 0x4
    12f0: 010edeb3     	srl	t4, t4, a6
    12f4: 00478313     	addi	t1, a5, 0x4
    12f8: 01c29f33     	sll	t5, t0, t3
    12fc: 01df6eb3     	or	t4, t5, t4
    1300: 00878f93     	addi	t6, a5, 0x8
    1304: 01d7a023     	sw	t4, 0x0(a5)
    1308: 00030793     	mv	a5, t1
    130c: 00038f13     	mv	t5, t2
    1310: 00028e93     	mv	t4, t0
    1314: fcdfeae3     	bltu	t6, a3, 0x12e8 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xf8>
    1318: 0480006f     	j	0x1360 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x170>
    131c: 0005c803     	lbu	a6, 0x0(a1)
    1320: 01028023     	sb	a6, 0x0(t0)
    1324: 00100813     	li	a6, 0x1
    1328: 00237313     	andi	t1, t1, 0x2
    132c: fa0300e3     	beqz	t1, 0x12cc <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xdc>
    1330: 01058333     	add	t1, a1, a6
    1334: 00031303     	lh	t1, 0x0(t1)
    1338: 01028833     	add	a6, t0, a6
    133c: 00681023     	sh	t1, 0x0(a6)
    1340: 00c12e83     	lw	t4, 0xc(sp)
    1344: 00389813     	slli	a6, a7, 0x3
    1348: 00478293     	addi	t0, a5, 0x4
    134c: 41158f33     	sub	t5, a1, a7
    1350: f8d2e8e3     	bltu	t0, a3, 0x12e0 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xf0>
    1354: 000e8293     	mv	t0, t4
    1358: 000f0393     	mv	t2, t5
    135c: 00078313     	mv	t1, a5
    1360: 00010423     	sb	zero, 0x8(sp)
    1364: 00100793     	li	a5, 0x1
    1368: 00010323     	sb	zero, 0x6(sp)
    136c: 00f89c63     	bne	a7, a5, 0x1384 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x194>
    1370: 00000893     	li	a7, 0x0
    1374: 00000793     	li	a5, 0x0
    1378: 00000e13     	li	t3, 0x0
    137c: 00810e93     	addi	t4, sp, 0x8
    1380: 01c0006f     	j	0x139c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1ac>
    1384: 0043c883     	lbu	a7, 0x4(t2)
    1388: 0053c783     	lbu	a5, 0x5(t2)
    138c: 00200e13     	li	t3, 0x2
    1390: 01110423     	sb	a7, 0x8(sp)
    1394: 00879793     	slli	a5, a5, 0x8
    1398: 00610e93     	addi	t4, sp, 0x6
    139c: 0015ff13     	andi	t5, a1, 0x1
    13a0: 000f1663     	bnez	t5, 0x13ac <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1bc>
    13a4: 00000393     	li	t2, 0x0
    13a8: 01c0006f     	j	0x13c4 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x1d4>
    13ac: 01c383b3     	add	t2, t2, t3
    13b0: 0043c883     	lbu	a7, 0x4(t2)
    13b4: 011e8023     	sb	a7, 0x0(t4)
    13b8: 00614383     	lbu	t2, 0x6(sp)
    13bc: 00814883     	lbu	a7, 0x8(sp)
    13c0: 01039393     	slli	t2, t2, 0x10
    13c4: 0113e8b3     	or	a7, t2, a7
    13c8: 0102d2b3     	srl	t0, t0, a6
    13cc: 41000833     	neg	a6, a6
    13d0: 0117e7b3     	or	a5, a5, a7
    13d4: 01887813     	andi	a6, a6, 0x18
    13d8: 010797b3     	sll	a5, a5, a6
    13dc: 0057e7b3     	or	a5, a5, t0
    13e0: 00f32023     	sw	a5, 0x0(t1)
    13e4: 00e585b3     	add	a1, a1, a4
    13e8: 00367613     	andi	a2, a2, 0x3
    13ec: 00c68733     	add	a4, a3, a2
    13f0: e8e6eae3     	bltu	a3, a4, 0x1284 <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0x94>
    13f4: ea9ff06f     	j	0x129c <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a+0xac>

000013f8 <memcmp>:
    13f8: 02060063     	beqz	a2, 0x1418 <memcmp+0x20>
    13fc: 00054683     	lbu	a3, 0x0(a0)
    1400: 0005c703     	lbu	a4, 0x0(a1)
    1404: 00e69e63     	bne	a3, a4, 0x1420 <memcmp+0x28>
    1408: fff60613     	addi	a2, a2, -0x1
    140c: 00158593     	addi	a1, a1, 0x1
    1410: 00150513     	addi	a0, a0, 0x1
    1414: fe0614e3     	bnez	a2, 0x13fc <memcmp+0x4>
    1418: 00000513     	li	a0, 0x0
    141c: 00008067     	ret
    1420: 40e68533     	sub	a0, a3, a4
    1424: 00008067     	ret

00001428 <memcpy>:
    1428: 00000317     	auipc	t1, 0x0
    142c: dc830067     	jr	-0x238(t1) <compiler_builtins::mem::memcpy::hba8dfe7939a2bb0a>

00001430 <memset>:
    1430: 01000693     	li	a3, 0x10
    1434: 08d66263     	bltu	a2, a3, 0x14b8 <memset+0x88>
    1438: 40a006b3     	neg	a3, a0
    143c: 0036f693     	andi	a3, a3, 0x3
    1440: 00d50733     	add	a4, a0, a3
    1444: 00e57e63     	bgeu	a0, a4, 0x1460 <memset+0x30>
    1448: 00068793     	mv	a5, a3
    144c: 00050813     	mv	a6, a0
    1450: 00b80023     	sb	a1, 0x0(a6)
    1454: fff78793     	addi	a5, a5, -0x1
    1458: 00180813     	addi	a6, a6, 0x1
    145c: fe079ae3     	bnez	a5, 0x1450 <memset+0x20>
    1460: 40d60633     	sub	a2, a2, a3
    1464: ffc67693     	andi	a3, a2, -0x4
    1468: 00d706b3     	add	a3, a4, a3
    146c: 02d77663     	bgeu	a4, a3, 0x1498 <memset+0x68>
    1470: 0ff5f793     	zext.b	a5, a1
    1474: 01859813     	slli	a6, a1, 0x18
    1478: 00879893     	slli	a7, a5, 0x8
    147c: 0117e8b3     	or	a7, a5, a7
    1480: 01079793     	slli	a5, a5, 0x10
    1484: 0107e7b3     	or	a5, a5, a6
    1488: 00f8e7b3     	or	a5, a7, a5
    148c: 00f72023     	sw	a5, 0x0(a4)
    1490: 00470713     	addi	a4, a4, 0x4
    1494: fed76ce3     	bltu	a4, a3, 0x148c <memset+0x5c>
    1498: 00367613     	andi	a2, a2, 0x3
    149c: 00c68733     	add	a4, a3, a2
    14a0: 00e6fa63     	bgeu	a3, a4, 0x14b4 <memset+0x84>
    14a4: 00b68023     	sb	a1, 0x0(a3)
    14a8: fff60613     	addi	a2, a2, -0x1
    14ac: 00168693     	addi	a3, a3, 0x1
    14b0: fe061ae3     	bnez	a2, 0x14a4 <memset+0x74>
    14b4: 00008067     	ret
    14b8: 00050693     	mv	a3, a0
    14bc: 00c50733     	add	a4, a0, a2
    14c0: fee562e3     	bltu	a0, a4, 0x14a4 <memset+0x74>
    14c4: ff1ff06f     	j	0x14b4 <memset+0x84>
