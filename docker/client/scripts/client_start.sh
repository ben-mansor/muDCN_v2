#!/bin/bash
set -e

# Set up environment variables
export SERVER_ADDR=${SERVER_ADDR:-172.20.0.2}
export SERVER_PORT=${SERVER_PORT:-9000}
export LATENCY=${LATENCY:-10ms}
export PACKET_LOSS=${PACKET_LOSS:-0%}
export BANDWIDTH=${BANDWIDTH:-100mbit}
export BENCHMARK_TYPE=${BENCHMARK_TYPE:-constant_interest_flood}
export INTEREST_RATE=${INTEREST_RATE:-1000}
export RUN_DURATION=${RUN_DURATION:-60}
export METRICS_DIR=/app/metrics
export CLIENT_ID=$(hostname)

# Skip network emulation for now to avoid container exit
echo "NOTICE: Skipping network emulation configuration to ensure container stability"
echo "Using environment variables: Latency=$LATENCY, Packet Loss=$PACKET_LOSS, Bandwidth=$BANDWIDTH"

# Create metrics directory if it doesn't exist
mkdir -p $METRICS_DIR

# Verify network conditions
echo "Network configuration:"
tc qdisc show dev eth0
tc class show dev eth0
tc filter show dev eth0

# Prepare metrics collection
mkdir -p $METRICS_DIR
CLIENT_METRICS_FILE="$METRICS_DIR/${CLIENT_ID}_metrics.csv"
touch $CLIENT_METRICS_FILE
echo "timestamp,interest_name,rtt_ms,data_size,success,error_type,measured_mtu" > $CLIENT_METRICS_FILE

# Define benchmark utility functions
run_constant_interest_flood() {
    echo "Running constant Interest flood benchmark at $INTEREST_RATE packets/sec for $RUN_DURATION seconds"
    
    # Calculate delay between interests in microseconds
    DELAY_US=$(echo "1000000 / $INTEREST_RATE" | bc)
    
    # Track start time
    START_TIME=$(date +%s)
    END_TIME=$((START_TIME + RUN_DURATION))
    COUNTER=0
    
    # Run loop until duration is reached
    while [ $(date +%s) -lt $END_TIME ]; do
        COUNTER=$((COUNTER + 1))
        TIMESTAMP=$(date +%s%3N)  # Milliseconds since epoch
        
        # Generate unique Interest name based on counter
        INTEREST_NAME="/test/flood/$(hostname)/${COUNTER}"
        
        # Send Interest and measure RTT
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 1000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Parse result
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        # Log metrics
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU" >> $CLIENT_METRICS_FILE
        
        # Sleep to maintain interest rate
        # Use sleep with decimal seconds instead of usleep
        sleep $(echo "scale=6; $DELAY_US/1000000" | bc)
    done
    
    echo "Sent $COUNTER Interest packets in $RUN_DURATION seconds"
}

run_repeated_interests() {
    echo "Running repeated Interests benchmark at $INTEREST_RATE packets/sec for $RUN_DURATION seconds"
    
    # Calculate delay between interests in microseconds
    DELAY_US=$(echo "1000000 / $INTEREST_RATE" | bc)
    
    # Track start time
    START_TIME=$(date +%s)
    END_TIME=$((START_TIME + RUN_DURATION))
    COUNTER=0
    
    # Create a set of repeating interests (10 different names)
    INTEREST_POOL=()
    for i in {1..10}; do
        INTEREST_POOL+=("/test/repeat/$(hostname)/${i}")
    done
    
    # Run loop until duration is reached
    while [ $(date +%s) -lt $END_TIME ]; do
        COUNTER=$((COUNTER + 1))
        TIMESTAMP=$(date +%s%3N)  # Milliseconds since epoch
        
        # Select interest name from pool (round robin)
        IDX=$((COUNTER % 10))
        INTEREST_NAME=${INTEREST_POOL[$IDX]}
        
        # Send Interest and measure RTT
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 1000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Parse result
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        # Log metrics
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU" >> $CLIENT_METRICS_FILE
        
        # Sleep to maintain interest rate
        # Use sleep with decimal seconds instead of usleep
        sleep $(echo "scale=6; $DELAY_US/1000000" | bc)
    done
    
    echo "Sent $COUNTER Interest packets (repeating pattern) in $RUN_DURATION seconds"
}

run_mtu_prediction_test() {
    echo "Running MTU prediction test with varying packet sizes for $RUN_DURATION seconds"
    
    # Track start time
    START_TIME=$(date +%s)
    END_TIME=$((START_TIME + RUN_DURATION))
    COUNTER=0
    
    # Run loop until duration is reached
    while [ $(date +%s) -lt $END_TIME ]; do
        COUNTER=$((COUNTER + 1))
        TIMESTAMP=$(date +%s%3N)  # Milliseconds since epoch
        
        # Generate Interest name with varying size request
        # Cycle through different sizes: 100, 500, 1000, 5000, 10000 bytes
        case $((COUNTER % 5)) in
            0) SIZE=100 ;;
            1) SIZE=500 ;;
            2) SIZE=1000 ;;
            3) SIZE=5000 ;;
            4) SIZE=10000 ;;
        esac
        
        INTEREST_NAME="/test/mtu/$(hostname)/size=${SIZE}"
        
        # Send Interest and measure RTT
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --request-size $SIZE --timeout 2000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Parse result
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        # Log metrics
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU" >> $CLIENT_METRICS_FILE
        
        # Sleep a bit between requests (adjust based on INTEREST_RATE)
        DELAY_MS=$(echo "1000 / $INTEREST_RATE" | bc)
        sleep 0.$(printf "%03d" $DELAY_MS)
    done
    
    echo "Completed MTU prediction test with $COUNTER Interest packets"
}

run_cache_warmup_test() {
    echo "Running cache cold start vs warmed test"
    
    # First phase: Cold start (unique interests)
    echo "Phase 1: Cold start with unique interests"
    for i in {1..100}; do
        TIMESTAMP=$(date +%s%3N)
        INTEREST_NAME="/test/cache/cold/$(hostname)/${i}"
        
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 1000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Parse result as before
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        # Mark as cold cache
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU,cold" >> $CLIENT_METRICS_FILE
        
        # Sleep briefly
        sleep 0.1
    done
    
    # Second phase: Warm cache (repeated interests)
    echo "Phase 2: Warm cache with repeated interests"
    for i in {1..100}; do
        TIMESTAMP=$(date +%s%3N)
        # Use same interest names as in cold phase
        INTEREST_NAME="/test/cache/cold/$(hostname)/$((i % 20 + 1))"
        
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 1000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Parse result as before
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        # Mark as warm cache
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU,warm" >> $CLIENT_METRICS_FILE
        
        # Sleep briefly
        sleep 0.1
    done
    
    echo "Completed cache warm/cold test"
}

run_controller_fallback_test() {
    echo "Running controller failure fallback test"
    
    # First phase: Normal operation
    echo "Phase 1: Normal operation"
    for i in {1..50}; do
        TIMESTAMP=$(date +%s%3N)
        INTEREST_NAME="/test/fallback/normal/${i}"
        
        START_NS=$(date +%s%N)
        RESULT=$(quic_test --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 1000 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Log metrics as before
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU,normal" >> $CLIENT_METRICS_FILE
        
        sleep 0.1
    done
    
    # Second phase: Simulate controller unreachable by increasing latency drastically
    echo "Phase 2: Simulating controller failure with high latency"
    tc qdisc change dev eth0 parent 1:12 netem delay 5000ms loss 20%
    
    for i in {1..50}; do
        TIMESTAMP=$(date +%s%3N)
        INTEREST_NAME="/test/fallback/degraded/${i}"
        
        START_NS=$(date +%s%N)
        RESULT=$(quic_client --server $SERVER_ADDR --port $SERVER_PORT --interest "$INTEREST_NAME" --timeout 6000 --fallback true 2>&1) || true
        END_NS=$(date +%s%N)
        RTT_MS=$(echo "($END_NS - $START_NS) / 1000000" | bc)
        
        # Log metrics with fallback indicator
        SUCCESS=0
        ERROR_TYPE=""
        DATA_SIZE=0
        MTU=0
        
        if echo "$RESULT" | grep -q "Received Data"; then
            SUCCESS=1
            DATA_SIZE=$(echo "$RESULT" | grep "Data size:" | awk '{print $3}')
            MTU=$(echo "$RESULT" | grep "MTU:" | awk '{print $2}' || echo "0")
        else
            ERROR_TYPE=$(echo "$RESULT" | grep "Error:" | sed 's/Error: //' || echo "timeout")
        fi
        
        echo "$TIMESTAMP,$INTEREST_NAME,$RTT_MS,$DATA_SIZE,$SUCCESS,$ERROR_TYPE,$MTU,fallback" >> $CLIENT_METRICS_FILE
        
        sleep 0.1
    done
    
    # Restore normal network conditions
    tc qdisc change dev eth0 parent 1:12 netem delay $LATENCY loss $PACKET_LOSS
    
    echo "Completed controller fallback test"
}

# Simple wait function to give the server time to start up
echo "Waiting for server at $SERVER_ADDR:$SERVER_PORT to be ready..."
sleep 5

# Create a simple health check function
check_server_health() {
    ping -c 1 $SERVER_ADDR &>/dev/null
    return $?
}

# Check if the server is reachable
if ! check_server_health; then
    echo "WARNING: Cannot reach server at $SERVER_ADDR. Network might not be properly set up."
    # Continue anyway for testing purposes
fi

# Now run the selected benchmark test with error handling
echo "Starting benchmark: $BENCHMARK_TYPE"
case $BENCHMARK_TYPE in
    "constant_interest_flood")
        run_constant_interest_flood || echo "Benchmark failed but continuing"
        ;;
    "repeated_interests")
        run_repeated_interests || echo "Benchmark failed but continuing"
        ;;
    "mtu_prediction_test")
        run_mtu_prediction_test || echo "Benchmark failed but continuing"
        ;;
    "cache_warmup_test")
        run_cache_warmup_test || echo "Benchmark failed but continuing"
        ;;
    "udcn_fallback_test")
        run_udcn_fallback_test || echo "Benchmark failed but continuing"
        sleep 5
        run_controller_fallback_test
        ;;
    *)
        echo "Unknown benchmark type: $BENCHMARK_TYPE, using default constant_interest_flood"
        run_constant_interest_flood || echo "Benchmark failed but continuing"
        ;;
esac

echo "Benchmark completed or failed. Keeping container alive for debugging."
sleep infinity

# Keep container running to allow data collection
while true; do
    sleep 10
done
