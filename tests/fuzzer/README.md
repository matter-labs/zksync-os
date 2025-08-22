# Fuzzer

## Commands Overview

### Running `cargo fuzz`

To run a specific fuzz test, use the following command:

```shell
cargo fuzz run precompiles_ecadd -- -rss_limit_mb=8192 -max_total_time=1
```

### Using the Fuzz Script

To run regression tests use the following command:
```shell
./fuzz.sh regression
```

To run smoke fuzz tests:
```shell
./fuzz.sh smoke
```

To run all fuzz tests use the following command:

```shell
./fuzz.sh parallel --jobs=8 --timeout=3600
```

To run all precompiles use the following command:

```shell
./fuzz.sh parallel --jobs=8 --target=precompiles --timeout=3600
```

This command runs fuzzing targets using the `fuzz.sh` script with the following options:
- **`parallel` **: Enables parallel mode to execute multiple instances at the same time.
- **`8`**: Sets the maximum number of parallel jobs (fuzzing instances) to 8.
- **`precompiles`**: Filters the fuzzing targets to run only those whose names start with `precompiles`.
- **`3600`**: Sets the total fuzzing runtime limit to 3600 seconds (1 hour).

To check if there are any crashes run the following command:

```shell
./fuzz.sh check
```

To clean all artifacts:

```shell
./fuzz.sh clean
```

To get all fuzz targets:

```shell
./fuzz.sh list
```

To get coverage reports (general and line-by-line) for fuzz targets inside the `fuzz/coverage-reports` directory:

```shell
./fuzz.sh parallel --jobs=8 --target=precompiles
./fuzz.sh cov-coverage --target=precompiles --triple=x86_64-unknown-linux-gnu
```
