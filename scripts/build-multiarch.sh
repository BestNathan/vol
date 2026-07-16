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

# Configuration - GitHub Container Registry (GHCR)
DOCKER_REGISTRY="${DOCKER_REGISTRY:-ghcr.io}"
IMAGE_NAME="${DOCKER_REGISTRY}/bestnathan/vol-monitor"
VERSION="beta"
DOCKERFILE="dockers/vol-monitor.cross.Dockerfile"

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

# Step 0: Login to GHCR
echo "[0/4] Logging in to GHCR..."
# Credentials must be provided via environment variables:
#   GITHUB_USER — GitHub username (default: BestNathan)
#   GITHUB_TOKEN — GitHub PAT with write:packages scope
if [ -z "${GITHUB_TOKEN:-}" ] && [ -z "${DOCKER_PASSWORD:-}" ]; then
    echo "ERROR: Set GITHUB_TOKEN (or DOCKER_PASSWORD) env var with a GitHub PAT (write:packages scope)"
    echo "  export GITHUB_TOKEN=<your-github-pat>"
    exit 1
fi
echo "${GITHUB_TOKEN:-$DOCKER_PASSWORD}" | docker login ghcr.io -u "${GITHUB_USER:-BestNathan}" --password-stdin

# Step 1: Pull base images (from Docker Hub)
echo "[1/4] Pulling base images..."
docker pull rust:latest || true
docker pull debian:bookworm-slim || true

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
echo "  (ensuring ghcr-registry-secret exists in the deribit namespace)"
echo ""
echo "To rollback:"
echo "  kubectl rollout undo deployment/vol-monitor -n deribit"
echo ""
