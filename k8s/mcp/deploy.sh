#!/bin/bash
# k8s/deploy-mcp.sh - Build, push and deploy an MCP server to Kubernetes
#
# Usage: ./k8s/deploy-mcp.sh <binary-name> [version]

set -e

IMAGE_NAME="crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor"
K8S_DIR="$(dirname "$0")"
BINARY_NAME="${1:?Usage: $0 <binary-name>}"
VERSION="${2:-latest}"

echo "Deploying $BINARY_NAME ($VERSION)..."

# Step 1: Build and push Docker image
echo "[1/3] Building Docker image..."
docker build --build-arg BIN_NAME="$BINARY_NAME" \
    -t "$IMAGE_NAME:$BINARY_NAME-$VERSION" \
    -f crates/vol-mcp-servers/Dockerfile .
docker push "$IMAGE_NAME:$BINARY_NAME-$VERSION"

# Step 2: Render template
echo "[2/3] Rendering deployment manifest..."
sed "s/\${MCP_NAME}/$BINARY_NAME/g" "$K8S_DIR/deployment-template.yaml" | \
    kubectl apply -f -

# Step 3: Wait for rollout
echo "[3/3] Waiting for rollout..."
kubectl -n mcp rollout status deployment/$BINARY_NAME --timeout=120s

echo "Done. Pod status:"
kubectl -n mcp get pods -l app="$BINARY_NAME"
echo "Logs: kubectl -n mcp logs -f deployment/$BINARY_NAME"
echo "Service: kubectl -n mcp get svc $BINARY_NAME"
