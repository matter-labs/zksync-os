# WASM interpreter for zkOS environment (integer-WASM)

Main features:
- integer support only
- incorporation of gas cost only at control-flow points
- all IO (as well as cross-contract calls and calldata/returndata) is by host functions
- U256 operations (and heavy math in general) also by host functions
- heavy use of code preprocessing:
    - validation inspired by [https://arxiv.org/pdf/2205.01183.pdf](https://arxiv.org/pdf/2205.01183.pdf), as well as sidetable use for control flow and gas
    - interpreted will run ONLY code that was validated, and will require validation artifact to run
    - validation artifact may contain not just sidetable described in the article, but also information that can be processes and cached once in internal formats for faster processing, e.g. function ABIs, precomputed globals, checked compliance to expected host functions and memory structure
    - as we expect to have quite a lot of host functions we can consider to remove "import" section and validate against static one (for smaller bytecode size) if import section stability can be achieved

TODO list:
- [x] validator
- [x] validation artifact format
- [ ] consider moving gas costs to the sidetable
- [ ] check what other preprocessed items can be saved as artifacts
- [ ] interpreted that will use `System` trait for zkOS
- [ ] efficiency testing vs EVM interpreter on "real" equivalent use cases
- [ ] testing whether extra cost of decommitment of some parts of preprocessing artifact will outweigh recomputing them
    - larger artifact <-> smaller main bytecode, so that is not obvious

## Notes
- U256 host functions looks to be more efficient by-value, but that requires further investigation
- Alternative is to "box" all large types that will be operated by host environment, and have basic bump-allocator instead of having LLVM shadow-stack