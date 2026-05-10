#!/bin/bash
# k8s/deploy-mcp.sh - Deploy MCP server(s) to Kubernetes

set -e

IMAGE_NAME="crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor"
K8S_DIR="$(dirname "$0")"
BINARY_NAME="${1:?Usage: $0 <binary-name>}"
VERSION="${2:-latest}"

echo "Deploying $BINARY_NAME ($VERSION)..."

# Build and push Docker image
echo "[1/3] Building Docker image..."
docker build --build-arg BIN_NAME="$BINARY_NAME" \
    -t "$IMAGE_NAME:$BINARY_NAME-$VERSION" \
    -f crates/vol-mcp-servers/Dockerfile .
docker push "$IMAGE_NAME:$BINARY_NAME-$VERSION"

# Update deployment image tag
echo "[2/3] Updating deployment manifest..."
sed -i "s|image: $IMAGE_NAME:$BINARY_NAME:.*|image: $IMAGE_NAME:$BINARY_NAME-$VERSION|" \
    "$K8S_DIR/deployment-$BINARY_NAME.yaml"

# Apply to cluster
echo "[3/3] Applying Kubernetes manifests..."
kubectl apply -f "$K8S_DIR/deployment-$BINARY_NAME.yaml"

echo "Waiting for rollout..."
kubectl -n deribit rollout status deployment/$BINARY_NAME --timeout=120s

echo "Done. Pod status:"
kubectl -n deribit get pods -l app="$BINARY_NAME"
echo "Logs: kubectl -n deribit logs -f deployment/$BINARY_NAME"
