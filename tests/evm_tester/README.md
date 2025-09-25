# ZKsync Era: The EVM Implementations Testing Framework

[![Logo](eraLogo.svg)](https://zksync.io/)

[![General state tests](https://github.com/matter-labs/era-evm-tester/actions/workflows/general_state_tests.yaml/badge.svg)](https://github.com/matter-labs/era-evm-tester/actions/workflows/general_state_tests.yaml)

ZKsync Era is a layer 2 rollup that uses zero-knowledge proofs to scale Ethereum without compromising on security
or decentralization. As it's EVM-compatible (with Solidity/Vyper), 99% of Ethereum projects can redeploy without
needing to refactor or re-audit any code. ZKsync Era also uses an LLVM-based compiler that will eventually enable
developers to write smart contracts in popular languages such as C++ and Rust.

The `era-evm-tester` test framework runs tests for EVM implementation on top of EraVM.

## Building

<details>
<summary>1. Install Rust.</summary>

   * Follow the latest [official instructions](https://www.rust-lang.org/tools/install):
      ```shell
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
      . ${HOME}/.cargo/env
      ```

      > Currently we are not pinned to any specific version of Rust, so just install the latest stable build for your platform.
</details>

<details>
<summary>2. Install ZKsync Foundry.</summary>

   * Follow the latest [instructions from ZKsync Foundry Book](https://foundry-book.zksync.io/getting-started/installation)
</details>

<details>
<summary>3. Checkout or clone the repository.</summary>

   * If you have not cloned this repository yet:
      ```shell
      git clone https://github.com/matter-labs/era-evm-tester.git --recursive
      ```

   * If you have already cloned this repository:
      ```shell
      git submodule update --init --recursive --remote
      ```

</details>

<details>
<summary>4. Build system contracts.</summary>

   * Install Node.js
  
   * Enter system contracts directory:

      ```shell
      cd era-contracts
      ```

   * Install dependencies:

      ```shell
      yarn
      ```

   * Build contracts:

      ```shell
      yarn build
      ```
</details>

When the build succeeds, you can run the tests using [the usage section](#usage).


## What is supported

### Platforms

- EVM emulator on top of EraVM



## Usage

Each command assumes you are at the root of the `evm-tester` repository.

### Generic command

```bash
cargo run --release --bin evm-tester -- [-v] \
	[--path="${PATH}"]*
```

There are more rarely used options, which you may check out with `./target/release/evm-tester --help`.

## License

The Era EVM Tester is distributed under the terms of either

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.



## Official Links

- [Website](https://zksync.io/)
- [GitHub](https://github.com/matter-labs)
- [Twitter](https://twitter.com/zksync)
- [Twitter for Devs](https://twitter.com/ZKsyncDevs)
- [Discord](https://join.zksync.dev/)



## Disclaimer

ZKsync Era has been through extensive testing and audits, and although it is live, it is still in alpha state and
will undergo further audits and bug bounty programs. We would love to hear our community's thoughts and suggestions
about it!
It's important to note that forking it now could potentially lead to missing important
security updates, critical features, and performance improvements.
