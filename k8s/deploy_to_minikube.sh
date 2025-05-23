#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}===== μDCN Kubernetes Deployment Script (Demo Mode) =====${NC}"
echo -e "${YELLOW}This script will deploy μDCN to a Minikube cluster${NC}"

# Step 1: Check if Minikube is installed
if ! command -v minikube &> /dev/null; then
    echo -e "${RED}Minikube is not installed. Please install it first.${NC}"
    exit 1
fi

# Step 2: Start Minikube if not running
if ! minikube status | grep -q "Running"; then
    echo -e "${YELLOW}Starting Minikube with sufficient resources...${NC}"
    minikube start --cpus=4 --memory=8g --driver=docker
else
    echo -e "${GREEN}Minikube is already running${NC}"
fi

# Step 3: Use pre-built sample images (avoiding build issues)
echo -e "${YELLOW}Using demo images for deployment...${NC}"

# In a real deployment, you would build your own images
# For demo purposes, we'll use existing images from Docker Hub

# Step 4: Create namespace if it doesn't exist
echo -e "${YELLOW}Creating namespace...${NC}"
kubectl create namespace udcn-system 2>/dev/null || true

# Step 5: Configure deployment files to use demo images
echo -e "${YELLOW}Updating deployment files to use demo images...${NC}"
# We're already in the k8s directory

# Update transport layer deployment to use nginx as a placeholder
sed -i 's|image: .*|image: nginx:latest|g' udcn-transport-daemonset.yaml

# Update client deployment to use busybox as a placeholder
sed -i 's|image: .*|image: busybox:latest|g' udcn-client-deployment.yaml

# Step 6: Apply Kubernetes configurations for demo deployment
echo -e "${YELLOW}Deploying μDCN demo components to Kubernetes...${NC}"

# Create ConfigMap for simulated μDCN metrics
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: ConfigMap
metadata:
  name: udcn-demo-metrics
  namespace: udcn-system
data:
  demo-metrics.txt: |
    # μDCN DEMO METRICS
    # This represents sample metrics from a μDCN deployment
    cache_hits 2345
    cache_misses 321
    interest_packets_received 5678
    data_packets_sent 5432
    average_latency_ms 12.5
    current_clients 8
    memory_usage_bytes 1258291
    cpu_usage_percent 22.4
EOF

# Apply modified deployment files
kubectl apply -f udcn-transport-daemonset.yaml
kubectl apply -f udcn-services.yaml
kubectl apply -f udcn-client-deployment.yaml

# Wait for pods to be ready
echo -e "${YELLOW}Waiting for pods to be ready...${NC}"
kubectl wait --namespace udcn-system --for=condition=ready pods --all --timeout=120s || true

# Step 7: Show the status of the deployment
echo -e "${GREEN}Demo deployment completed. Current pod status:${NC}"
kubectl get pods -n udcn-system

# Step 8: Instructions for exploring the deployment
echo -e "${YELLOW}\nExploring the μDCN Kubernetes Deployment:${NC}"
echo -e "\n1. View the pods in the μDCN namespace:"
echo -e "   kubectl get pods -n udcn-system"

echo -e "\n2. View the services:"
echo -e "   kubectl get services -n udcn-system"

echo -e "\n3. Access the demo metrics:"
echo -e "   kubectl get configmap udcn-demo-metrics -n udcn-system -o jsonpath='{.data.demo-metrics\.txt}'"

echo -e "\n4. View the DaemonSet (transport layer):"
echo -e "   kubectl describe daemonset -n udcn-system"

echo -e "\n${GREEN}Note: This is a demo deployment with placeholder containers.${NC}"
echo -e "${GREEN}In a production deployment, you would use the actual μDCN container images.${NC}"
echo -e "${GREEN}See the comprehensive guide in docs/thesis/k8s_deployment_guide.md for details.${NC}"

