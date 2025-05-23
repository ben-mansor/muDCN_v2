# μDCN Kubernetes Deployment

This directory contains the configuration files needed to deploy the μDCN (micro Data-Centric Networking) system on Kubernetes. This deployment is designed for exploratory and future work purposes, showing how μDCN could be deployed in a production-grade environment.

## Architecture Overview

The Kubernetes deployment consists of the following components:

1. **Transport Layer (DaemonSet)** - Runs the Rust+XDP implementation on each node
2. **ML Controller (Deployment)** - Handles ML-based network adaptation
3. **Benchmark Clients (Deployment)** - Test clients for performance measurement
4. **Calico CNI** - Provides advanced networking capabilities
5. **Prometheus + Grafana** - Monitoring and visualization
6. **Istio Service Mesh** - Enhanced traffic management and observability
7. **Jaeger Distributed Tracing** - End-to-end request tracing
8. **Chaos Mesh** - Resilience testing through controlled fault injection
9. **Horizontal Pod Autoscaler** - Automatic scaling based on resource utilization

## Directory Contents

### Core Deployment Files
- `setup_minikube.sh` - Script to set up a local Minikube environment
- `deploy_udcn.sh` - Main deployment script for μDCN components
- `build_containers.sh` - Script to build and push container images

### Component Configuration
- `udcn-transport-daemonset.yaml` - Transport layer configuration
- `udcn-controller-deployment.yaml` - ML Controller configuration
- `udcn-client-deployment.yaml` - Benchmark clients configuration
- `udcn-services.yaml` - Service definitions for component communication
- `udcn-network-policies.yaml` - Calico CNI network policies
- `udcn-autoscaling.yaml` - Horizontal Pod Autoscaler configuration

### Monitoring and Observability
- `udcn-monitoring.yaml` - Prometheus monitoring configuration
- `udcn-grafana-dashboard.json` - Grafana dashboard for μDCN metrics
- `udcn-istio-integration.yaml` - Istio service mesh configuration
- `udcn-jaeger-tracing.yaml` - Distributed tracing setup

### Resilience Testing
- `udcn-chaos-tests.yaml` - Chaos Mesh tests for resilience testing

### Container Definitions
- `containers/Dockerfile.transport` - Container build for transport layer
- `containers/Dockerfile.controller` - Container build for ML controller
- `containers/Dockerfile.benchmark` - Container build for benchmark client

### Benchmarking
- `benchmark/kubernetes_benchmark_plan.md` - Comprehensive benchmarking methodology
- `benchmark/baseline-config.yaml` - Baseline performance test configuration
- `benchmark/progressive-load-config.yaml` - Load testing configuration
- `benchmark/visualization_template.md` - Results visualization guidance

## Deployment Instructions

### Prerequisites

- Kubernetes cluster (Minikube, K3s, or a production cluster)
- kubectl installed and configured
- Calico CNI installed (optional, but recommended)
- Prometheus Operator installed (optional, for monitoring)
- Chaos Mesh installed (optional, for resilience testing)

### Local Development Setup (Minikube)

To set up a local development environment using Minikube:

```bash
# Make scripts executable
chmod +x setup_minikube.sh deploy_udcn.sh

# Set up Minikube with required components
./setup_minikube.sh

# Wait for all components to be ready
```

### Deploying μDCN

To deploy μDCN to your Kubernetes cluster:

```bash
# Deploy all components
./deploy_udcn.sh
```

This script will:
1. Create the necessary namespace
2. Deploy the transport layer, controller, and benchmark clients
3. Configure services and network policies
4. Set up monitoring
5. Wait for all components to be ready

### Verifying the Deployment

Check that all pods are running:

```bash
kubectl get pods -n udcn-system
```

You should see pods for the transport layer, controller, and benchmark clients.

## Monitoring

### Accessing Prometheus Metrics

To view μDCN metrics in Prometheus:

```bash
# Port-forward Prometheus UI
kubectl port-forward -n monitoring svc/prometheus-k8s 9090:9090
```

Then open `http://localhost:9090` in your browser.

### Accessing Grafana Dashboard

To view the μDCN dashboard in Grafana:

```bash
# Port-forward Grafana
kubectl port-forward -n monitoring svc/grafana 3000:3000
```

Then open `http://localhost:3000` in your browser and import the dashboard from `udcn-grafana-dashboard.json`.

## Resilience Testing

To test the resilience of μDCN, you can use the provided Chaos Mesh configurations:

```bash
# Apply chaos tests
kubectl apply -f udcn-chaos-tests.yaml
```

This will periodically:
- Simulate controller failures
- Simulate transport layer failures
- Introduce network delays
- Introduce packet loss

## Collecting Benchmark Results

To collect benchmark results:

```bash
# Get a list of client pods
kubectl get pods -n udcn-system -l app=udcn-benchmark-client

# Copy benchmark results from a client pod
kubectl cp udcn-system/<pod-name>:/results/benchmark_results.csv ./benchmark_results.csv
```

## Cleanup

To remove the μDCN deployment:

```bash
# Delete the udcn-system namespace
kubectl delete namespace udcn-system

# If you've installed Chaos Mesh for testing only
kubectl delete namespace chaos-testing
```

## Future Enhancements

Potential enhancements to this Kubernetes deployment:
1. Helm chart for easier deployment and configuration
2. Auto-scaling based on traffic load
3. Integration with service mesh (like Istio)
4. Multi-cluster federation for edge-to-cloud scenarios
5. Enhanced security with pod security policies and network encryption

## Troubleshooting

### Common Issues

1. **Transport layer pods crash**: Check if the DaemonSet has sufficient privileges for eBPF/XDP operations.
2. **Controller can't connect to transport**: Verify service names and network policies.
3. **No metrics in Prometheus**: Ensure ServiceMonitor is correctly configured and the Prometheus Operator is installed.

For logs:
```bash
kubectl logs -n udcn-system -l app=udcn-transport
kubectl logs -n udcn-system -l app=udcn-controller
```

## Integration with Thesis

This Kubernetes deployment is intended as exploratory work and can be documented in the thesis under a "Future Work" section. It demonstrates how μDCN could be deployed in a production environment, beyond the current Docker-based implementation described in the main thesis.
