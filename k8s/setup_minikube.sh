#!/bin/bash
set -e

echo "Setting up Minikube for μDCN deployment..."

# Start Minikube with sufficient resources
minikube start --cpus=4 --memory=8g --driver=docker \
  --feature-gates="EphemeralContainers=true" \
  --addons=ingress,metrics-server

# Enable Calico CNI in Minikube
echo "Installing Calico CNI..."
kubectl create -f https://raw.githubusercontent.com/projectcalico/calico/v3.26.1/manifests/tigera-operator.yaml
kubectl create -f https://raw.githubusercontent.com/projectcalico/calico/v3.26.1/manifests/custom-resources.yaml

# Wait for Calico to be ready
echo "Waiting for Calico to initialize..."
kubectl wait --namespace=calico-system --for=condition=ready pod --selector=k8s-app=calico-node --timeout=90s

# Create the μDCN namespace
kubectl create namespace udcn-system

# Install Prometheus Operator for monitoring
echo "Installing Prometheus Operator..."
kubectl apply -f https://github.com/prometheus-operator/kube-prometheus/releases/download/v0.12.0/manifests/setup/
kubectl apply -f https://github.com/prometheus-operator/kube-prometheus/releases/download/v0.12.0/manifests/

echo "Minikube setup complete!"
echo "To access the Kubernetes dashboard, run: minikube dashboard"
