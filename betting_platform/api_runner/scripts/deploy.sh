#!/bin/bash

# Production deployment script for Betting Platform API
# Usage: ./deploy.sh [environment] [version]
# Example: ./deploy.sh production v1.2.3

set -euo pipefail

# Configuration
ENVIRONMENT=${1:-staging}
VERSION=${2:-latest}
REGISTRY="your-registry.com"
IMAGE_NAME="betting-platform-api"
NAMESPACE="betting-platform"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Betting Platform API Deployment${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "Environment: ${YELLOW}$ENVIRONMENT${NC}"
echo -e "Version: ${YELLOW}$VERSION${NC}"
echo ""

# Function to check prerequisites
check_prerequisites() {
    echo -e "${YELLOW}Checking prerequisites...${NC}"
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Docker is not installed${NC}"
        exit 1
    fi
    
    # Check kubectl
    if ! command -v kubectl &> /dev/null; then
        echo -e "${RED}kubectl is not installed${NC}"
        exit 1
    fi
    
    # Check if logged in to registry
    if ! docker info 2>/dev/null | grep -q "Username"; then
        echo -e "${YELLOW}Not logged in to Docker registry. Logging in...${NC}"
        docker login $REGISTRY
    fi
    
    echo -e "${GREEN}Prerequisites check passed${NC}"
}

# Function to run tests
run_tests() {
    echo -e "${YELLOW}Running tests...${NC}"
    
    # Run unit tests
    cargo test --release
    
    # Run integration tests
    cargo test --test '*' -- --test-threads=1
    
    echo -e "${GREEN}All tests passed${NC}"
}

# Function to build and push Docker image
build_and_push() {
    echo -e "${YELLOW}Building Docker image...${NC}"
    
    # Build the image
    docker build -t $REGISTRY/$IMAGE_NAME:$VERSION -f docker/Dockerfile .
    
    # Tag as latest if deploying to production
    if [ "$ENVIRONMENT" == "production" ]; then
        docker tag $REGISTRY/$IMAGE_NAME:$VERSION $REGISTRY/$IMAGE_NAME:latest
    fi
    
    # Push to registry
    echo -e "${YELLOW}Pushing image to registry...${NC}"
    docker push $REGISTRY/$IMAGE_NAME:$VERSION
    
    if [ "$ENVIRONMENT" == "production" ]; then
        docker push $REGISTRY/$IMAGE_NAME:latest
    fi
    
    echo -e "${GREEN}Image pushed successfully${NC}"
}

# Function to update Kubernetes deployment
deploy_to_k8s() {
    echo -e "${YELLOW}Deploying to Kubernetes...${NC}"
    
    # Check if namespace exists
    if ! kubectl get namespace $NAMESPACE &> /dev/null; then
        echo -e "${YELLOW}Creating namespace $NAMESPACE...${NC}"
        kubectl create namespace $NAMESPACE
    fi
    
    # Update the deployment with new image
    kubectl set image deployment/betting-platform-api \
        api=$REGISTRY/$IMAGE_NAME:$VERSION \
        -n $NAMESPACE
    
    # Wait for rollout to complete
    echo -e "${YELLOW}Waiting for rollout to complete...${NC}"
    kubectl rollout status deployment/betting-platform-api -n $NAMESPACE
    
    # Check pod status
    echo -e "${YELLOW}Checking pod status...${NC}"
    kubectl get pods -n $NAMESPACE -l app=betting-platform-api
    
    echo -e "${GREEN}Deployment completed successfully${NC}"
}

# Function to run post-deployment checks
post_deployment_checks() {
    echo -e "${YELLOW}Running post-deployment checks...${NC}"
    
    # Get service endpoint
    ENDPOINT=$(kubectl get svc betting-platform-api -n $NAMESPACE -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
    
    if [ -z "$ENDPOINT" ]; then
        ENDPOINT=$(kubectl get svc betting-platform-api -n $NAMESPACE -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
    fi
    
    if [ -z "$ENDPOINT" ]; then
        echo -e "${YELLOW}Service endpoint not available yet${NC}"
    else
        # Check health endpoint
        echo -e "${YELLOW}Checking health endpoint...${NC}"
        curl -s http://$ENDPOINT/health | jq .
    fi
    
    # Show logs
    echo -e "${YELLOW}Recent logs:${NC}"
    kubectl logs -n $NAMESPACE -l app=betting-platform-api --tail=20
    
    echo -e "${GREEN}Post-deployment checks completed${NC}"
}

# Function to rollback deployment
rollback() {
    echo -e "${RED}Rolling back deployment...${NC}"
    kubectl rollout undo deployment/betting-platform-api -n $NAMESPACE
    kubectl rollout status deployment/betting-platform-api -n $NAMESPACE
    echo -e "${GREEN}Rollback completed${NC}"
}

# Main deployment flow
main() {
    check_prerequisites
    
    # Only run tests for staging/production deployments
    if [ "$ENVIRONMENT" != "development" ]; then
        run_tests
    fi
    
    build_and_push
    deploy_to_k8s
    post_deployment_checks
    
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}Deployment completed successfully!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo -e "Environment: ${YELLOW}$ENVIRONMENT${NC}"
    echo -e "Version: ${YELLOW}$VERSION${NC}"
    echo -e "Image: ${YELLOW}$REGISTRY/$IMAGE_NAME:$VERSION${NC}"
}

# Run main function with error handling
if main; then
    exit 0
else
    echo -e "${RED}Deployment failed!${NC}"
    read -p "Do you want to rollback? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rollback
    fi
    exit 1
fi