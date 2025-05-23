#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

REGISTRY=${REGISTRY:-"localhost:5000"}
TAG=${TAG:-"latest"}

echo -e "${GREEN}Building μDCN container images for Kubernetes deployment...${NC}"

# Build Transport Layer Container
echo -e "${YELLOW}Building Transport Layer Image...${NC}"
docker build -t ${REGISTRY}/udcn-transport:${TAG} -f containers/Dockerfile.transport ../

# Build ML Controller Container
echo -e "${YELLOW}Building ML Controller Image...${NC}"
docker build -t ${REGISTRY}/udcn-controller:${TAG} -f containers/Dockerfile.controller ../

# Build Benchmark Client Container
echo -e "${YELLOW}Building Benchmark Client Image...${NC}"
docker build -t ${REGISTRY}/udcn-benchmark:${TAG} -f containers/Dockerfile.benchmark ../

echo -e "${GREEN}Container builds complete!${NC}"

# Push to registry if specified
if [ "$PUSH" = "true" ]; then
    echo -e "${YELLOW}Pushing images to registry ${REGISTRY}...${NC}"
    docker push ${REGISTRY}/udcn-transport:${TAG}
    docker push ${REGISTRY}/udcn-controller:${TAG}
    docker push ${REGISTRY}/udcn-benchmark:${TAG}
    echo -e "${GREEN}Images pushed to registry!${NC}"
fi

echo
echo "To use these images with Kubernetes:"
echo "1. Update the image names in the Kubernetes YAML files:"
echo "   - udcn-transport-daemonset.yaml"
echo "   - udcn-controller-deployment.yaml"
echo "   - udcn-client-deployment.yaml"
echo
echo "2. Apply the configurations using ./deploy_udcn.sh"
