//! The dispatch loop.

use zk_ee::system::tracer::Tracer;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt};

use super::{Env, Hot, Interpreter};
use crate::interpreter::CallScheme;
use crate::opcodes;

#[cfg(not(target_arch = "riscv32"))]
use zk_ee::system::tracer::evm_tracer::EvmTracer;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    /// Runs instructions until one stops the run with an exit code.
    ///
    /// One frame for the whole run: the hot state is loaded into locals once, the inlined
    /// instructions work on those, and the outlined ones get a copy (`Hot::outlined`).
    pub(crate) fn run_loop<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>)
    where
        S::IO: IOSubsystemExt,
    {
        let Interpreter {
            hot: holder,
            cold,
            exit_code,
        } = self;
        let mut hot = Hot::<S>::new(holder, cold.bytecode);
        loop {
            #[cfg(not(target_arch = "riscv32"))]
            let opcode = {
                let opcode = hot.peek();
                env.tracer
                    .evm_tracer()
                    .before_evm_interpreter_execution_step(
                        opcode,
                        &super::FrameView::from_hot(&hot, &*cold, &*env.system),
                    );
                env.cycles += 1;
                hot.ip = hot.ip.wrapping_add(1);
                opcode
            };
            #[cfg(target_arch = "riscv32")]
            let opcode = hot.fetch_and_advance();
            cycle_marker::opcode_start!();

            // On the proving target an instruction that stops the run breaks out of the loop
            // from its own arm, so the continuing arms jump straight back to the fetch with no
            // result value to test. The host keeps the value: its tracer sees the stopping step.
            #[cfg(not(target_arch = "riscv32"))]
            let result: crate::InstructionResult;
            macro_rules! arm {
                ($e:expr) => {{
                    #[cfg(target_arch = "riscv32")]
                    {
                        if let Err(code) = $e {
                            *exit_code = Some(code);
                            break;
                        }
                    }
                    #[cfg(not(target_arch = "riscv32"))]
                    {
                        result = $e;
                    }
                }};
            }
            match opcode {
                // inlined, on the hot state
                opcodes::STOP => arm!(hot.stop()),
                opcodes::ADD => arm!(hot.add()),
                opcodes::MUL => arm!(hot.mul()),
                opcodes::SUB => arm!(hot.sub()),
                opcodes::LT => arm!(hot.lt()),
                opcodes::GT => arm!(hot.gt()),
                opcodes::SLT => arm!(hot.slt()),
                opcodes::SGT => arm!(hot.sgt()),
                opcodes::EQ => arm!(hot.eq()),
                opcodes::ISZERO => arm!(hot.iszero()),
                opcodes::AND => arm!(hot.bitand()),
                opcodes::OR => arm!(hot.bitor()),
                opcodes::XOR => arm!(hot.bitxor()),
                opcodes::NOT => arm!(hot.not()),
                opcodes::BYTE => arm!(hot.byte()),
                opcodes::SHL => arm!(hot.shl()),
                opcodes::SHR => arm!(hot.shr()),
                opcodes::SAR => arm!(hot.sar()),
                opcodes::ADDRESS => arm!(hot.address(cold)),
                opcodes::CALLER => arm!(hot.caller(cold)),
                opcodes::CALLVALUE => arm!(hot.callvalue(cold)),
                opcodes::CALLDATALOAD => arm!(hot.calldataload(cold)),
                opcodes::CALLDATASIZE => arm!(hot.calldatasize(cold)),
                opcodes::CODESIZE => arm!(hot.codesize(cold)),
                opcodes::RETURNDATASIZE => arm!(hot.returndatasize(cold)),
                opcodes::POP => arm!(hot.pop()),
                opcodes::MLOAD => arm!(hot.mload(cold)),
                opcodes::MSTORE => arm!(hot.mstore(cold)),
                opcodes::MSTORE8 => arm!(hot.mstore8(cold)),
                opcodes::JUMP => arm!(hot.jump(cold)),
                opcodes::JUMPI => arm!(hot.jumpi(cold)),
                opcodes::PC => arm!(hot.pc(cold)),
                opcodes::MSIZE => arm!(hot.msize(cold)),
                opcodes::GAS => arm!(hot.gas()),
                opcodes::JUMPDEST => arm!(hot.jumpdest()),
                opcodes::PUSH0 => arm!(hot.push0()),
                opcodes::PUSH1 => arm!(hot.push_small::<1>()),
                opcodes::PUSH2 => arm!(hot.push_small::<2>()),
                opcodes::PUSH3 => arm!(hot.push_small::<3>()),
                opcodes::PUSH4 => arm!(hot.push_small::<4>()),
                opcodes::PUSH5 => arm!(hot.push_small::<5>()),
                opcodes::PUSH6 => arm!(hot.push_small::<6>()),
                opcodes::PUSH7 => arm!(hot.push_small::<7>()),
                opcodes::PUSH8 => arm!(hot.push_small::<8>()),
                opcodes::PUSH9 => arm!(hot.push_wide::<9>()),
                opcodes::PUSH10 => arm!(hot.push_wide::<10>()),
                opcodes::PUSH11 => arm!(hot.push_wide::<11>()),
                opcodes::PUSH12 => arm!(hot.push_wide::<12>()),
                opcodes::PUSH13 => arm!(hot.push_wide::<13>()),
                opcodes::PUSH14 => arm!(hot.push_wide::<14>()),
                opcodes::PUSH15 => arm!(hot.push_wide::<15>()),
                opcodes::PUSH16 => arm!(hot.push_wide::<16>()),
                opcodes::PUSH17 => arm!(hot.push_wide::<17>()),
                opcodes::PUSH18 => arm!(hot.push_wide::<18>()),
                opcodes::PUSH19 => arm!(hot.push_wide::<19>()),
                opcodes::PUSH20 => arm!(hot.push_wide::<20>()),
                opcodes::PUSH21 => arm!(hot.push_wide::<21>()),
                opcodes::PUSH22 => arm!(hot.push_wide::<22>()),
                opcodes::PUSH23 => arm!(hot.push_wide::<23>()),
                opcodes::PUSH24 => arm!(hot.push_wide::<24>()),
                opcodes::PUSH25 => arm!(hot.push_wide::<25>()),
                opcodes::PUSH26 => arm!(hot.push_wide::<26>()),
                opcodes::PUSH27 => arm!(hot.push_wide::<27>()),
                opcodes::PUSH28 => arm!(hot.push_wide::<28>()),
                opcodes::PUSH29 => arm!(hot.push_wide::<29>()),
                opcodes::PUSH30 => arm!(hot.push_wide::<30>()),
                opcodes::PUSH31 => arm!(hot.push_wide::<31>()),
                opcodes::PUSH32 => arm!(hot.push_wide::<32>()),
                opcodes::DUP1 => arm!(hot.dup_op::<1>()),
                opcodes::DUP2 => arm!(hot.dup_op::<2>()),
                opcodes::DUP3 => arm!(hot.dup_op::<3>()),
                opcodes::DUP4 => arm!(hot.dup_op::<4>()),
                opcodes::DUP5 => arm!(hot.dup_op::<5>()),
                opcodes::DUP6 => arm!(hot.dup_op::<6>()),
                opcodes::DUP7 => arm!(hot.dup_op::<7>()),
                opcodes::DUP8 => arm!(hot.dup_op::<8>()),
                opcodes::DUP9 => arm!(hot.dup_op::<9>()),
                opcodes::DUP10 => arm!(hot.dup_op::<10>()),
                opcodes::DUP11 => arm!(hot.dup_op::<11>()),
                opcodes::DUP12 => arm!(hot.dup_op::<12>()),
                opcodes::DUP13 => arm!(hot.dup_op::<13>()),
                opcodes::DUP14 => arm!(hot.dup_op::<14>()),
                opcodes::DUP15 => arm!(hot.dup_op::<15>()),
                opcodes::DUP16 => arm!(hot.dup_op::<16>()),
                opcodes::SWAP1 => arm!(hot.swap_op::<1>()),
                opcodes::SWAP2 => arm!(hot.swap_op::<2>()),
                opcodes::SWAP3 => arm!(hot.swap_op::<3>()),
                opcodes::SWAP4 => arm!(hot.swap_op::<4>()),
                opcodes::SWAP5 => arm!(hot.swap_op::<5>()),
                opcodes::SWAP6 => arm!(hot.swap_op::<6>()),
                opcodes::SWAP7 => arm!(hot.swap_op::<7>()),
                opcodes::SWAP8 => arm!(hot.swap_op::<8>()),
                opcodes::SWAP9 => arm!(hot.swap_op::<9>()),
                opcodes::SWAP10 => arm!(hot.swap_op::<10>()),
                opcodes::SWAP11 => arm!(hot.swap_op::<11>()),
                opcodes::SWAP12 => arm!(hot.swap_op::<12>()),
                opcodes::SWAP13 => arm!(hot.swap_op::<13>()),
                opcodes::SWAP14 => arm!(hot.swap_op::<14>()),
                opcodes::SWAP15 => arm!(hot.swap_op::<15>()),
                opcodes::SWAP16 => arm!(hot.swap_op::<16>()),

                // outlined, on a copy of the hot state
                opcodes::DIV => arm!(hot.outlined(|h| h.div(env))),
                opcodes::SDIV => arm!(hot.outlined(|h| h.sdiv(env))),
                opcodes::MOD => arm!(hot.outlined(|h| h.rem(env))),
                opcodes::SMOD => arm!(hot.outlined(|h| h.smod(env))),
                opcodes::ADDMOD => arm!(hot.outlined(|h| h.addmod(env))),
                opcodes::MULMOD => arm!(hot.outlined(|h| h.mulmod(env))),
                opcodes::EXP => arm!(hot.outlined(|h| h.exp())),
                opcodes::SIGNEXTEND => arm!(hot.outlined(|h| h.signextend())),
                opcodes::CLZ => arm!(hot.outlined(|h| h.clz())),
                opcodes::SHA3 => arm!(hot.outlined(|h| h.sha3(cold, env))),
                opcodes::BALANCE => arm!(hot.outlined(|h| h.balance(env))),
                opcodes::ORIGIN => arm!(hot.outlined(|h| h.origin(cold, env))),
                opcodes::CALLDATACOPY => arm!(hot.outlined(|h| h.calldatacopy(cold))),
                opcodes::CODECOPY => arm!(hot.outlined(|h| h.codecopy(cold))),
                opcodes::GASPRICE => arm!(hot.outlined(|h| h.gasprice(env))),
                opcodes::EXTCODESIZE => arm!(hot.outlined(|h| h.extcodesize(env))),
                opcodes::EXTCODECOPY => arm!(hot.outlined(|h| h.extcodecopy(cold, env))),
                opcodes::RETURNDATACOPY => arm!(hot.outlined(|h| h.returndatacopy(cold))),
                opcodes::EXTCODEHASH => arm!(hot.outlined(|h| h.extcodehash(env))),
                opcodes::BLOCKHASH => arm!(hot.outlined(|h| h.blockhash(env))),
                opcodes::COINBASE => arm!(hot.outlined(|h| h.coinbase(env))),
                opcodes::TIMESTAMP => arm!(hot.outlined(|h| h.timestamp(env))),
                opcodes::NUMBER => arm!(hot.outlined(|h| h.number(env))),
                opcodes::DIFFICULTY => arm!(hot.outlined(|h| h.difficulty(env))),
                opcodes::GASLIMIT => arm!(hot.outlined(|h| h.gaslimit(env))),
                opcodes::CHAINID => arm!(hot.outlined(|h| h.chainid(env))),
                opcodes::SELFBALANCE => arm!(hot.outlined(|h| h.selfbalance(cold, env))),
                opcodes::BASEFEE => arm!(hot.outlined(|h| h.basefee(env))),
                opcodes::BLOBHASH => arm!(hot.outlined(|h| h.blobhash(env))),
                opcodes::BLOBBASEFEE => arm!(hot.outlined(|h| h.blobbasefee(env))),
                opcodes::SLOAD => arm!(hot.outlined(|h| h.storage_read::<false, T>(cold, env))),
                opcodes::SSTORE => arm!(hot.outlined(|h| h.storage_write::<false, T>(cold, env))),
                opcodes::TLOAD => arm!(hot.outlined(|h| h.storage_read::<true, T>(cold, env))),
                opcodes::TSTORE => arm!(hot.outlined(|h| h.storage_write::<true, T>(cold, env))),
                opcodes::MCOPY => arm!(hot.outlined(|h| h.mcopy(cold))),
                opcodes::LOG0 => arm!(hot.outlined(|h| h.log::<0, T>(cold, env))),
                opcodes::LOG1 => arm!(hot.outlined(|h| h.log::<1, T>(cold, env))),
                opcodes::LOG2 => arm!(hot.outlined(|h| h.log::<2, T>(cold, env))),
                opcodes::LOG3 => arm!(hot.outlined(|h| h.log::<3, T>(cold, env))),
                opcodes::LOG4 => arm!(hot.outlined(|h| h.log::<4, T>(cold, env))),
                opcodes::CREATE => arm!(hot.outlined(|h| h.create::<false, T>(cold, env))),
                opcodes::CALL => arm!(hot.outlined(|h| h.call(cold, CallScheme::Call))),
                opcodes::CALLCODE => arm!(hot.outlined(|h| h.call(cold, CallScheme::CallCode))),
                opcodes::RETURN => arm!(hot.outlined(|h| h.ret(cold))),
                opcodes::DELEGATECALL => {
                    arm!(hot.outlined(|h| h.call(cold, CallScheme::DelegateCall)))
                }
                opcodes::CREATE2 => arm!(hot.outlined(|h| h.create::<true, T>(cold, env))),
                opcodes::STATICCALL => {
                    arm!(hot.outlined(|h| h.call(cold, CallScheme::StaticCall)))
                }
                opcodes::REVERT => arm!(hot.outlined(|h| h.revert(cold))),
                opcodes::SELFDESTRUCT => arm!(hot.outlined(|h| h.selfdestruct(cold, env))),
                _ => arm!(hot.outlined(|h| h.invalid_opcode(opcode))),
            }

            cycle_marker::opcode_end!(opcodes::OPCODE_JUMPMAP[opcode as usize].unwrap_or("UNKNOWN"));
            #[cfg(not(target_arch = "riscv32"))]
            {
                env.tracer
                    .evm_tracer()
                    .after_evm_interpreter_execution_step(
                        opcode,
                        &super::FrameView::from_hot(&hot, &*cold, &*env.system),
                    );
                if let Err(code) = result {
                    *exit_code = Some(code);
                    break;
                }
            }
        }
        hot.finish(cold.bytecode.as_ptr());
    }
}
