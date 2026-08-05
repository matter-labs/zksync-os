# ETH Runner Scripts

## run_parallel_processes.sh

Launches multiple parallel eth-runner processes, splitting a block range across processes.

### Running Parallel Processes

```bash
./run_parallel_processes.sh [START_BLOCK] [END_BLOCK] [DB_PATH]
```

### Environment Variables

- `NUM_PROCESSES`: Number of parallel processes (default: 6)
- `ENDPOINT`: RPC endpoint URL
- `BACKUP_ENDPOINT`: Optional backup RPC endpoint
- `WEBHOOK`: Optional Slack webhook URL
- `REFETCH_TRACES`: Optional flag to refetch traces
- `LOG_DIR`: Base directory for logs (default: `logs_parallel`)
- `DB_DIR`: Base directory for databases (default: `profiling/dbs_parallel`)

### Output

Creates a log directory with metadata, process logs, and PIDs.

## overview_logs.py

Overview script for monitoring parallel eth-runner processes. Shows status, progress, metrics, and errors.

### Viewing Logs

```bash
# Basic overview
python3 tests/instances/eth_runner/scripts/overview_logs.py logs_parallel_start_19299000_end_19300000_procs_20

# With detailed block analysis
python3 tests/instances/eth_runner/scripts/overview_logs.py logs_parallel_start_19299000_end_19300000_procs_20 --block-analysis

# Save errors to JSON
python3 tests/instances/eth_runner/scripts/overview_logs.py logs_parallel_start_19299000_end_19300000_procs_20 --save-errors errors.json

# Coverage analysis across multiple runs
python3 tests/instances/eth_runner/scripts/overview_logs.py logs_parallel_start_19299000_end_19300000_procs_20 --coverage

# Show recent activity
python3 tests/instances/eth_runner/scripts/overview_logs.py logs_parallel_start_19299000_end_19300000_procs_20 --show-activity
```
