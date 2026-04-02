# vol-monitor Kubernetes 部署方案设计

## 提交信息

- **Commit**: TBD (实施时填写)
- **日期**: 2026-04-02
- **作者**: 实施者

## 改动概述

为 vol-monitor 服务创建 Kubernetes 部署配置，实现容器化部署到现有 k8s 集群。

## 设计目标

| 目标 | 说明 |
|------|------|
| **实用为主** | 个人项目，不需要过度工程化的高可用设计 |
| **简单易维护** | 配置简洁，更新流程清晰 |
| **可扩展** | 后续可根据需要添加监控、限流等增强功能 |

## 部署架构

```
┌─────────────────────────────────────────────────────────┐
│                    Higress Gateway                       │
│                     (NodePort 80/443)                    │
└─────────────────────┬───────────────────────────────────┘
                      │
                      │ Ingress (暂不配置域名，直接 IP 访问)
                      │
┌─────────────────────▼───────────────────────────────────┐
│              vol-monitor Service (ClusterIP)             │
│                     Port: 8080                           │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│          vol-monitor Deployment (replicas: 1)            │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Container: vol-monitor:latest                     │ │
│  │  - Image: Docker Hub (版本标签策略)                 │ │
│  │  - ConfigMap: config.toml 完整配置                  │ │
│  │  - Resources: 暂不限制                              │ │
│  │  - Probes: 暂不需要                                 │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## 变更文件统计

### 新增文件

| 文件 | 说明 |
|------|------|
| `Dockerfile` | 多阶段构建，Rust 编译 + 运行时镜像 |
| `k8s/namespace.yaml` | deribit 命名空间定义 |
| `k8s/configmap.yaml` | config.toml 配置 |
| `k8s/deployment.yaml` | Deployment 配置 |
| `k8s/deploy.sh` | 一键部署脚本 |

### 修改文件

无

**注意**: Service 和 Ingress 配置在设计文档中作为未来扩展预留，初始部署不需要。

## 技术实现

### 1. Dockerfile（多阶段构建）

```dockerfile
# 阶段 1: 构建
FROM rust:1.75-slim as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo build --release

# 阶段 2: 运行时
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/vol-monitor /usr/local/bin/vol-monitor
# config.toml 通过 ConfigMap 挂载，不需要打包进镜像
ENTRYPOINT ["/usr/local/bin/vol-monitor"]
```

### 2. Namespace

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: deribit
```

### 3. ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vol-monitor-config
  namespace: deribit
data:
  config.toml: |
    # 完整 config.toml 内容
    [data_sources.deribit]
    ...
```

### 4. Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vol-monitor
  namespace: deribit
  labels:
    app: vol-monitor
spec:
  replicas: 1
  selector:
    matchLabels:
      app: vol-monitor
  template:
    metadata:
      labels:
        app: vol-monitor
    spec:
      containers:
      - name: vol-monitor
        image: <your-dockerhub-username>/vol-monitor:<version-tag>
        imagePullPolicy: Always
        ports:
        - containerPort: 8080
        volumeMounts:
        - name: config
          mountPath: /etc/vol-monitor
      volumes:
      - name: config
        configMap:
          name: vol-monitor-config
```

### 5. Service

```yaml
# vol-monitor 是纯后台服务，不需要 Service 暴露
# 如需 future HTTP 健康检查端点，可添加：
apiVersion: v1
kind: Service
metadata:
  name: vol-monitor
  namespace: deribit
spec:
  type: ClusterIP
  ports:
  - port: 8080
    targetPort: 8080
    protocol: TCP
  selector:
    app: vol-monitor
```

**注意**: vol-monitor 当前是纯后台服务（WebSocket + 通知），没有 HTTP 监听。Service 预留用于未来健康检查端点。

### 6. Ingress (Higress)

**注意**: vol-monitor 当前没有 HTTP 服务，Ingress 配置仅作为未来扩展预留。

```yaml
# 未来添加 HTTP API 时启用
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: vol-monitor
  namespace: deribit
  annotations:
    kubernetes.io/ingress.class: higress
spec:
  rules:
  - http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: vol-monitor
            port:
              number: 8080
```

### 7. 部署脚本

```bash
#!/bin/bash
# k8s/deploy.sh - 一键部署脚本

set -e

IMAGE_NAME="your-dockerhub-username/vol-monitor"
VERSION="${1:-latest}"

echo "=== vol-monitor Kubernetes Deploy ==="
echo "Image: $IMAGE_NAME:$VERSION"

# 1. 构建 Docker 镜像
echo "[1/5] Building Docker image..."
docker build -t $IMAGE_NAME:$VERSION .

# 2. 推送到 Docker Hub
echo "[2/5] Pushing to Docker Hub..."
docker push $IMAGE_NAME:$VERSION

# 3. 更新 Deployment 镜像标签
echo "[3/5] Updating Deployment..."
sed -i "s|image: $IMAGE_NAME:.*|image: $IMAGE_NAME:$VERSION|" k8s/deployment.yaml

# 4. 应用 K8s 配置
echo "[4/5] Applying Kubernetes manifests..."
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/configmap.yaml
kubectl apply -f k8s/deployment.yaml

# 5. 等待部署完成
echo "[5/5] Waiting for deployment..."
kubectl -n deribit rollout status deployment/vol-monitor

echo ""
echo "✅ Deploy complete!"
echo "Service: vol-monitor.deribit.svc.cluster.local:8080"
```

## 配置说明

### ConfigMap 管理

- **方式**: 完整 config.toml 放入 ConfigMap
- **挂载路径**: `/etc/vol-monitor/config.toml`
- **更新方式**: 修改 ConfigMap 后重启 Pod

### 镜像标签策略

| 场景 | 标签 | 说明 |
|------|------|------|
| 开发测试 | `dev-YYYYMMDD` | 每日构建 |
| 版本发布 | `v1.0.0` | Git tag 版本 |
| 最新稳定 | `latest` | 手动更新 |

### 资源限制（可选）

后续可根据实际运行情况添加：

```yaml
resources:
  requests:
    memory: "256Mi"
    cpu: "100m"
  limits:
    memory: "512Mi"
    cpu: "500m"
```

## 部署流程

```bash
# 1. 首次部署
./k8s/deploy.sh v0.1.0

# 2. 查看状态
kubectl -n deribit get pods
kubectl -n deribit logs -f deployment/vol-monitor

# 3. 更新版本
./k8s/deploy.sh v0.1.1

# 4. 回滚（如需）
kubectl -n deribit rollout undo deployment/vol-monitor
```

## 后续计划

- [ ] 添加健康检查端点和 liveness/readiness 探针
- [ ] 根据实际运行情况配置资源限制
- [ ] 配置 Prometheus metrics 暴露
- [ ] 配置日志收集（可选）

**注意**: 初始部署不需要 Service 和 Ingress，因为 vol-monitor 是纯后台服务。未来添加 HTTP API 时再添加。

## 依赖变更

无

## 构建验证

```bash
# Docker 构建
docker build -t vol-monitor:test .
# ✓ 编译成功

# K8s 配置验证
kubectl apply --dry-run=client -f k8s/
# ✓ 配置有效
```

## 方案优势

| 优势 | 说明 |
|------|------|
| **简单** | 3 个 YAML 文件（namespace + configmap + deployment） |
| **实用** | 满足个人使用需求，无过度设计 |
| **可扩展** | 后续可逐步增强监控、限流等功能 |
| **成本低** | Docker Hub 免费，单副本资源占用低 |

---

*此文档由 brainstorming skill 生成*
