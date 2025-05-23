#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}Deploying μDCN to Kubernetes...${NC}"

# Check if namespace exists, create if needed
if ! kubectl get namespace udcn-system &> /dev/null; then
    echo -e "${YELLOW}Creating udcn-system namespace...${NC}"
    kubectl create namespace udcn-system
fi

# Apply all configuration
echo -e "${YELLOW}Applying Kubernetes configurations...${NC}"

echo "1. Creating services..."
kubectl apply -f udcn-services.yaml

echo "2. Deploying transport layer (DaemonSet)..."
kubectl apply -f udcn-transport-daemonset.yaml

echo "3. Deploying ML controller..."
kubectl apply -f udcn-controller-deployment.yaml

echo "4. Setting up network policies..."
kubectl apply -f udcn-network-policies.yaml || {
    echo -e "${RED}Warning: Could not apply network policies. This may indicate Calico is not properly installed.${NC}"
    echo -e "${YELLOW}Continuing with deployment...${NC}"
}

echo "5. Setting up monitoring..."
kubectl apply -f udcn-monitoring.yaml || {
    echo -e "${RED}Warning: Could not apply monitoring configuration. This may indicate Prometheus Operator is not properly installed.${NC}"
    echo -e "${YELLOW}Continuing with deployment...${NC}"
}

echo "6. Deploying benchmark clients..."
kubectl apply -f udcn-client-deployment.yaml

# Wait for deployments to be ready
echo -e "${YELLOW}Waiting for deployments to be ready...${NC}"
kubectl -n udcn-system rollout status daemonset/udcn-transport
kubectl -n udcn-system rollout status deployment/udcn-controller
kubectl -n udcn-system rollout status deployment/udcn-benchmark-client

echo -e "${GREEN}μDCN deployment complete!${NC}"
echo
echo "To view the status of the components, run:"
echo "  kubectl get pods -n udcn-system"
echo
echo "To check logs from the transport layer, run:"
echo "  kubectl logs -n udcn-system -l app=udcn-transport"
echo
echo "To check logs from the controller, run:"
echo "  kubectl logs -n udcn-system -l app=udcn-controller"
echo
echo "To port-forward Prometheus UI (if installed):"
echo "  kubectl port-forward -n monitoring svc/prometheus-k8s 9090:9090"
echo
echo "To port-forward Grafana (if installed):"
echo "  kubectl port-forward -n monitoring svc/grafana 3000:3000"
