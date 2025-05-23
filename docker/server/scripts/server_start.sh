#!/bin/bash
set -e

# Set up environment
export LOG_LEVEL=${LOG_LEVEL:-info}
export METRICS_INTERVAL=${METRICS_INTERVAL:-1000}
export CACHE_SIZE=${CACHE_SIZE:-10000}
export MTU_PREDICTION_ENABLED=${MTU_PREDICTION_ENABLED:-true}
export METRICS_DIR=/app/metrics

# Configure interface for XDP
echo "Configuring network for XDP..."
ip link set dev eth0 up
ethtool -L eth0 combined 4 || true  # Set combined queues if supported

# Start XDP data plane
echo "Starting XDP data plane..."
mkdir -p /sys/fs/bpf
ndn_xdp -i eth0 --pinned-path /sys/fs/bpf/ndn_xdp &
XDP_PID=$!
echo "XDP data plane started with PID: $XDP_PID"

# Start Rust gRPC control plane
echo "Starting gRPC server..."
grpc_server --addr 0.0.0.0:9090 --cache-size $CACHE_SIZE --metrics-interval $METRICS_INTERVAL &
GRPC_PID=$!
echo "gRPC server started with PID: $GRPC_PID"

# Start node service for NDN transport
echo "Starting NDN node..."
node --bind-addr 0.0.0.0 --port 9000 --quic --mtu-prediction $MTU_PREDICTION_ENABLED &
NODE_PID=$!
echo "NDN node started with PID: $NODE_PID"

# Start metrics collection
echo "Starting metrics collection..."
mkdir -p $METRICS_DIR
touch $METRICS_DIR/server_metrics.csv
echo "timestamp,cpu_usage,memory_usage,cache_hits,cache_misses,cache_hit_ratio,avg_latency_ms,mtu_predictions,packet_drops" > $METRICS_DIR/server_metrics.csv

# Define metric collection function
collect_metrics() {
    TIMESTAMP=$(date +%s%3N)  # Milliseconds since epoch
    
    # Get CPU and memory usage
    CPU_USAGE=$(top -bn1 | grep "Cpu(s)" | sed "s/.*, *\([0-9.]*\)%* id.*/\1/" | awk '{print 100 - $1}')
    MEMORY_USAGE=$(free -m | awk '/Mem/{print $3}')
    
    # Extract metrics from our services
    # In a real implementation, these would come from the μDCN services
    # For this demo, we'll generate synthetic data based on time
    CACHE_HITS=$(expr $TIMESTAMP % 1000)
    CACHE_MISSES=$(expr $TIMESTAMP % 500)
    TOTAL_CACHE=$(expr $CACHE_HITS + $CACHE_MISSES)
    
    if [ $TOTAL_CACHE -eq 0 ]; then
        CACHE_HIT_RATIO=0
    else
        CACHE_HIT_RATIO=$(echo "scale=4; $CACHE_HITS / $TOTAL_CACHE" | bc)
    fi
    
    AVG_LATENCY=$(expr $TIMESTAMP % 100)
    MTU_PREDICTIONS=$(expr $TIMESTAMP % 10)
    PACKET_DROPS=$(expr $TIMESTAMP % 50)
    
    # Write to CSV
    echo "$TIMESTAMP,$CPU_USAGE,$MEMORY_USAGE,$CACHE_HITS,$CACHE_MISSES,$CACHE_HIT_RATIO,$AVG_LATENCY,$MTU_PREDICTIONS,$PACKET_DROPS" >> $METRICS_DIR/server_metrics.csv
}

# Collect metrics every second
while true; do
    collect_metrics
    sleep 1
done &
METRICS_PID=$!

# Handle shutdown gracefully
trap 'kill $XDP_PID $GRPC_PID $NODE_PID $METRICS_PID; exit 0' SIGTERM SIGINT

# Wait for all processes
wait
