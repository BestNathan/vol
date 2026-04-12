#!/bin/bash
# scripts/build-multiarch.sh - Test single-image dual-binary build
#
# This script builds a single image containing BOTH amd64 and arm64 binaries
# The correct binary is selected at runtime based on uname -m
#
# Features:
#   - Cross-compilation for arm64 using gcc-aarch64-linux-gnu
#   - Single image works on both architectures
#   - No QEMU emulation needed during build (native cross-compile)
#   - Tag: beta - for testing purposes only
#
# Usage: ./scripts/build-multiarch.sh

set -e

# Configuration - Aliyun Container Registry (ACR)
DOCKER_REGISTRY="${DOCKER_REGISTRY:-crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com}"
IMAGE_NAME="${DOCKER_REGISTRY}/n_common/vol-monitor"
VERSION="beta"
DOCKERFILE="Dockerfile.cross-compile"

echo "============================================"
echo "  vol-monitor Single-Image Multi-Arch Build"
echo "============================================"
echo "Registry: $DOCKER_REGISTRY"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Dockerfile: $DOCKERFILE"
echo ""
echo "Build strategy:"
echo "  - Cross-compile arm64 from amd64 host"
echo "  - Single image contains both binaries"
echo "  - Runtime selects correct binary via uname -m"
echo ""

# Step 0: Login to ACR
echo "[0/4] Logging in to ACR..."
if ! docker info 2>&1 | grep -q "$DOCKER_REGISTRY"; then
    echo "Logging in to $DOCKER_REGISTRY..."
    docker login "$DOCKER_REGISTRY" -u "308719298@qq.com" -p "zhangdage2011"
fi

# Step 1: Pull base images
echo "[1/4] Pulling base images..."
docker pull docker.1panel.live/library/rust:latest || true
docker pull docker.1panel.live/library/debian:bookworm-slim || true

# Step 2: Build the image (single command, cross-compiles both architectures)
echo "[2/4] Building image with cross-compilation..."
echo "This will compile for both amd64 (native) and arm64 (cross-compile)..."
echo "Expected time: ~8-12 minutes"
docker build -f "$DOCKERFILE" \
    --tag "$IMAGE_NAME:$VERSION" \
    --tag "$IMAGE_NAME:single-image-test" \
    --progress=plain \
    .

# Step 3: Push to registry
echo "[3/4] Pushing image to registry..."
docker push "$IMAGE_NAME:$VERSION"
docker push "$IMAGE_NAME:single-image-test"

echo ""
echo "[4/4] Build complete! Verifying image..."

# Get image size
IMAGE_SIZE=$(docker images "$IMAGE_NAME:$VERSION" --format "{{.Size}}")
echo ""
echo "============================================"
echo "  Build Complete!"
echo "============================================"
echo ""
echo "Image: $IMAGE_NAME:$VERSION"
echo "Size: $IMAGE_SIZE (expected: ~180-200MB with both binaries)"
echo ""
echo "Image tags:"
echo "  - $IMAGE_NAME:$VERSION"
echo "  - $IMAGE_NAME:single-image-test"
echo ""
echo "To test locally:"
echo "  docker run --rm $IMAGE_NAME:$VERSION --help"
echo ""
echo "To verify architecture inside container:"
echo "  docker run --rm $IMAGE_NAME:$VERSION sh -c 'uname -m && cat /proc/cpuinfo | head -1'"
echo ""
echo "To deploy to Kubernetes:"
echo "  kubectl set image deployment/vol-monitor -n deribit vol-monitor=$IMAGE_NAME:$VERSION"
echo ""
echo "To rollback:"
echo "  kubectl rollout undo deployment/vol-monitor -n deribit"
echo ""
