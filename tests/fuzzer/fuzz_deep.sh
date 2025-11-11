#!/usr/bin/env bash

# Settings to configure through env variables
ONE_SHOT="${ONE_SHOT:-}"
ROTATE_SECONDS="${ROTATE_SECONDS:-86400}"
FORK="${FORK:-$(nproc)}"
SKIP_BUILD="${SKIP_BUILD:-0}"
: "${SLACK_WEBHOOK_URL:?Set SLACK_WEBHOOK_URL env var}"

# Cargo fuzz run parameters
FUZZ_FLAGS=(
  -use_value_profile=1
  -entropic=1
  -reload=1
  -max_total_time="$ROTATE_SECONDS"
  -timeout=5
  -ignore_timeouts=1
  -fork="$FORK"
)

notify_slack() {
  local msg="$1"
  local commit_hash=$(git rev-parse HEAD)
  local check_output="ZKSync OS fuzzer on $commit_hash: $msg"
  local payload=$(printf '{"text": "%s"}' "$check_output")

  # Send the payload to the Slack webhook
  curl -sS -X POST -H "Content-Type: application/json" -d "$payload" "$SLACK_WEBHOOK_URL" >/dev/null
}

discover_all_targets() {
  cargo fuzz list 2>/dev/null | awk NF
}

filter_targets_by_prefixes() {
  local -a pref=("$@"); local t; local -a all
  mapfile -t all < <(discover_all_targets)
  for p in "${pref[@]}"; do
    for t in "${all[@]}"; do
      [[ "$t" == "$p"* ]] && echo "$t"
    done
  done | awk '!seen[$0]++'
}

monitor_findings() {
  local artifacts="$1" target="$2"
  local -a known=()
  mapfile -t known < <(ls -1 "$artifacts/$target" 2>/dev/null)
  while sleep 30; do
    local -a current=()
    mapfile -t current < <(ls -1 "$artifacts/$target" 2>/dev/null)
    for f in "${current[@]}"; do
      [[ " ${known[*]} " == *" $f "* ]] && continue
      notify_slack ":rotating_light: Crash detected :rotating_light: $artifacts/$target/$f"
    done
    known=("${current[@]}")
  done
}

cleanup() {
  if [[ -n "${run_pid:-}" ]]; then
    kill "$run_pid" 2>/dev/null
    wait "$run_pid" 2>/dev/null
  fi
  if [[ -n "${mon_pid:-}" ]]; then
    kill "$mon_pid" 2>/dev/null
    wait "$mon_pid" 2>/dev/null
  fi

  # Minimize corpus for the current target
  if [[ -n "${target:-}" ]]; then
    cargo fuzz cmin "$target" "$corpus/$target" -- -merge=1
  fi
}

trap halt INT TERM

halt() {
  notify_slack ":octagonal_sign: Fuzzer halted"
  cleanup
  exit 0
}

if [[ "$SKIP_BUILD" != "1" ]]; then
  # Build fuzzer with optimizations
  RUSTFLAGS="-C target-cpu=native -C codegen-units=1 -C panic=abort -C debuginfo=0" \
  cargo fuzz build -O
else
  echo "skipping cargo fuzz build"
fi

if [[ "${1:-}" == "all" ]]; then
  mapfile -t TARGETS < <(discover_all_targets)
else
  [[ $# -gt 0 ]] || { echo "Usage: $0 all | <prefix> [prefix ...]"; exit 1; }
  mapfile -t TARGETS < <(filter_targets_by_prefixes "$@")
fi

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  echo "No fuzz targets matched. Available targets:"
  discover_all_targets
  exit 1
fi

workdir="fuzz"
corpus="$workdir/corpus"
artifacts="$workdir/artifacts"
logs="$workdir/logs"

mkdir -p "$logs"

run_target() {
  local target="$1"

  stamp="$(date +'%Y-%m-%d_%H-%M-%S')"
  log_file="$logs/${target}_${stamp}.log"

  notify_slack ":hourglass_flowing_sand: Fuzzing $target for ${ROTATE_SECONDS}s (fork=$FORK)"

  monitor_findings "$artifacts" "$target" &
  mon_pid=$!

  mapfile -t flags < <(printf "%s\n" "${FUZZ_FLAGS[@]}")

  ( setsid stdbuf -oL -eL cargo fuzz run "$target" -- "${flags[@]}" 2>&1 | tee -a "$log_file" ) &
  run_pid=$!

  wait "$run_pid" 2>/dev/null

  cleanup

  notify_slack ":white_check_mark: Completed $target fuzzing; corpus minimized."
}

# Run each target exactly once then exit when ONE_SHOT is set
if [[ -n "${ONE_SHOT:-}" ]]; then
  overall=0
  for t in "${TARGETS[@]}"; do
    run_target "$t" || overall=$?
  done
  exit "$overall"
fi

# Run fuzzing round-robin schedule (default)
idx=0
while true; do
  target="${TARGETS[$idx]}"
  idx=$(( (idx + 1) % ${#TARGETS[@]} ))
  run_target "$target"
done