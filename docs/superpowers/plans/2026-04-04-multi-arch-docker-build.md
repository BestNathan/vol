# Multi-Architecture Docker Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable building and deploying multi-architecture Docker images (linux/amd64 and linux/arm64) for vol-monitor using Docker Buildx.

**Architecture:** Use Docker Buildx with docker-container driver to build multi-arch images. The deploy.sh script will use `docker buildx build --push` instead of separate `docker build` + `docker push` commands. The deployment.yaml will remove the amd64-only nodeSelector to allow scheduling on any architecture node.

**Tech Stack:** Docker Buildx, QEMU emulation for cross-architecture builds, Aliyun Container Registry (ACR) for storing manifest lists.

---

### Task 1: Verify Buildx Builder Setup

**Files:**
- No file changes - verification task

- [ ] **Step 1: Check if buildx is available**

Run:
```bash
docker buildx version
```
Expected: Buildx version output (e.g., v0.12.0)

- [ ] **Step 2: List existing builders**

Run:
```bash
docker buildx ls
```
Expected: Shows available builders including default docker driver

- [ ] **Step 3: Check if multiarch builder exists**

Run:
```bash
docker buildx inspect multiarch 2>/dev/null || echo "Builder does not exist"
```
Expected: Either builder details or "Builder does not exist"

- [ ] **Step 4: Create multiarch builder if needed**

If builder doesn't exist, run:
```bash
docker buildx create --use --name multiarch --driver docker-container
docker buildx inspect multiarch --bootstrap
```
Expected: New builder created and bootstrapped with QEMU support

- [ ] **Step 5: Verify QEMU support**

Run:
```bash
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
```
Expected: QEMU binfmt_misc registered for arm64

### Task 2: Modify deploy.sh for Buildx

**Files:**
- Modify: `k8s/deploy.sh:32-66`

**Current code (lines 32-66):**
```bash
# Step 2: Enable QEMU for arm64 build
echo "[2/7] Setting up QEMU for arm64 build..."
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes >/dev/null 2>&1 || true

# Step 3: Build and push amd64 image
echo "[3/7] Building amd64 image..."
docker build --platform linux/amd64 -t "$IMAGE_NAME:amd64" -f Dockerfile .
echo "Pushing amd64 image..."
docker push "$IMAGE_NAME:amd64"

# Step 4: Build and push arm64 image (using QEMU)
echo "[4/7] Building arm64 image (this may take 10-15 minutes)..."
docker build --platform linux/arm64 -t "$IMAGE_NAME:arm64" -f Dockerfile .
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
```

**Replace with:**
```bash
# Step 2: Initialize buildx builder if needed
echo "[2/7] Initializing Docker Buildx..."
if ! docker buildx inspect multiarch >/dev/null 2>&1; then
    echo "Creating multiarch builder..."
    docker buildx create --use --name multiarch --driver docker-container
    docker buildx inspect multiarch --bootstrap
fi
docker buildx use multiarch

# Step 3-5: Build and push multi-architecture image
echo "[3/7] Building multi-architecture image (this may take 10-15 minutes)..."
docker buildx build --platform linux/amd64,linux/arm64 \
    --push \
    -t "$IMAGE_NAME:$VERSION" \
    -f Dockerfile .

# Tag as latest if not already
if [ "$VERSION" != "latest" ]; then
    docker buildx build --platform linux/amd64,linux/arm64 \
        --push \
        -t "$IMAGE_NAME:latest" \
        -f Dockerfile .
fi
```

- [ ] **Step 1: Read current deploy.sh**

Run:
```bash
cat k8s/deploy.sh
```

- [ ] **Step 2: Replace steps 2-5 with buildx command**

Use sed or manual edit to replace lines 32-66 with the new code above.

- [ ] **Step 3: Verify the modified script syntax**

Run:
```bash
bash -n k8s/deploy.sh
```
Expected: No output (syntax OK)

- [ ] **Step 4: Commit changes**

Run:
```bash
git add k8s/deploy.sh
git commit -m "feat: use buildx for multi-arch image builds"
```

### Task 3: Remove nodeSelector from deployment.yaml

**Files:**
- Modify: `k8s/deployment.yaml:24-25`

**Current code (lines 23-26):**
```yaml
      restartPolicy: Always
      nodeSelector:
        kubernetes.io/arch: amd64
      containers:
```

**Replace with:**
```yaml
      restartPolicy: Always
      containers:
```

- [ ] **Step 1: Read current deployment.yaml**

Run:
```bash
cat k8s/deployment.yaml
```

- [ ] **Step 2: Remove nodeSelector lines**

Edit the file to remove lines 24-25 (the nodeSelector block).

- [ ] **Step 3: Validate YAML syntax**

Run:
```bash
kubectl apply --dry-run=client -f k8s/deployment.yaml 2>&1 | head -5
```
Expected: "deployment.apps/vol-monitor configured" or similar (validation pass)

- [ ] **Step 4: Commit changes**

Run:
```bash
git add k8s/deployment.yaml
git commit -m "feat: remove amd64-only nodeSelector for multi-arch support"
```

### Task 4: Test Local Multi-Arch Build

**Files:**
- No file changes - testing task

- [ ] **Step 1: Ensure logged in to ACR**

Run:
```bash
docker login crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com -u "308719298@qq.com" -p "zhangdage2011"
```
Expected: "Login Succeeded"

- [ ] **Step 2: Run modified deploy.sh with test tag**

Run:
```bash
cd /root/nq-deribit
./k8s/deploy.sh test-multiarch
```
Expected: Build completes in 5-15 minutes, manifest pushed

- [ ] **Step 3: Verify multi-arch manifest**

Run:
```bash
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:test-multiarch
```
Expected output shows both architectures:
```
MediaType: application/vnd.docker.distribution.manifest.list.v2+json
Manifests: 
  Name:      ...linux/amd64
  Name:      ...linux/arm64
```

- [ ] **Step 4: Document result**

Note the build time and any issues encountered.

### Task 5: Update CLAUDE.md Documentation

**Files:**
- Modify: `CLAUDE.md:105-140` (Multi-Architecture Builds section)

**Current section exists - verify it matches actual usage:**

- [ ] **Step 1: Read current Multi-Architecture Builds section**

Check lines 105-140 in CLAUDE.md.

- [ ] **Step 2: Update if needed**

The section should reference `./k8s/deploy.sh` using buildx automatically. Add verification command:

```markdown
**Verify deployed image:**
```bash
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
```

- [ ] **Step 3: Commit documentation update**

Run:
```bash
git add CLAUDE.md
git commit -m "docs: update multi-arch build documentation"
```

### Task 6: Deploy and Verify in Kubernetes

**Files:**
- No file changes - deployment task

- [ ] **Step 1: Apply updated deployment**

Run:
```bash
kubectl apply -f k8s/deployment.yaml
```

- [ ] **Step 2: Watch pod scheduling**

Run:
```bash
kubectl -n deribit get pods -w
```
Expected: Pod schedules on available node (amd64 or arm64)

- [ ] **Step 3: Verify pod is running**

Run:
```bash
kubectl -n deribit get pods -l app=vol-monitor
```
Expected: STATUS=Running

- [ ] **Step 4: Check logs**

Run:
```bash
kubectl -n deribit logs deployment/vol-monitor --tail=20
```
Expected: Normal vol-monitor startup logs

- [ ] **Step 5: Verify node architecture**

Run:
```bash
kubectl -n deribit get pods -l app=vol-monitor -o jsonpath='{.items[0].spec.nodeName}'
kubectl get node <NODE_NAME> -o jsonpath='{.status.nodeInfo.architecture}'
```
Expected: Shows which architecture the pod is running on

---

## Testing Summary

After all tasks complete:

```bash
# Verify multi-arch image in registry
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest

# Verify K8s deployment
kubectl -n deribit get pods -l app=vol-monitor -o wide

# Check which node (architecture) is being used
kubectl -n deribit get pods -l app=vol-monitor -o jsonpath='{.items[0].spec.nodeName}'
```

## Rollback

If issues occur:
```bash
# Revert to amd64-only
git revert HEAD~3..HEAD
./k8s/deploy.sh latest
```
