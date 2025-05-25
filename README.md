# μDCN: Micro Data-Centric Networking

## Project Overview

μDCN (Micro Data-Centric Networking) is a novel networking architecture that combines the principles of Named Data Networking (NDN) with high-performance eBPF/XDP packet processing and machine learning-based adaptive control. The project aims to provide efficient content distribution with optimized caching strategies and real-time adaptation to network conditions.

### Core Technologies

- **eBPF/XDP**: High-performance packet processing directly in the Linux kernel
- **Rust/QUIC**: Reliable and secure transport layer with modern protocol features
- **Python/ML**: Intelligent control plane with machine learning-based optimization
- **Named Data Networking (NDN)**: Content-centric networking paradigm focusing on data rather than hosts

## Architecture Versions

### Docker-based Implementation (Primary)

The main implementation uses Docker containers to deploy the complete μDCN stack. This version is thoroughly tested and used for benchmarking and performance evaluation in the thesis.

### Kubernetes-based Deployment (Still in testing)

An exploratory Kubernetes deployment has been designed and prepared as future work. This implementation demonstrates how μDCN could scale in production environments using Kubernetes features like DaemonSets for the transport layer and Deployments for the control plane.

## Docker-Based Instructions

### Prerequisites

- Docker and Docker Compose v2.x
- At least 8GB of RAM and 4 CPU cores recommended

### Building and Running

```bash
# Clone the repository
git clone https://github.com/ben-mansor/muDCN_v2.git
cd muDCN_v2

# Build and run the containers
docker-compose up --build

# In a separate terminal, run the benchmark
./demo.sh
```

### Using This GitHub Repository

This repository contains the complete implementation of the μDCN architecture. Here's how to work with it effectively:

1. **Clone with submodules**: Some components are included as submodules. Use `git clone --recursive` to get everything.

2. **Branches**:
   - `main`: Stable, tested version used for thesis evaluation
   - `kubernetes`: Experimental Kubernetes deployment
   - `develop`: Latest development changes

3. **Issues and Pull Requests**:
   - Use GitHub Issues to report bugs or suggest features
   - Submit Pull Requests for code contributions
   - Check existing issues before creating new ones

4. **Releases**:
   - Tagged releases correspond to thesis versions and major milestones
   - Download release packages for stable snapshots of the codebase

Refer to `DEMO_INSTRUCTIONS.md` for detailed step-by-step instructions on running various benchmark scenarios.

### Metrics and Visualization

Benchmark results are stored in the `results/` directory. Each run creates:

- Raw data in CSV and JSON formats
- Performance metrics (throughput, latency, cache hit ratios)
- Resource utilization statistics

To generate visualization plots from benchmark results:

```bash
python visualization_plots/generate_plots.py --input results/latest_run/ --output results/plots/
```

## Kubernetes Deployment (Future Work)

The Kubernetes deployment is designed as a blueprint for production-scale implementations of μDCN.

### Components

- **Transport Layer**: Deployed as a DaemonSet to ensure one instance per node
- **ML Controller**: Central Deployment managing the network behavior
- **Benchmark Clients**: Deployments for load generation and testing
- **Prometheus & Grafana**: For metrics collection and visualization

### Running on Minikube

```bash
# Start Minikube
minikube start --cpus=4 --memory=8g

# Deploy μDCN components
cd k8s
./deploy_to_minikube.sh

# Access the dashboard
kubectl port-forward -n udcn-system svc/udcn-dashboard 8080:80
```

For detailed information on the Kubernetes deployment, refer to `docs/thesis/k8s_integration_guide.md`.

## Metrics & Visualization

μDCN generates comprehensive metrics in both CSV and JSON formats for flexible analysis and visualization:

- **Throughput**: Measured in packets per second and bytes per second
- **Latency**: Per-packet and average response times
- **Cache Performance**: Hit/miss ratios and content popularity distribution
- **Resource Usage**: CPU, memory, and network utilization

Pre-generated visualization scripts are available in the `visualization_plots/` directory for common analysis tasks.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- The Named Data Networking (NDN) community
- The eBPF and XDP development teams
- All contributors and advisors listed in the thesis acknowledgments
