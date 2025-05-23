# μDCN System Demonstration Guide

This guide provides step-by-step instructions for demonstrating the complete μDCN system, including the ML-based MTU prediction, XDP acceleration, QUIC transport, and gRPC communication.

## Quick Start

To run the full demonstration with a single command:

```bash
cd ebpf_xdp
make demo
```

This will:
1. Build all components
2. Set up a virtual network interface
3. Start the Rust gRPC + QUIC server
4. Launch the Python controller with ML integration
5. Load the enhanced XDP program
6. Send test Interest packets
7. Monitor and display system performance

## Manual Step-by-Step Instructions

If you prefer to run each component manually or want to understand the system in more detail, follow these steps:

### 1. Building Components

#### Build XDP Components
```bash
cd ebpf_xdp
make clean
make
```

#### Build Rust Components
```bash
cd rust_ndn_transport
cargo build --release
```

### 2. Setting up Network Interfaces

Create a virtual network interface pair for testing:

```bash
# Create veth pair
sudo ip link add veth0 type veth peer name veth1
sudo ip link set dev veth0 up
sudo ip link set dev veth1 up

# Assign IP addresses
sudo ip addr add 192.168.100.1/24 dev veth0
sudo ip addr add 192.168.100.2/24 dev veth1
```

### 3. Starting the Rust gRPC + QUIC Server

To start the Rust gRPC server with QUIC transport:

```bash
cd rust_ndn_transport
./target/release/udcn_server --port 50051
```

The server provides the following gRPC services:
- QUIC connection management
- NDN packet forwarding
- XDP program configuration
- MTU prediction API

### 4. Running the Python Controller

To start the Python controller:

```bash
cd python_client
python3 controller.py --grpc-port 50051 --web-port 8000
```

The controller:
- Connects to the Rust gRPC server
- Integrates with the ML predictor for MTU optimization
- Provides a web API for monitoring and configuration
- Collects and displays system metrics

You can access the web interface at:
- `http://localhost:8000/` - Main page
- `http://localhost:8000/status` - System status
- `http://localhost:8000/metrics` - Performance metrics
- `http://localhost:8000/predictions` - Recent MTU predictions

### 5. Attaching the XDP Program

To load the enhanced XDP program to a network interface:

```bash
cd ebpf_xdp
sudo ./ndn_xdp_loader_v2 -i veth0
```

Configuration options:
- `-i` - Specify the interface name
- `-S` - Use SKB mode instead of driver mode
- `-c` - Set content store capacity
- `-t` - Set content TTL in seconds
- `-f` - Set userspace fallback percentage
- `-z` - Disable zero-copy optimization

### 6. Sending Test Packets

To manually send NDN Interest packets:

```bash
cd python_client
sudo python3 test_client.py --interface veth1 --count 10 --interval 1
```

Options:
- `--interface` - Interface to send packets on
- `--count` - Number of packets to send
- `--interval` - Time between packets (seconds)
- `--random` - Use random names instead of sequential

### 7. Verifying System Performance

#### Monitor XDP Statistics

```bash
cd ebpf_xdp
sudo ./monitor_xdp_stats.sh veth0
```

#### View BPF Maps Directly

Using bpftool to inspect the BPF maps:

```bash
# Find map IDs
sudo bpftool map | grep -E "name metrics|content_store"

# Dump metrics map
sudo bpftool map dump id <metrics_map_id>

# Dump content store map
sudo bpftool map dump id <content_store_map_id>
```

#### Monitor System via /sys/fs/bpf

```bash
# List pinned maps
ls -la /sys/fs/bpf/

# If maps are pinned, view metrics
cat /sys/fs/bpf/metrics

# View XDP program info
cat /proc/net/dev
```

#### View Logs

```bash
# View Rust server logs
cat demo_logs/rust_server.log

# View Python controller logs
cat demo_logs/python_controller.log

# View XDP loader logs
cat demo_logs/xdp_loader.log
```

## Tracing the Complete Pipeline

Here's how a packet flows through the entire system:

1. An Interest packet arrives at the network interface (veth0)
2. The XDP program processes the packet at the kernel level:
   - Checks if content is in the kernel content store
   - Makes a decision to serve, drop, or forward to userspace
3. For forwarded packets, the Rust QUIC engine receives the Interest
4. The gRPC server processes the Interest and requests MTU prediction
5. The Python controller:
   - Collects network metrics
   - Uses the ML model to predict optimal MTU
   - Returns the prediction via gRPC
6. The Rust server adjusts packet parameters based on predictions
7. For cache misses, the QUIC engine generates a Data packet
8. The Data packet is sent back through the XDP program
9. The XDP program may cache the content for future requests

## Troubleshooting

### XDP Program Issues

- **Problem**: XDP program fails to load
  **Solution**: Verify interface support with `ip link show <interface>`

- **Problem**: XDP maps not accessible
  **Solution**: Run loader with sudo/root permissions

### Rust Server Issues

- **Problem**: Server fails to start
  **Solution**: Check port availability with `netstat -tuln`

### Python Controller Issues

- **Problem**: gRPC connection fails
  **Solution**: Ensure server is running and port is correct

- **Problem**: ML model not found
  **Solution**: Check that ML model files exist in ml_models directory

### Network Interface Issues

- **Problem**: Packets not flowing through interfaces
  **Solution**: Verify interface state with `ip link show` and `ip addr show`
