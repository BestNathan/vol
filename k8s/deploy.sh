#!/bin/bash
# k8s/deploy.sh - One-click deploy script for vol-monitor

set -e

# Configuration - UPDATE THESE VALUES
DOCKERHUB_USERNAME="${DOCKERHUB_USERNAME:-your-dockerhub-username}"
IMAGE_NAME="${DOCKERHUB_USERNAME}/vol-monitor"
VERSION="${1:-latest}"
K8S_DIR="$(dirname "$0")"

echo "============================================"
echo "  vol-monitor Kubernetes Deploy"
echo "============================================"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Version: $VERSION"
echo ""

# Step 1: Build Docker image
echo "[1/5] Building Docker image..."
docker build -t "$IMAGE_NAME:$VERSION" -f Dockerfile .

# Tag as latest if not already
if [ "$VERSION" != "latest" ]; then
    docker tag "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:latest"
fi

# Step 2: Push to Docker Hub
echo "[2/5] Pushing to Docker Hub..."
docker push "$IMAGE_NAME:$VERSION"
if [ "$VERSION" != "latest" ]; then
    docker push "$IMAGE_NAME:latest"
fi

# Step 3: Update Deployment image tag
echo "[3/5] Updating Deployment image tag..."
sed -i.bak "s|image: .*/vol-monitor:.*|image: $IMAGE_NAME:$VERSION|" "$K8S_DIR/deployment.yaml"
rm -f "$K8S_DIR/deployment.yaml.bak"

# Step 4: Apply Kubernetes manifests
echo "[4/5] Applying Kubernetes manifests..."
kubectl apply -f "$K8S_DIR/namespace.yaml"
kubectl apply -f "$K8S_DIR/configmap.yaml"
kubectl apply -f "$K8S_DIR/deployment.yaml"

# Step 5: Wait for deployment
echo "[5/5] Waiting for deployment to complete..."
kubectl -n deribit rollout status deployment/vol-monitor --timeout=300s

echo ""
echo "============================================"
echo "  Deploy Complete!"
echo "============================================"
echo ""
echo "Pod status:"
kubectl -n deribit get pods -l app=vol-monitor
echo ""
echo "View logs:"
echo "  kubectl -n deribit logs -f deployment/vol-monitor"
echo ""
echo "Update version:"
echo "  $0 v0.1.1"
echo ""
echo "Rollback:"
echo "  kubectl -n deribit rollout undo deployment/vol-monitor"
echo ""
