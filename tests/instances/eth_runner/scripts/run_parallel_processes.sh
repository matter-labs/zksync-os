#!/bin/bash

START_BLOCK=${1:-5187585}
END_BLOCK=${2:-5187704}
DB_PATH=${3:-"eth_runner"}
ENDPOINT=${ENDPOINT:-"https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY"}
BACKUP_ENDPOINT=${BACKUP_ENDPOINT:-""}
WEBHOOK=${WEBHOOK:-""}
NUM_PROCESSES=${NUM_PROCESSES:-6}
REFETCH_TRACES=${REFETCH_TRACES:-""}

TOTAL_BLOCKS=$((END_BLOCK - START_BLOCK + 1))
BLOCKS_PER_PROCESS=$((TOTAL_BLOCKS / NUM_PROCESSES))
REMAINDER=$((TOTAL_BLOCKS % NUM_PROCESSES))

# Create log directory with metadata
LOG_DIR_BASE=${LOG_DIR:-"logs_parallel"}
LOG_DIR="${LOG_DIR_BASE}_start_${START_BLOCK}_end_${END_BLOCK}_procs_${NUM_PROCESSES}"
mkdir -p "$LOG_DIR"

# Create DB directory with metadata
DB_DIR_BASE=${DB_DIR:-"profiling/dbs_parallel"}
DB_DIR="${DB_DIR_BASE}_start_${START_BLOCK}_end_${END_BLOCK}_procs_${NUM_PROCESSES}"
mkdir -p "$DB_DIR"

# Save metadata to file
cat > "$LOG_DIR/metadata.txt" <<EOF
Start Block: $START_BLOCK
End Block: $END_BLOCK
Total Blocks: $TOTAL_BLOCKS
Number of Processes: $NUM_PROCESSES
Blocks per Process: $BLOCKS_PER_PROCESS
Remainder: $REMAINDER
Endpoint: $ENDPOINT
Backup Endpoint: ${BACKUP_ENDPOINT:-"none"}
DB Path Base: $DB_PATH
DB Directory: $DB_DIR
Webhook: ${WEBHOOK:-"none"}
REFETCH_TRACES: ${REFETCH_TRACES:-"not set"}
Started: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
EOF

echo "=== Running $NUM_PROCESSES Single-Threaded Processes ==="
echo "Total blocks: $TOTAL_BLOCKS"
echo "Blocks per process: $BLOCKS_PER_PROCESS"
echo "Remainder: $REMAINDER"
echo "Endpoint: $ENDPOINT"
if [ ! -z "$BACKUP_ENDPOINT" ]; then
    echo "Backup endpoint: $BACKUP_ENDPOINT"
fi
echo "Log directory: $LOG_DIR"
echo ""

# Start processes
PIDS=()
CURRENT_START=$START_BLOCK

for i in $(seq 1 $NUM_PROCESSES); do
    # Calculate range for this process
    if [ $i -le $REMAINDER ]; then
        # First REMAINDER processes get one extra block
        CURRENT_END=$((CURRENT_START + BLOCKS_PER_PROCESS))
    else
        CURRENT_END=$((CURRENT_START + BLOCKS_PER_PROCESS - 1))
    fi
    
    # Don't exceed end_block
    if [ $CURRENT_END -gt $END_BLOCK ]; then
        CURRENT_END=$END_BLOCK
    fi
    
    if [ $CURRENT_START -le $END_BLOCK ]; then
        echo "Starting process $i: blocks $CURRENT_START to $CURRENT_END"
        
        # Build command with optional webhook and REFETCH_TRACES
        CMD="RUST_LOG=eth_runner=debug"
        
        # Add HOSTNAME if set
        if [ ! -z "$HOSTNAME" ]; then
            CMD="$CMD HOSTNAME=\"$HOSTNAME\""
        fi
        
        # Add process number
        CMD="$CMD PROC_NUM=$i"
        
        # Add REFETCH_TRACES if set
        if [ ! -z "$REFETCH_TRACES" ]; then
            CMD="$CMD REFETCH_TRACES=$REFETCH_TRACES"
        fi
        
        CMD="$CMD cargo run --manifest-path tests/instances/eth_runner/Cargo.toml --release --features rig/no_print,rig/unlimited_native -- \
            live-run \
            --start-block $CURRENT_START \
            --end-block $CURRENT_END \
            --endpoint \"$ENDPOINT\" \
            --skip-successful \
            --db \"${DB_DIR}/${DB_PATH}_proc${i}\""
        
        # Add backup endpoint if provided
        if [ ! -z "$BACKUP_ENDPOINT" ]; then
            CMD="$CMD --backup-endpoint \"$BACKUP_ENDPOINT\""
        fi
        
        # Add webhook if provided
        if [ ! -z "$WEBHOOK" ]; then
            CMD="$CMD --slack-webhook \"$WEBHOOK\""
        fi
        
        nohup bash -c "$CMD" > "$LOG_DIR/process_${i}.log" 2>&1 &
        
        PID=$!
        PIDS+=($PID)
        echo "  Process $i started with PID: $PID"
        CURRENT_START=$((CURRENT_END + 1))
    fi
done

echo ""
echo "Started ${#PIDS[@]} processes. PIDs: ${PIDS[@]}"
echo ""
echo "=== Processes are running in background ==="
echo "You can safely exit SSH. Processes will continue running."
echo ""
echo "Log directory: $LOG_DIR"
echo "DB directory: $DB_DIR"
echo "Metadata saved to: $LOG_DIR/metadata.txt"
echo ""
echo "To check status, run: python3 tests/instances/eth_runner/scripts/overview_logs.py $LOG_DIR"
echo "To stop processes, run: pkill -f 'eth_runner.*live-run'"
echo ""
echo "PIDs saved to: $LOG_DIR/pids.txt"
echo "${PIDS[@]}" > "$LOG_DIR/pids.txt"

