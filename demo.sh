#!/bin/bash
# μDCN Comprehensive Demo Script
# This script demonstrates the full pipeline from Interest packets through XDP, QUIC, ML prediction and back

# Exit on any error
set -e

# Configuration
INTERFACE="veth0"
PEER_INTERFACE="veth1"
RUST_SERVER_PORT=50051
PYTHON_CONTROLLER_PORT=8000
PACKET_COUNT=10
DEMO_DURATION=60
LOG_DIR="demo_logs"

# Text formatting
BOLD="\e[1m"
RED="\e[31m"
GREEN="\e[32m"
YELLOW="\e[33m"
BLUE="\e[34m"
RESET="\e[0m"

# Create log directory
mkdir -p $LOG_DIR

echo -e "${BOLD}${BLUE}===========================================${RESET}"
echo -e "${BOLD}${BLUE}      μDCN Comprehensive Demo Script      ${RESET}"
echo -e "${BOLD}${BLUE}===========================================${RESET}"

# Step 1: Build all components
echo -e "\n${BOLD}${GREEN}[1/7] Building all components...${RESET}"
# Build XDP components
cd ebpf_xdp
make clean > /dev/null
make > /dev/null
echo "✓ XDP components built successfully"

# Build Rust components
cd ../rust_ndn_transport
cargo build --release > /dev/null
echo "✓ Rust components built successfully"

# Ensure Python dependencies
cd ../python_client
echo "✓ Python components verified"
cd ..

# Step 2: Set up virtual network interfaces for testing
echo -e "\n${BOLD}${GREEN}[2/7] Setting up virtual network...${RESET}"
# Check if interfaces already exist
if ip link show $INTERFACE >/dev/null 2>&1; then
    sudo ip link del $INTERFACE >/dev/null 2>&1 || true
fi

# Create veth pair
sudo ip link add $INTERFACE type veth peer name $PEER_INTERFACE
sudo ip link set dev $INTERFACE up
sudo ip link set dev $PEER_INTERFACE up
sudo ip addr add 192.168.100.1/24 dev $INTERFACE
sudo ip addr add 192.168.100.2/24 dev $PEER_INTERFACE
echo "✓ Virtual network interfaces created"

# Step 3: Start Rust QUIC + gRPC server
echo -e "\n${BOLD}${GREEN}[3/7] Starting Rust QUIC + gRPC server...${RESET}"
cd rust_ndn_transport
./target/release/udcn_server --port $RUST_SERVER_PORT > ../$LOG_DIR/rust_server.log 2>&1 &
RUST_SERVER_PID=$!
echo "✓ Rust server started (PID: $RUST_SERVER_PID)"
cd ..
sleep 2  # Wait for server to start

# Step 4: Start Python controller
echo -e "\n${BOLD}${GREEN}[4/7] Starting Python controller...${RESET}"
cd python_client
python3 controller.py --grpc-port $RUST_SERVER_PORT --web-port $PYTHON_CONTROLLER_PORT > ../$LOG_DIR/python_controller.log 2>&1 &
PYTHON_CONTROLLER_PID=$!
echo "✓ Python controller started (PID: $PYTHON_CONTROLLER_PID)"
cd ..
sleep 2  # Wait for controller to start

# Step 5: Load XDP program
echo -e "\n${BOLD}${GREEN}[5/7] Loading enhanced XDP program...${RESET}"
cd ebpf_xdp
sudo ./ndn_xdp_loader_v2 -i $INTERFACE > ../$LOG_DIR/xdp_loader.log 2>&1 &
XDP_LOADER_PID=$!
echo "✓ XDP program loaded"
cd ..
sleep 1

# Step 6: Generate and send Interest packets
echo -e "\n${BOLD}${GREEN}[6/7] Sending test Interest packets...${RESET}"
cd python_client
python3 test_client.py --interface $PEER_INTERFACE --count $PACKET_COUNT > ../$LOG_DIR/test_client.log 2>&1 &
TEST_CLIENT_PID=$!
echo "✓ Test client started, sending $PACKET_COUNT Interest packets"
cd ..

# Step 7: Monitor the system and display results
echo -e "\n${BOLD}${GREEN}[7/7] Monitoring system performance...${RESET}"
echo -e "${YELLOW}Watching system for $DEMO_DURATION seconds...${RESET}"

# Start monitoring scripts
cd ebpf_xdp
sudo ./monitor_xdp_stats.sh $INTERFACE > ../$LOG_DIR/xdp_stats.log 2>&1 &
MONITOR_XDP_PID=$!
cd ..

# Wait for the specified duration
counter=0
while [ $counter -lt $DEMO_DURATION ]; do
    echo -ne "\r${YELLOW}Demo running: ${counter}/${DEMO_DURATION} seconds${RESET}"
    sleep 1
    counter=$((counter+1))
done

echo -e "\n\n${BOLD}${BLUE}===========================================${RESET}"
echo -e "${BOLD}${BLUE}              Demo Results                 ${RESET}"
echo -e "${BOLD}${BLUE}===========================================${RESET}"

# Display XDP statistics
echo -e "\n${BOLD}${GREEN}XDP Performance Statistics:${RESET}"
tail -10 $LOG_DIR/xdp_stats.log

# Display MTU predictions
echo -e "\n${BOLD}${GREEN}ML-based MTU Predictions:${RESET}"
tail -10 $LOG_DIR/python_controller.log | grep "MTU prediction"

# Display overall pipeline performance
echo -e "\n${BOLD}${GREEN}Overall Pipeline Performance:${RESET}"
echo "Interests processed: $(grep "Interest received" $LOG_DIR/rust_server.log | wc -l)"
echo "Data packets sent: $(grep "Data sent" $LOG_DIR/rust_server.log | wc -l)"
echo "Cache hit rate: $(grep "Cache hit" $LOG_DIR/xdp_stats.log | tail -1 | cut -d':' -f2 | tr -d ' ')%"
echo "Average latency: $(grep "Avg processing time" $LOG_DIR/xdp_stats.log | tail -1 | cut -d':' -f2 | tr -d ' ')"

# Cleanup
echo -e "\n${BOLD}${GREEN}Cleaning up...${RESET}"
sudo kill -9 $RUST_SERVER_PID $PYTHON_CONTROLLER_PID $XDP_LOADER_PID $TEST_CLIENT_PID $MONITOR_XDP_PID >/dev/null 2>&1 || true
sudo ip link set dev $INTERFACE xdp off >/dev/null 2>&1 || true
sudo ip link del $INTERFACE >/dev/null 2>&1 || true

echo -e "\n${BOLD}${GREEN}Demo completed successfully!${RESET}"
echo -e "Log files are available in the ${YELLOW}$LOG_DIR${RESET} directory"
echo -e "${BOLD}${BLUE}===========================================${RESET}"
