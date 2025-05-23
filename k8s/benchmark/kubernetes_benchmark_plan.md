# μDCN Kubernetes Benchmark Plan

This document outlines a comprehensive approach to benchmarking μDCN in a Kubernetes environment, providing standardized methodology and metrics collection procedures.

## Benchmark Objectives

1. **Performance Comparison**
   - Compare μDCN performance metrics between Docker and Kubernetes deployments
   - Measure overhead (if any) introduced by Kubernetes orchestration
   - Identify performance improvements from enhanced networking in Kubernetes

2. **Scalability Testing**
   - Evaluate horizontal scaling capabilities with increasing client loads
   - Measure performance impact of controller replicas (1 to 5)
   - Test capacity with large numbers of simultaneous clients (10 to 1000)

3. **Resilience Evaluation**
   - Measure recovery time after component failures
   - Evaluate performance degradation during disruption events
   - Test adaptability to varying network conditions

4. **Resource Efficiency**
   - Track resource utilization patterns across components
   - Determine optimal resource requests/limits for each component
   - Compare resource efficiency between deployment models

## Test Scenarios

### 1. Baseline Performance

**Setup**: 
- 3-node Kubernetes cluster (each with 8 CPU cores, 32GB RAM)
- 1 replica of controller
- DaemonSet transport layer on all nodes
- Fixed client count (20)

**Tests**:
- Throughput (Mbps) for small (1KB), medium (100KB), and large (10MB) content
- Latency (ms) for various content sizes
- Cache hit rates during steady-state operation
- CPU and memory utilization per component

### 2. Progressive Load Testing

**Setup**:
- Same cluster configuration as baseline
- Progressive increase in client count (10, 50, 100, 250, 500, 1000)
- Fixed content catalog with zipf distribution

**Tests**:
- Maximum sustainable request rate per client count
- Latency percentiles (50th, 95th, 99th) under increasing load
- System stability indicators (error rates, timeouts)
- Resource utilization scaling patterns

### 3. Horizontal Scaling Evaluation

**Setup**:
- Controller scaled to different replica counts (1, 2, 3, 5)
- High client load (500 concurrent clients)
- Mixed workload pattern

**Tests**:
- Throughput per controller replica
- Load distribution across replicas
- Control plane synchronization efficiency
- Adaptation quality metrics with multiple controllers

### 4. Resilience Testing

**Setup**:
- Chaos Mesh scenarios (component failures, network disruptions)
- Various failure patterns (single component, cascading, network partitions)
- Controller failover testing

**Tests**:
- Time to recover normal operation
- Performance during degraded operation
- Success rate during disruption
- Cache consistency after recovery

### 5. Network Configuration Comparison

**Setup**:
- Different CNI configurations (Calico, Flannel, Cilium)
- Various network policy configurations
- MTU optimization testing

**Tests**:
- Network throughput with different CNIs
- Latency impact of network policies
- Packet processing overhead
- Traffic isolation effectiveness

## Metrics Collection

### Core Metrics

| Metric | Description | Collection Method |
|--------|-------------|-------------------|
| Throughput | Data transferred per second | Prometheus (custom metric) |
| Latency | End-to-end request time | Prometheus (histogram) |
| Cache Hit Rate | Percentage of requests served from cache | Prometheus (ratio calculation) |
| Success Rate | Percentage of successful requests | Prometheus (counter) |
| CPU Utilization | Percentage of CPU used per component | Kubernetes metrics |
| Memory Usage | Memory consumption per component | Kubernetes metrics |
| Network I/O | Network traffic volume | Prometheus node_exporter |
| Storage I/O | Storage operations for content | Prometheus node_exporter |

### Advanced Metrics

| Metric | Description | Collection Method |
|--------|-------------|-------------------|
| Adaptation Frequency | ML model adaptation rate | Controller logs + Prometheus |
| MTU Prediction Accuracy | Accuracy of ML predictions | Controller custom metric |
| Packet Processing Rate | XDP packet handling speed | Transport custom metric |
| Control Plane Latency | Time for configuration changes | Distributed tracing |
| Recovery Time | Time to recover from failures | Chaos Mesh + Prometheus |
| Network Policy Impact | Performance impact of policies | Comparative analysis |

## Benchmarking Tooling

1. **Load Generation**
   - Custom μDCN benchmark clients running as pods
   - Configurable request patterns (constant, poisson, bursty)
   - Workload distribution control (uniform, zipf)
   - Automated test sequencing

2. **Metrics Collection**
   - Prometheus server with long-term storage
   - Custom ServiceMonitors for μDCN components
   - Recording rules for derived metrics
   - Metric retention policy for historical comparison

3. **Results Analysis**
   - Grafana dashboards for visualization
   - Automated report generation scripts
   - Statistical analysis of performance data
   - Benchmark result versioning and comparison

4. **Chaos Testing**
   - Chaos Mesh for orchestrated fault injection
   - Scheduled chaos experiments
   - Controlled failure scenarios
   - Automated resilience evaluation

## Benchmark Execution Guide

1. **Preparation**
   ```bash
   # Deploy monitoring stack
   kubectl apply -f udcn-monitoring.yaml
   
   # Deploy μDCN components
   ./deploy_udcn.sh
   
   # Deploy benchmark clients
   kubectl apply -f udcn-client-deployment.yaml
   
   # Wait for all components to be ready
   kubectl -n udcn-system wait --for=condition=ready pod --all
   ```

2. **Baseline Performance Test**
   ```bash
   # Configure for baseline test
   kubectl apply -f benchmark/baseline-config.yaml
   
   # Run the benchmark
   kubectl -n udcn-system exec deploy/udcn-benchmark-client -- python run_benchmark.py --scenario baseline
   
   # Collect results
   kubectl cp udcn-system/$(kubectl get pod -n udcn-system -l app=udcn-benchmark-client -o jsonpath='{.items[0].metadata.name}'):/results/baseline.csv ./results/baseline.csv
   ```

3. **Progressive Load Test**
   ```bash
   # Configure for load test
   kubectl apply -f benchmark/progressive-load-config.yaml
   
   # Run benchmark with increasing clients
   for clients in 10 50 100 250 500 1000; do
     kubectl -n udcn-system exec deploy/udcn-benchmark-client -- python run_benchmark.py --scenario load --clients $clients
   done
   
   # Collect results
   kubectl cp udcn-system/$(kubectl get pod -n udcn-system -l app=udcn-benchmark-client -o jsonpath='{.items[0].metadata.name}'):/results/load-test.csv ./results/load-test.csv
   ```

4. **Horizontal Scaling Test**
   ```bash
   # Scale controller to different replica counts
   for replicas in 1 2 3 5; do
     kubectl -n udcn-system scale deployment udcn-controller --replicas=$replicas
     kubectl -n udcn-system wait --for=condition=available deployment/udcn-controller
     kubectl -n udcn-system exec deploy/udcn-benchmark-client -- python run_benchmark.py --scenario scaling --replicas $replicas
   done
   
   # Collect results
   kubectl cp udcn-system/$(kubectl get pod -n udcn-system -l app=udcn-benchmark-client -o jsonpath='{.items[0].metadata.name}'):/results/scaling-test.csv ./results/scaling-test.csv
   ```

5. **Resilience Test**
   ```bash
   # Apply chaos tests
   kubectl apply -f udcn-chaos-tests.yaml
   
   # Run benchmark during chaos
   kubectl -n udcn-system exec deploy/udcn-benchmark-client -- python run_benchmark.py --scenario resilience
   
   # Collect results
   kubectl cp udcn-system/$(kubectl get pod -n udcn-system -l app=udcn-benchmark-client -o jsonpath='{.items[0].metadata.name}'):/results/resilience-test.csv ./results/resilience-test.csv
   ```

## Result Analysis

Results should be analyzed considering:

1. **Performance Metrics**
   - Absolute performance values (throughput, latency)
   - Comparative analysis against Docker deployment
   - Resource efficiency metrics

2. **Scalability Characteristics**
   - Performance vs. scale curves
   - Scaling efficiency (linear vs. non-linear)
   - Resource consumption scaling patterns

3. **Resilience Indicators**
   - Recovery time metrics
   - Performance degradation percentages
   - Availability during disruptions

4. **Operational Insights**
   - Deployment complexity comparison
   - Maintenance overhead assessment
   - Monitoring effectiveness evaluation

## Reporting

The benchmark results should be compiled into a comprehensive report with:

1. **Executive Summary**
   - Key findings and recommendations
   - Performance highlights
   - Decision guidance for deployment model

2. **Detailed Metrics**
   - Complete performance data tables
   - Comparative visualizations
   - Statistical analysis

3. **Kubernetes-Specific Insights**
   - Benefits and trade-offs observed
   - Configuration optimization recommendations
   - Resource allocation guidance

4. **Future Optimization Opportunities**
   - Identified performance bottlenecks
   - Suggested enhancements for Kubernetes deployment
   - Recommended next steps

This benchmark plan provides a systematic approach to evaluating μDCN's performance, scalability, and resilience in a Kubernetes environment, generating actionable insights for optimizing deployment configurations.
