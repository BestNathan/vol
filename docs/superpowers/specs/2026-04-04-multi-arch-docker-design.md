# vol-monitor 多架构 Docker 镜像设计

**日期:** 2026-04-04
**作者:** Claude Code
**状态:** 已实施

## 概述

支持构建和推送多架构 Docker 镜像，使 vol-monitor 可部署到不同 CPU 架构的 Kubernetes 节点。

## 目标架构

- `linux/amd64` - x86_64 服务器（k8s-master, k8s-worker1）
- `linux/arm64` - ARM 服务器（rock-5b-plus）

## 技术方案

### Docker Buildx

使用 Docker Buildx 的 docker-container driver：

```bash
docker buildx create --use --name multiarch --driver docker-container
docker buildx build --platform linux/amd64,linux/arm64 --push -t repo/vol-monitor:latest .
```

### 工作原理

1. Buildx 创建独立的 builder container
2. 使用 QEMU 模拟非原生架构编译
3. 分别构建各架构镜像层
4. 创建并推送 manifest list 到仓库

### 优势

- 单一命令构建多架构
- 与现有 deploy.sh 集成简单
- Docker 原生支持，无需外部 CI

### 限制

- QEMU 模拟编译较慢（5-10 分钟）
- 需要足够的 Docker 内存（建议 ≥4GB）

## 修改内容

1. **k8s/deploy.sh** - 替换 `docker build` + `docker push` 为 `docker buildx build --push`
2. **k8s/deployment.yaml** - 移除 `nodeSelector: kubernetes.io/arch: amd64`
3. **CLAUDE.md** - 添加多架构构建说明
4. **Builder 初始化** - 首次运行前创建 builder 实例

## 部署流程

```bash
# 1. 初始化（一次性）
docker buildx create --use --name multiarch --driver docker-container
docker buildx inspect multiarch --bootstrap

# 2. 构建并推送
./k8s/deploy.sh latest

# 3. 验证
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
# 输出应显示：
#   Manifests:
#     ...  # linux/amd64
#     ...  # linux/arm64

# 4. 部署到 k8s（自动选择正确架构）
kubectl -n deribit get pods -l app=vol-monitor
```

## 修改的文件

| 文件 | 修改内容 |
|------|----------|
| `k8s/deploy.sh` | 步骤 1-2 改为使用 buildx，添加 builder 初始化逻辑 |
| `k8s/deployment.yaml` | 移除 nodeSelector，允许调度到任意架构节点 |
| `CLAUDE.md` | 添加"Multi-Architecture Builds"章节 |

## 回滚方案

如多架构构建失败，可临时改回单架构：

```bash
# 修改 deploy.sh 使用标准 docker build
docker build -t repo/vol-monitor:latest .
docker push repo/vol-monitor:latest
```

## 测试验证

```bash
# 本地验证 amd64 构建
docker buildx build --platform linux/amd64 --load -t vol-monitor:test .
docker run --rm vol-monitor:test --version

# 验证多架构镜像
docker buildx imagetools inspect <image>

# 验证 K8s 部署
kubectl -n deribit get pods -l app=vol-monitor
kubectl -n deribit logs deployment/vol-monitor
```

## 参考

- Docker Buildx 文档：https://docs.docker.com/buildx/
- 多架构镜像：https://www.docker.com/blog/multi-arch-images/
- BuildKit 文档：https://github.com/moby/buildkit
