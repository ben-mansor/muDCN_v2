# μDCN Benchmark Testbed

This directory contains the Docker-based benchmarking environment for testing and evaluating the μDCN architecture under various network conditions and workloads.

## Overview

The benchmark testbed simulates real-world deployment scenarios for the μDCN system, including:

- Multiple client containers generating Interest traffic
- Server container running the complete μDCN stack (XDP, Rust, Controller)
- Configurable network conditions (latency, packet loss, bandwidth)
- Comprehensive metrics collection and visualization

## Components

- **Server Container**: Runs the full μDCN stack with XDP data plane and Rust transport layer
- **Client Containers**: Generate different types of Interest traffic patterns
- **Metrics Collector**: Aggregates data and generates visualizations
- **Network Configuration**: Uses `tc/netem` to simulate real-world network conditions

## Benchmark Scenarios

The testbed includes the following predefined benchmark scenarios:

1. **Constant Interest Flood**: Stress test with continuous high-rate Interest packets
2. **Repeated Interests**: Test cache efficiency with repeated content requests
3. **MTU Prediction Test**: Evaluate ML-based MTU prediction under varying network conditions
4. **Cache Warmup Test**: Compare performance between cold start and warmed cache
5. **Controller Failure Test**: Measure resilience during controller unavailability

## Running the Benchmarks

### Prerequisites

- Docker 20.10+
- Docker Compose v2+
- 8GB+ RAM
- Linux host with container networking support

### Quick Start

1. Build and start the testbed:

```bash
cd /home/ben/Downloads/date_5_5_2025
docker-compose up --build -d
```

2. Run all benchmarks sequentially:

```bash
docker-compose exec client1 /usr/local/bin/client_start.sh all
```

3. Access the results dashboard:

```bash
# Open in your browser
http://localhost:8080
```

### Running Individual Benchmarks

To run specific benchmark scenarios:

```bash
# For constant Interest flood test
docker-compose exec client1 env BENCHMARK_TYPE=constant_interest_flood /usr/local/bin/client_start.sh

# For repeated interests test
docker-compose exec client1 env BENCHMARK_TYPE=repeated_interests /usr/local/bin/client_start.sh

# For MTU prediction test
docker-compose exec client1 env BENCHMARK_TYPE=mtu_prediction_test /usr/local/bin/client_start.sh

# For cache warmup test  
docker-compose exec client1 env BENCHMARK_TYPE=cache_warmup_test /usr/local/bin/client_start.sh

# For controller fallback test
docker-compose exec client1 env BENCHMARK_TYPE=controller_fallback_test /usr/local/bin/client_start.sh
```

### Custom Network Conditions

You can customize network conditions for each client:

```bash
# Example: Set high latency and packet loss for client2
docker-compose exec client2 tc qdisc change dev eth0 parent 1:12 netem delay 100ms loss 5%

# Check current network settings
docker-compose exec client2 tc qdisc show dev eth0
```

## Collected Metrics

The benchmark system collects and visualizes the following metrics:

- **Server-side**: 
  - Cache hit/miss ratio
  - End-to-end latency
  - CPU and memory usage
  - MTU predictions
  - Packet drop events

- **Client-side**:
  - Round-trip time (RTT)
  - Success/failure rates
  - Data transfer sizes
  - Throughput

## Analyzing Results

The metrics collector automatically processes the collected data and generates visualizations:

1. **Dashboard**: Access at http://localhost:8080
2. **Raw Data**: Available in CSV format at `./docker/metrics/`
3. **Plot Images**: Stored in `./docker/benchmark/`
4. **Summary Report**: JSON and text format in `./docker/benchmark/summary_report.json`

### Key Visualizations

- **Cache Performance**: Cache hit ratio over time
- **Latency Distribution**: RTT histograms by benchmark type
- **MTU Prediction**: Actual vs. predicted MTU values
- **Cache Warmup**: Cold vs. warm cache performance comparison
- **Controller Fallback**: Performance during normal operation vs. fallback mode

## Extending the Testbed

### Adding New Benchmark Scenarios

1. Add new benchmark type to `client_start.sh`
2. Modify the `docker-compose.yml` to configure client parameters
3. Update the analysis script to process new metrics

### Custom Network Topologies

For more complex network topologies:

1. Create a custom Docker network configuration
2. Adjust the `docker-compose.yml` network definitions
3. Update `tc` commands to reflect the new topology

## Troubleshooting

### Common Issues

- **Container fails to start**: Check resource allocation in Docker settings
- **Network configuration errors**: Verify that the container has NET_ADMIN capability
- **Missing metrics**: Ensure volume mounts are properly configured
- **Dashboard not showing**: Verify the metrics collector container is running

### Logs Access

```bash
# View server logs
docker-compose logs server

# View client logs
docker-compose logs client1 client2 client3

# View metrics collector logs
docker-compose logs metrics_collector
```

## Further Reading

- [μDCN Architecture Documentation](../docs/architecture.md)
- [QUIC Transport Implementation](../rust_ndn_transport/README.md)
- [XDP Data Plane Details](../ebpf_ndn/README.md)
