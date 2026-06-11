#!/bin/bash
# k8s/vol-monitor/deploy.sh - One-click deploy script for vol-monitor

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
echo "[0/7] Logging in to ACR..."
if ! docker info 2>&1 | grep -q "$DOCKER_REGISTRY"; then
    echo "Logging in to $DOCKER_REGISTRY..."
    docker login "$DOCKER_REGISTRY" -u "308719298@qq.com" -p "zhangdage2011"
fi

# Step 1: Pull base images (Docker will use configured registry mirrors)
echo "[1/7] Pulling base images..."
docker pull rust:latest || true
docker pull debian:bookworm-slim || true

# Step 2: Enable QEMU for arm64 build
echo "[2/7] Setting up QEMU for arm64 build..."
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes >/dev/null 2>&1 || true

# Step 3: Build and push amd64 image
echo "[3/7] Building amd64 image..."
docker build --platform linux/amd64 -t "$IMAGE_NAME:amd64" -f dockers/vol-monitor.Dockerfile .
echo "Pushing amd64 image..."
docker push "$IMAGE_NAME:amd64"

# Step 4: Build and push arm64 image (using QEMU)
echo "[4/7] Building arm64 image (this may take 10-15 minutes)..."
docker build --platform linux/arm64 -t "$IMAGE_NAME:arm64" -f dockers/vol-monitor.Dockerfile .
echo "Pushing arm64 image..."
docker push "$IMAGE_NAME:arm64"

# Step 5: Create and push manifest list
echo "[5/7] Creating manifest list..."
docker manifest create "$IMAGE_NAME:$VERSION" \
    "$IMAGE_NAME:amd64" \
    "$IMAGE_NAME:arm64"
docker manifest annotate "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:amd64" --arch amd64 --os linux
docker manifest annotate "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:arm64" --arch arm64 --os linux
echo "Pushing manifest list..."
docker manifest push "$IMAGE_NAME:$VERSION"

# Tag as latest if not already
if [ "$VERSION" != "latest" ]; then
    docker manifest create "$IMAGE_NAME:latest" \
        "$IMAGE_NAME:amd64" \
        "$IMAGE_NAME:arm64"
    docker manifest annotate "$IMAGE_NAME:latest" "$IMAGE_NAME:amd64" --arch amd64 --os linux
    docker manifest annotate "$IMAGE_NAME:latest" "$IMAGE_NAME:arm64" --arch arm64 --os linux
    docker manifest push "$IMAGE_NAME:latest"
fi

# Step 6: Update Deployment image tag
echo "[6/7] Updating Deployment image tag..."
sed -i.bak "s|image: .*/vol-monitor:.*|image: $IMAGE_NAME:$VERSION|" "$K8S_DIR/deployment.yaml"
rm -f "$K8S_DIR/deployment.yaml.bak"

# Step 7: Apply Kubernetes manifests
echo "[7/7] Applying Kubernetes manifests..."
kubectl apply -f "$K8S_DIR/../namespace.yaml"
kubectl apply -f "$K8S_DIR/configmap.yaml"
kubectl apply -f "$K8S_DIR/deployment.yaml"

# Wait for deployment
echo "Waiting for deployment to complete..."
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
