# Alternative proving workflow with Prover CLI

**Generating the CRS File**

You can set the `CSR_READS_DUMP` env variable to dump CSR reads for proving (witnesses) and then run any test.
It will create a CSR file with the path `CSR_READS_DUMP`.

**Using the Prover CLI**

The Prover CLI is part of the `zksync-airbender` repository, located in the [tools/cli](https://github.com/matter-labs/zksync-airbender/tree/main/tools/cli) directory.

Run the following from the zksync-airbender repository:

```
mkdir zkee_output

cargo run --profile cli --no-default-features -p cli prove --bin ../zksync-os/zksync_os/for_tests.bin --input-file ${CSR_READS_DUMP} --output-dir zkee_output
```

This generates multiple proof files in the `zkee_output` directory. For recursion (compressing proofs into fewer files), refer to the instructions in the `zksync-airbender` repository.
