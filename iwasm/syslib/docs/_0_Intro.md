# Intro

**Everything is still in development and the docs should be treated as suggestion/reminder/todo list**

## Requirements

- The library should not require any changes or alterations of the Rust toolchain.

## Architecture abstraction

The library can be used in 3 modes:
 - Wasm: compiled to wasm and uses `host` for system calls.
 - Native: compiled to current cpu native and doesn't use the host but executes the specification code directly.
 - Native with host emulation: compiled to current cpu native and uses host emulation to call the specification code.

The indirection is achieved as follows:

### IntX
 
`IntX<N>` -> `sys::intx::*` -> `impl_arch::*` ─┬> `impl_native` -> `native_host` ─┬> `host_specification`
                                               |                                  |
                                               └> `impl_wasm` -> `wasm_host` ─────┘

`impl_arch` - conditionally chosen based on the architecture at compile time.

Calls to host are made through the `short_host_op`/`long_host_op`. In case of `risc-v` + `wasm` the host is defined in the `zkOS` and the host ops are handled through the extern calls.
