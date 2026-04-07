#!/bin/bash
# scripts/build-multiarch.sh - Test multi-arch cross-compile build
#
# This script builds and pushes a multi-architecture image using cross-compilation
# Tag: beta - for testing purposes only, not for production deployment
#
# Usage: ./scripts/build-multiarch.sh

set -e

# Configuration - Aliyun Container Registry (ACR)
DOCKER_REGISTRY="${DOCKER_REGISTRY:-crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com}"
IMAGE_NAME="${DOCKER_REGISTRY}/n_common/vol-monitor"
VERSION="beta"
DOCKERFILE="Dockerfile.cross-compile"

echo "============================================"
echo "  vol-monitor Multi-Arch Cross-Compile Build"
echo "============================================"
echo "Registry: $DOCKER_REGISTRY"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Dockerfile: $DOCKERFILE"
echo ""

# Step 0: Login to ACR
echo "[0/5] Logging in to ACR..."
if ! docker info 2>&1 | grep -q "$DOCKER_REGISTRY"; then
    echo "Logging in to $DOCKER_REGISTRY..."
    docker login "$DOCKER_REGISTRY" -u "308719298@qq.com" -p "zhangdage2011"
fi

# Step 1: Create multi-arch builder (if not exists)
echo "[1/5] Setting up multi-arch builder..."
if ! docker buildx ls 2>/dev/null | grep -q "multiarch-builder"; then
    docker buildx create --name multiarch-builder --driver docker-container --use
    docker buildx inspect multiarch-builder --bootstrap
    echo "Created new builder: multiarch-builder"
else
    docker buildx use multiarch-builder
    echo "Using existing builder: multiarch-builder"
fi

# Step 2: Build and push multi-arch image
echo "[2/5] Building multi-arch image (cross-compile)..."
echo "This should take ~5-8 minutes for both architectures..."
docker buildx build --platform linux/amd64,linux/arm64 \
    --push \
    --tag "$IMAGE_NAME:$VERSION" \
    --tag "$IMAGE_NAME:cross-compile-test" \
    -f "$DOCKERFILE" \
    .

echo ""
echo "[3/5] Build complete! Verifying image..."

# Step 3: Verify the multi-arch image
echo "[4/5] Inspecting manifest..."
docker buildx imagetools inspect "$IMAGE_NAME:$VERSION"

echo ""
echo "============================================"
echo "  Build Complete!"
echo "============================================"
echo ""
echo "Image tags:"
echo "  - $IMAGE_NAME:$VERSION"
echo "  - $IMAGE_NAME:cross-compile-test"
echo ""
echo "To test locally (amd64):"
echo "  docker run --rm $IMAGE_NAME:$VERSION --version"
echo ""
echo "To deploy to Kubernetes:"
echo "  kubectl set image deployment/vol-monitor -n deribit vol-monitor=$IMAGE_NAME:$VERSION"
echo ""
echo "To rollback:"
echo "  kubectl rollout undo deployment/vol-monitor -n deribit"
echo ""
