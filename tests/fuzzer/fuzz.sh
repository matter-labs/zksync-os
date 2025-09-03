#!/usr/bin/env bash

FUZZ_LOGS="./fuzz_logs"
FUZZ_ARTIFACTS="./fuzz/artifacts"
FUZZ_CORPUS="./fuzz/corpus"
FUZZ_SEEDS="./fuzz/seeds"
# Resolve absolute path for zksync-os
OVERRIDE_ZKSYNC_OS_PATH="../../zksync_os"
export OVERRIDE_ZKSYNC_OS_PATH

function usage() {
    echo "
Usage: $0 <command> [options]
Commands:
    list                          List existing fuzz targets
    smoke --timeout=<seconds>     Sequentially run all fuzzers for X seconds (default: 60)
    regression                    Run fuzzer on the test inputs (corpus) without fuzzing
    lint                          Lint fuzz tests
    check                         Check if a crash has occurred
    clean                         Clean fuzz data (artifacts and corpus)
    prepare                       Prepare the system for fuzzing
    corpus                        Generate corpus files from seeds explicitly
    install                       Install dependencies
    run                           Run default fuzz tests
        --jobs=<number>           Max number of jobs to run in parallel (default: 16)
        --timeout=<seconds>       Timeout for the fuzz tests (default: 21600)
    report                        Make a report
    coverage                      Collect coverage
        --target=<target>         Fuzz target to collect coverage for (default: '*')
        --triple=<triple>         Arch-platform-OS triple, e.g., aarch64-apple-darwin, (default: computed)
        --profile                 Build profile, e.g., debug or release (default: release)
    cov-coverage                  Collect coverage with cargo cov
        --target=<target>         Fuzz target to collect coverage for (default: '*')
        --triple=<triple>         Arch-platform-OS triple, e.g., aarch64-apple-darwin, (default: computed)
        --profile                 Build profile, e.g., debug or release (default: release)
    parallel                      Run fuzz tests in parallel
        --jobs=<number>           Max number of jobs to run in parallel
        --target=<target>         Prefix for fuzz targets to run (default: '*')
        --timeout=<seconds>       Timeout for the fuzz tests (default: 600)
Options:
    -h, --help                    Show this help message
"
    exit 1
}

# Utility function to parse key-value arguments (--key=value)
function parse_args() {
    for arg in "$@"; do
        case $arg in
            --*=*) key=$(echo "$arg" | cut -d '=' -f 1); value=$(echo "$arg" | cut -d '=' -f 2); eval "${key#--}='$value'" ;;
            --*) key=$(echo "$arg" | cut -d '=' -f 1); eval "${key#--}=1" ;;
        esac
    done
}

function run_report() {
  	echo "Sending report to Slack..."
  	if [ -z "$SLACK_WEBHOOK_URL" ]; then
  			echo "Error: SLACK_WEBHOOK_URL is not set. Aborting.";
  			exit 1; \
  	fi

  	commit_hash=$(git rev-parse HEAD)

  	check_output="zk_ee fuzzer on $commit_hash: $(check)"
  	if [ -z "$check_output" ]; then
        echo "Error: Check output is empty. Aborting."
        exit 1
    fi

    payload=$(printf '{"text": "%s"}' "$check_output")

    # Send the payload to the Slack webhook
    response=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" -d "$payload" "$SLACK_WEBHOOK_URL")

    if [ "$response" -ne 200 ]; then
        echo "Error: Failed to send report to Slack. HTTP status code: $response"
        exit 1
    fi

    echo "Report successfully sent to Slack."

}

function run_run() {
    echo "Running fuzz tests session with the default settings..."

    parse_args "$@"

    jobs="${jobs:-16}"
    timeout="${timeout:-21600}" # 6 hours

    echo "Running default fuzzing with the following parameters:"
    echo "  Jobs: $jobs"
    echo "  Timeout: $timeout seconds"

    if [ -z "$SLACK_WEBHOOK_URL" ]; then
        echo "Error: SLACK_WEBHOOK_URL is not set. Aborting.";
        exit 1; \
    fi

    clean
    install
    prepare

    run_parallel --jobs="$jobs" --timeout="$timeout" --target="*"

    run_report

    coverage
}

function run_parallel() {
    corpus

    parse_args "$@"

    # Default argument values
    jobs="${jobs:-4}"
    target="${target:-"*"}"
    timeout="${timeout:-600}"

    echo "Running parallel fuzzing with the following parameters:"
    echo "  Jobs: $jobs"
    echo "  Fuzz targets template: $target"
    echo "  Timeout: $timeout seconds"

    # Match all fuzzing targets if target is '*'
    if [ "$target" = "*" ]; then
        targets=$(cargo fuzz list)
    else
        targets=$(cargo fuzz list | grep -E "^${target}")
    fi

    if [ -z "$targets" ]; then
        echo "No fuzzing targets matching '$target' found!"
        exit 1
    fi

    echo "  Fuzz targets:"
    for target in $targets; do
        echo -e "\t$target"
    done


    mkdir -p "$FUZZ_LOGS"

    memory="-rss_limit_mb=8192"

    parallel -j "$jobs" -v --eta --progress --results "$FUZZ_LOGS" \
        cargo fuzz run -D "{}" -- "$memory" -max_total_time="$timeout" -close_fd_mask=3 ::: "$targets"
}

function check() {
    if [[ ! -d "$FUZZ_ARTIFACTS" ]]; then
        echo "Error: Output directory '$FUZZ_ARTIFACTS' not found!"
        exit 1
    fi

    CRASH_FILES=$(find "$FUZZ_ARTIFACTS" -type f \( -name "crash-*" -o -name "leak-*" -o -name "timeout-*" \))

    if [[ -n "$CRASH_FILES" ]]; then
        echo "🚨 Crash detected! 🚨"
        echo "$CRASH_FILES"
        exit 1
    else
        echo "✅ No crashes found."
        exit 0
    fi
}

function clean() {
    echo "Cleaning artifacts directory: $FUZZ_ARTIFACTS"
    rm -rf ${FUZZ_ARTIFACTS:?}/*

    echo "Cleaning logs directory: $FUZZ_LOGS"
    rm -rf ${FUZZ_LOGS:?}/*

    echo "Cleaning logs directory: $FUZZ_CORPUS"
    rm -rf ${FUZZ_CORPUS:?}/*
}

function list() {
    cargo fuzz list
}

function corpus() {
    echo "Creating corpus directories for all fuzz targets..."
    rm -rf $FUZZ_CORPUS

    # Create corpus directories for all fuzz targets
    FUZZ_TARGETS=$(cargo fuzz list)
    for target in $FUZZ_TARGETS; do
        mkdir -p "$FUZZ_CORPUS/$target"
    done

    for seed_dir in "$FUZZ_SEEDS"/*; do
        if [ -d "$seed_dir" ]; then
            seed_name=$(basename "$seed_dir")

            # Check if the name matches any fuzz target
            if echo "$FUZZ_TARGETS" | grep -q "$seed_name"; then
                cp -v "$seed_dir"/* "$FUZZ_CORPUS/$seed_name/" 2>/dev/null || true
            # else do nothing
            else
                # noop
                :
            fi
        fi
    done

    # use the same local seeds for bootloader transactions handlers
    mkdir -p "$FUZZ_CORPUS/bootloader_process_transaction"
    mkdir -p "$FUZZ_CORPUS/bootloader_tx_calculate_signed_hash"
    mkdir -p "$FUZZ_CORPUS/bootloader_tx_parser"
    mkdir -p "$FUZZ_CORPUS/bootloader_tx_validate"

    find "$FUZZ_SEEDS/bootloader_transaction_data/" -type f | while read -r file; do
        cp "$file" "$FUZZ_CORPUS/bootloader_process_transaction/"
        cp "$file" "$FUZZ_CORPUS/bootloader_tx_calculate_signed_hash/"
        cp "$file" "$FUZZ_CORPUS/bootloader_tx_parser/"
        cp "$file" "$FUZZ_CORPUS/bootloader_tx_validate/"
    done
}

function install() {
    rustup set profile minimal
    rustup target add riscv32i-unknown-none-elf
    rustup target add wasm32-unknown-unknown
    cargo install cargo-binutils
    rustup component add llvm-tools-preview
    rustup component add rust-src
    cargo install cargo-fuzz
    cargo install rustfilt
    # this is required for merging coverage (lcov) files
    cargo install lcov-util
    # this needs nodejs and npm
    npm install -g @lcov-viewer/cli
}

function prepare() {
    pushd ../../zksync_os || exit
    /bin/bash ./dump_bin.sh || exit
    popd || exit
}

function lint() {
    cargo fmt
    cargo clippy --workspace -- -D warnings
}

function smoke() {
    clean
    corpus
    prepare

    parse_args "$@"

    timeout="${timeout:-60}"
    jobs="${jobs:-8}"

    FUZZ_TARGETS=(
        "bootloader_run_single_interaction"
        "bootloader_supported_ees"
        "bootloader_tx_parser"
        "bootloader_tx_try_from_slice"
        "bootloader_tx_validate"
        "precompiles_ecpairing"
        "precompiles_ecrecover"
        "precompiles_modexp"
        "precompiles_modexplen"
        "precompiles_ripemd160"
        "precompiles_sha256"
        "precompiles_id"
        "precompiles_ecadd"
        "precompiles_ecmul"
        "precompiles_p256"
    )

    for TARGET in "${FUZZ_TARGETS[@]}"; do
        echo "============================================"
        echo "Running fuzz target: $TARGET"
        echo "Duration: $timeout seconds"
        echo "============================================"
        cargo fuzz run -D "$TARGET" -- -max_total_time="$timeout" -jobs="$jobs" -rss_limit_mb=8192 -close_fd_mask=3
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
          echo "$TARGET: finished fuzz target"
        else
          echo "$TARGET: cargo fuzz failed or found a crash. Exit code: $exit_code"
          if [ -n "$SLACK_WEBHOOK_URL" ]; then
              echo "SLACK_WEBHOOK_URL is set. Sending report."
              run_report
          fi
          exit $exit_code
        fi
        echo "============================================"
    done

    if [ -n "$SLACK_WEBHOOK_URL" ]; then
        echo "SLACK_WEBHOOK_URL is set. Sending report."
        run_report
    fi

    check
}

function regression() {
    corpus

    prepare

    cargo fuzz build
    echo "Running regression tests on all fuzz targets..."

    # Get the list of fuzz targets
    FUZZ_TARGETS=$(cargo fuzz list)

    if [ -z "$FUZZ_TARGETS" ]; then
        echo "No fuzz targets found."
        exit 1
    fi

    # detect the Rust host triple correctly
    TRIPLE=$(rustc -Vv | sed -n 's/^host: //p')

    # For each fuzz target, find corresponding corpus files
    for TARGET in $FUZZ_TARGETS; do
        CORPUS_DIR="${FUZZ_CORPUS}/${TARGET}"

        if [[ ! -d "$CORPUS_DIR" ]]; then
            echo "⚠️  No corpus directory found for fuzz target: $TARGET"
            continue
        fi

        BIN="target/${TRIPLE}/release/${TARGET}"

        # Loop through each file in the corpus directory
        for CORPUS_FILE in "$CORPUS_DIR"/*; do
            # Ensure the file exists before testing
            if [[ ! -f "$CORPUS_FILE" ]]; then
                echo "⚠️  No corpus files found for fuzz target: $TARGET"
                continue
            fi


            echo "============================================"
            echo "Running regression test for target: $TARGET"
            echo "Using corpus file: $CORPUS_FILE"
            echo "============================================"

            # Run the target against the corpus file
            "$BIN" "$CORPUS_FILE" -rss_limit_mb=8192 -close_fd_mask=3
            exit_code=$?
            if [ $exit_code -eq 0 ]; then
                echo "$TARGET: finished fuzz target"
            else
                echo "$TARGET: cargo fuzz failed or found a crash on. Exit code: $exit_code"
                exit $exit_code
            fi
        done
    done

    echo "Regression tests completed."
}

function target_triple() {
    # print the triple of the current system, e.g., aarch64-apple-darwin
    rustc -vV | grep "host:" | awk '{print $2}'
}

function cov_coverage() {
    # get the script directory, resolve symlinks
    script_dir=$(dirname $0)
    script_dir=$(cd $script_dir && pwd)
    report_dir="fuzz/coverage-reports"

    parse_args "$@"

    set -e

    # Default argument values
    target=${target:-"*"}
    triple=${triple:-$(target_triple)}
    profile=${profile:-"release"}

    # Match all fuzzing targets if target is '*'
    if [ "$target" = "*" ]; then
        targets=$(cargo fuzz list)
    else
        targets=$(cargo fuzz list | grep -E "^${target}")
    fi

    if [ -z "$targets" ]; then
        echo "⚠️ ️No fuzzing targets matching '$target' found!"
        exit 1
    fi

    echo "  Fuzz targets:"
    for fuzz_target in $targets; do
        echo -e "\t$fuzz_target"

        # if profile=="debug", set coverage_flags to "-D"
        coverage_flags=""
        if [ "$profile" == "debug" ]; then
            coverage_flags="-D"
        fi

        # see https://rust-fuzz.github.io/book/cargo-fuzz/coverage.html
        profdata=$(cargo fuzz coverage ${coverage_flags} ${fuzz_target} 2>&1 -- -rss_limit_mb=8192 | \
            tee /dev/stderr | \
            grep 'Coverage data merged and saved in' | \
            sed 's/Coverage data merged and saved in "\(.*\)"./\1/')

        if [ -z "$profdata" ]; then
            echo "⚠️ Failed to generate coverage data"
            exit 1
        fi

        # find the target executable
        target_bin="${script_dir}/target/${triple}/coverage/${triple}/${profile}/${fuzz_target}"
        if [ ! -f "$target_bin" ]; then
            echo "⚠️ Failed to find the executable for ${fuzz_target}"
            exit 3
        fi

        cargo cov -- --help &> /dev/null
        if [ $? -ne 0 ]; then
            echo "⚠️ llvm-cov not found. Install llvm-tools-preview:"
            echo "️ℹ️ rustup component add --toolchain nightly llvm-tools-preview"
            exit 5
        fi

        components=("basic_system" "basic_bootloader" "evm_interpreter" "system_hooks" "crypto" "storage_models" "supporting_crates" \
        "zksync_os" "zk_ee" "forward_system")
        sources=" "
        for comp in "${components[@]}"
        do
            sources="$sources -sources ../../$comp "
        done

        mkdir -p ${report_dir}/${fuzz_target}

        cargo cov -- report $target_bin \
            -instr-profile=$profdata -Xdemangler=rustfilt -use-color \
            ${sources} > fuzz/coverage-reports/${fuzz_target}/report.txt

        cargo cov -- show $target_bin \
            -instr-profile=$profdata -Xdemangler=rustfilt -format=html -use-color \
            ${sources} > fuzz/coverage-reports/${fuzz_target}/coverage.html
    done
}

function coverage() {
    # get the script directory, resolve symlinks
    script_dir=$(dirname $0)
    script_dir=$(cd $script_dir && pwd)
    coverage_dir="./coverage"
    report_dir="./report-coverage"
    mkdir -p "$coverage_dir"

    parse_args "$@"

    set -e

    # Default argument values
    target=${target:-"*"}
    triple=${triple:-$(target_triple)}
    profile=${profile:-"release"}

    # Match all fuzzing targets if target is '*'
    if [ "$target" = "*" ]; then
        targets=$(cargo fuzz list)
    else
        targets=$(cargo fuzz list | grep -E "^${target}")
    fi

    if [ -z "$targets" ]; then
        echo "⚠️ ️No fuzzing targets matching '$target' found!"
        exit 1
    fi

    echo "  Fuzz targets:"
    all_cov_files=""
    for fuzz_target in $targets; do
        echo -e "\t$fuzz_target"

        # if profile=="debug", set coverage_flags to "-D"
        coverage_flags=""
        if [ "$profile" == "debug" ]; then
            coverage_flags="-D"
        fi

        # see https://rust-fuzz.github.io/book/cargo-fuzz/coverage.html
        profdata=$(cargo fuzz coverage ${coverage_flags} ${fuzz_target} 2>&1 -- -rss_limit_mb=8192 | \
            tee /dev/stderr | \
            grep 'Coverage data merged and saved in' | \
            sed 's/Coverage data merged and saved in "\(.*\)"./\1/')

        if [ -z "$profdata" ]; then
            echo "⚠️ Failed to generate coverage data"
            exit 1
        fi

        # find the target executable
        target_bin="${script_dir}/target/${triple}/coverage/${triple}/${profile}/${fuzz_target}"
        if [ ! -f "$target_bin" ]; then
            echo "⚠️ Failed to find the executable for ${fuzz_target}"
            exit 3
        fi

        if [ ! -f $(which llvm-cov) ]; then
            echo "⚠️ llvm-cov not found. Install llvm-tools-preview:"
            echo "️ℹ️ rustup component add --toolchain nightly llvm-tools-preview"
            exit 5
        fi

        cov_file="${coverage_dir}/${fuzz_target}.lcov"
        all_cov_files="${all_cov_files} ${cov_file}"

        set -x
        llvm-cov export ${target_bin} --format=lcov \
            -instr-profile=${profdata} >${cov_file}
        { set +x; } 2>/dev/null
        # temporary workaround for: https://github.com/davglass/lcov-parse/pull/12
        perl -pi -e 's/end_of_record/end_of_recorD/ if /^.*:.*end_of_record/' ${cov_file}

        echo "✅ Coverage data saved to ${cov_file}"
    done

    echo "✅ OK. Targets: $targets"

    if [ ! -f $(which lcov-merge) ]; then
        echo "⚠️ lcov-merge not found. Run $0 install."
        exit 6
    fi

    # aggregate the coverage data
    lcov_all="lcov-all.lcov"
    set -x
    lcov-merge --loose ${all_cov_files} >${lcov_all}
    { set +x; } 2>/dev/null
    echo "✅ Coverage data merged to ${lcov_all}"
    echo "ℹ️  Use an LCOV viewer, e.g., https://marketplace.visualstudio.com/items?itemName=rherrmannr.code-coverage-lcov"

    if [ ! -f $(which lcov-viewer) ]; then
        echo "⚠️ llvm-viewer not found. Run $0 install."
        exit 7
    fi

    echo "Generating an HTML report"
    set -x
    lcov-viewer lcov -o ${report_dir} ./lcov-all.lcov
    { set +x; } 2>/dev/null
    echo "✅ HTML report generated in ${report_dir}"
}

# Main script logic
case "$1" in
    "list")
        shift
        list
        ;;
    "corpus")
        shift
        corpus
        ;;
    "lint")
        shift
        lint
        ;;
    "smoke")
        shift
        smoke "$@"
        ;;
    "regression")
        shift
        regression "$@"
        ;;
    "check")
        shift
        check
        ;;
    "clean")
        shift
        clean
        ;;
    "prepare")
        shift
        prepare
        ;;
    "install")
        shift
        install
        ;;
    "parallel")
        shift
        run_parallel "$@"
        ;;
    "run")
        shift
        run_run "$@"
        ;;
    "report")
        shift
        run_report
        ;;
    "coverage")
        shift
        coverage "$@"
        ;;
    "cov-coverage")
        shift
        cov_coverage "$@"
        ;;
    *)
        usage
        ;;
esac
