#!/bin/bash
# scripts/test-agent.sh - Agent Advice 本地测试脚本
#
# 用法:
#   ./scripts/test-agent.sh          # 使用默认测试配置
#   ./scripts/test-agent.sh --help   # 显示帮助
#
# 测试流程:
# 1. 加载 .env 环境变量
# 2. 使用 config.agent-test.toml 配置
# 3. 运行 vol-monitor 并观察日志
# 4. 检查 Agent Advice 是否正常工作

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_step() {
    echo -e "${GREEN}==>${NC} $1"
}

echo_warn() {
    echo -e "${YELLOW}WARN:${NC} $1"
}

echo_error() {
    echo -e "${RED}ERROR:${NC} $1"
}

# 帮助信息
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    cat << EOF
Agent Advice 本地测试脚本

用法: $0 [OPTIONS]

选项:
  --help, -h     显示帮助信息
  --config FILE  使用指定的配置文件 (默认：config.agent-test.toml)
  --dry-run      只验证配置，不实际运行
  --verbose      显示详细的调试日志

环境要求:
  - .env 文件包含必要的环境变量
  - config.agent-test.toml 或其他配置文件

测试内容:
  1. TDengine 客户端初始化
  2. ToolRegistry 初始化
  3. FeishuNotification 初始化
  4. AgentAdviceService 注册
  5. ReAct Agent 工具调用

示例:
  $0                          # 使用默认配置运行
  $0 --config config.prod.toml  # 使用生产配置
  $0 --dry-run                # 只验证配置
EOF
    exit 0
fi

# 解析参数
CONFIG_FILE="config.agent-test.toml"
DRY_RUN=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        *)
            echo_error "未知选项：$1"
            echo "使用 --help 查看用法"
            exit 1
            ;;
    esac
done

# 检查 .env 文件
echo_step "检查环境变量配置"
if [[ ! -f .env ]]; then
    echo_error ".env 文件不存在"
    echo_warn "请创建 .env 文件，参考 .env.example"
    exit 1
fi

# 检查配置文件
if [[ ! -f "$CONFIG_FILE" ]]; then
    echo_error "配置文件不存在：$CONFIG_FILE"
    exit 1
fi

# 加载环境变量
echo_step "加载环境变量"
set -a
source .env
set +a

# 验证必要的环境变量
REQUIRED_VARS=("DERIBIT_CLIENT_ID" "DERIBIT_CLIENT_SECRET")
MISSING_VARS=()

for var in "${REQUIRED_VARS[@]}"; do
    if [[ -z "${!var}" ]]; then
        MISSING_VARS+=("$var")
    fi
done

if [[ ${#MISSING_VARS[@]} -gt 0 ]]; then
    echo_error "缺少必要的环境变量："
    for var in "${MISSING_VARS[@]}"; do
        echo "  - $var"
    done
    echo_warn "请在 .env 文件中配置这些变量"
    exit 1
fi

# 检查 LLM API Key
if [[ -z "$ANTHROPIC_AUTH_TOKEN" ]]; then
    echo_warn "ANTHROPIC_AUTH_TOKEN 未设置，Agent Advice 将无法工作"
    echo_warn "如仅需测试基础功能，可设置 agent_advice.enabled = false"
fi

# 显示配置
echo ""
echo "=== 测试配置 ==="
echo "配置文件：$CONFIG_FILE"
echo "日志级别：${RUST_LOG:-info}"
echo "代理：${HTTPS_PROXY:-未设置}"
echo ""

# Dry run 模式
if [[ "$DRY_RUN" == "true" ]]; then
    echo_step "Dry run 模式 - 仅验证配置"

    echo_step "检查二进制文件"
    if [[ ! -f target/release/vol-monitor ]]; then
        echo_warn "release 版本不存在，正在构建..."
        cargo build --release -p vol-monitor
    fi

    echo_step "验证配置文件格式"
    # 这里可以添加 TOML 验证逻辑

    echo_step "检查 LLM Provider 配置"
    grep -A5 "\[agent_advice\]" "$CONFIG_FILE" || echo_warn "未找到 agent_advice 配置"

    echo ""
    echo -e "${GREEN}验证完成${NC}"
    exit 0
fi

# 运行测试
echo_step "启动 vol-monitor"
echo_warn "按 Ctrl+C 停止"
echo ""

# 设置日志级别
export RUST_LOG="${RUST_LOG:-info,vol_llm_bridge=debug,vol_llm_agent=debug}"

# 运行程序
./target/release/vol-monitor --config "$CONFIG_FILE"

# 退出码检查
if [[ $? -eq 0 ]]; then
    echo ""
    echo -e "${GREEN}测试完成${NC}"
else
    echo ""
    echo -e "${RED}程序异常退出${NC}"
fi
