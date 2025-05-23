# μDCN Project Directory Map

This document provides a classification of all directories in the μDCN project codebase, indicating which components are active, deprecated, or optional.

## Legend
- ✅ Actively used in final implementation
- ❌ Deprecated / unused
- ⚠️ Optional / archive if needed

## Core Components

`/docker/` ✅ – Docker configurations for all μDCN containers, used in the primary implementation
`/docker-compose.yml` ✅ – Main deployment configuration for Docker-based implementation
`/demo.sh` ✅ – Main script for running benchmarks and demonstrations
`/DEMO_INSTRUCTIONS.md` ✅ – User guide for running the demonstration

`/ebpf_ndn/` ✅ – Core eBPF/XDP implementations for NDN packet processing
`/ebpf_xdp/` ✅ – Standalone XDP programs for high-performance packet handling

`/rust_ndn_transport/` ✅ – Rust implementation of the NDN transport layer with QUIC
`/proto/` ✅ – Protocol buffer definitions for component communication

`/python_control_plane/` ✅ – ML-based control plane implementation
`/python_client/` ✅ – Client libraries and benchmark tools
`/ml_models/` ✅ – Trained ML models for adaptive caching and forwarding

`/Makefile` ✅ – Build system for the entire project

## Deployment & Testing

`/k8s/` ⚠️ – Kubernetes deployment configurations (future work/experimental)
`/deployment/` ✅ – Scripts and configurations for different deployment scenarios
`/testbed/` ✅ – Test environment configurations and validation scripts

## Documentation & Research

`/docs/` ✅ – Documentation, design specifications, and thesis materials
`/IEEE_OJCOMS-template-LaTex_202401/` ✅ – Paper template and publication materials

## Results & Analysis

`/results/` ✅ – Storage for benchmark results and performance data
`/visualization_plots/` ✅ – Scripts for generating visualizations from results

## Experimental & Development

`/quic_ndn_test/` ⚠️ – Testing utilities for QUIC-based NDN transport (development only)
`/archive/` ❌ – Deprecated code and outdated implementations

## Build Artifacts

`/rust_ndn_transport/target/` ❌ – Build artifacts from Rust compilation (not part of source)

## Project Management

`/PROGRESS.md` ⚠️ – Development progress tracking (for developers only)
`/install_docker.sh` ✅ – Helper script for environment setup

## Packaging

`/udcn_project_v1.0-final.zip` ⚠️ – Project archive (backup/distribution only)
