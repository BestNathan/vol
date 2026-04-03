#!/bin/bash
# k8s/deploy.sh - One-click deploy script for vol-monitor

set -e

# Configuration - Aliyun Container Registry (ACR)
DOCKER_REGISTRY="${DOCKER_REGISTRY:-crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com}"
IMAGE_NAME="${DOCKER_REGISTRY}/n_common/vol-monitor"
VERSION="${1:-latest}"
K8S_DIR="$(dirname "$0")"

echo "============================================"
echo "  vol-monitor Kubernetes Deploy"
echo "============================================"
echo "Registry: $DOCKER_REGISTRY"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Version: $VERSION"
echo ""

# Step 0: Login to ACR
echo "[0/6] Logging in to ACR..."
if ! docker info 2>&1 | grep -q "$DOCKER_REGISTRY"; then
    echo "Logging in to $DOCKER_REGISTRY..."
    docker login "$DOCKER_REGISTRY" -u "308719298@qq.com" -p "zhangdage2011"
fi

# Step 1: Build Docker image
echo "[1/6] Building Docker image..."
docker build -t "$IMAGE_NAME:$VERSION" -f Dockerfile .

# Tag as latest if not already
if [ "$VERSION" != "latest" ]; then
    docker tag "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:latest"
fi

# Step 2: Push to ACR
echo "[2/6] Pushing to ACR..."
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
