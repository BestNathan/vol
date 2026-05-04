#!/bin/bash
# k8s/deploy-agent-manager.sh - One-click deploy for vol-agent-manager

set -e

# Configuration - Aliyun Container Registry (ACR), same repo as vol-monitor
DOCKER_REGISTRY="${DOCKER_REGISTRY:-crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com}"
IMAGE_NAME="${DOCKER_REGISTRY}/n_common/vol-monitor"
VERSION="${1:-agent-manager-latest}"
K8S_DIR="$(dirname "$0")"

echo "============================================"
echo "  vol-agent-manager Kubernetes Deploy"
echo "============================================"
echo "Registry: $DOCKER_REGISTRY"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Version: $VERSION"
echo ""

# Step 0: Login to ACR
echo "[0/7] Logging in to ACR..."
if ! docker info 2>&1 | grep -q "$DOCKER_REGISTRY"; then
    echo "Logging in to $DOCKER_REGISTRY..."
    docker login "$DOCKER_REGISTRY" -u "308719298@qq.com" -p "zhangdage2011"
fi

# Step 1: Pull base images
echo "[1/7] Pulling base images..."
docker pull rust:latest || true
docker pull debian:bookworm-slim || true

# Step 2: Enable QEMU for arm64 build
echo "[2/7] Setting up QEMU for arm64 build..."
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes >/dev/null 2>&1 || true

# Step 3: Build and push amd64 image
echo "[3/7] Building amd64 image..."
docker build --platform linux/amd64 -t "$IMAGE_NAME:agent-manager-amd64" -f Dockerfile.agent-manager .
echo "Pushing amd64 image..."
docker push "$IMAGE_NAME:agent-manager-amd64"

# Step 4: Build and push arm64 image
echo "[4/7] Building arm64 image (this may take 10-15 minutes)..."
docker build --platform linux/arm64 -t "$IMAGE_NAME:agent-manager-arm64" -f Dockerfile.agent-manager .
echo "Pushing arm64 image..."
docker push "$IMAGE_NAME:agent-manager-arm64"

# Step 5: Create and push manifest list
echo "[5/7] Creating manifest list..."
docker manifest create "$IMAGE_NAME:$VERSION" \
    "$IMAGE_NAME:agent-manager-amd64" \
    "$IMAGE_NAME:agent-manager-arm64"
docker manifest annotate "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:agent-manager-amd64" --arch amd64 --os linux
docker manifest annotate "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:agent-manager-arm64" --arch arm64 --os linux
echo "Pushing manifest list..."
docker manifest push "$IMAGE_NAME:$VERSION"

# Also tag as agent-manager-latest
docker manifest create "$IMAGE_NAME:agent-manager-latest" \
    "$IMAGE_NAME:agent-manager-amd64" \
    "$IMAGE_NAME:agent-manager-arm64"
docker manifest annotate "$IMAGE_NAME:agent-manager-latest" "$IMAGE_NAME:agent-manager-amd64" --arch amd64 --os linux
docker manifest annotate "$IMAGE_NAME:agent-manager-latest" "$IMAGE_NAME:agent-manager-arm64" --arch arm64 --os linux
docker manifest push "$IMAGE_NAME:agent-manager-latest"

# Step 6: Update Deployment image tag
echo "[6/7] Updating Deployment image tag..."
sed -i.bak "s|image: .*/vol-monitor:agent-manager.*|image: $IMAGE_NAME:$VERSION|" "$K8S_DIR/deployment-agent-manager.yaml"
rm -f "$K8S_DIR/deployment-agent-manager.yaml.bak"

# Step 7: Apply Kubernetes manifests
echo "[7/7] Applying Kubernetes manifests..."
kubectl apply -f "$K8S_DIR/namespace.yaml"
kubectl apply -f "$K8S_DIR/deployment-agent-manager.yaml"

# Wait for deployment
echo "Waiting for deployment to complete..."
kubectl -n deribit rollout status deployment/vol-agent-manager --timeout=300s

echo ""
echo "============================================"
echo "  Deploy Complete!"
echo "============================================"
echo ""
echo "Pod status:"
kubectl -n deribit get pods -l app=vol-agent-manager
echo ""
echo "View logs:"
echo "  kubectl -n deribit logs -f deployment/vol-agent-manager"
echo ""
echo "Update version:"
echo "  $0 agent-manager-v0.1.1"
echo ""
echo "Rollback:"
echo "  kubectl -n deribit rollout undo deployment/vol-agent-manager"
echo ""
