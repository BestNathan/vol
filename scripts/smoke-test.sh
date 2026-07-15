#!/bin/bash
# scripts/smoke-test.sh — Post-deployment smoke test for agent-server
# =============================================================================
# Validates that the agent-server deployment is healthy and responds to basic
# JSON-RPC requests over WebSocket.
#
# Prerequisites:
#   - kubectl configured with access to the target cluster
#   - websocat (brew install websocat / cargo install websocat) or python3
#     for WebSocket testing
#   - curl for HTTP health checks
#
# Usage:
#   ./scripts/smoke-test.sh                          # auto-detect via kubectl
#   ./scripts/smoke-test.sh -H localhost:3001         # direct endpoint
#   ./scripts/smoke-test.sh -n vol-agent-system       # specify namespace
#   ./scripts/smoke-test.sh --control-plane           # test control plane only
#   ./scripts/smoke-test.sh --data-plane              # test data plane only
#   ./scripts/smoke-test.sh --all                     # test all components
# =============================================================================

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
NAMESPACE="${NAMESPACE:-vol-agent-system}"
TIMEOUT="${TIMEOUT:-10}"
PASS=0
FAIL=0

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# ── Helpers ───────────────────────────────────────────────────────────────────
pass() { echo -e "  ${GREEN}✓ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}✗ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }
info() { echo -e "  ${YELLOW}→${NC} $1"; }

# ── Port forward helper ───────────────────────────────────────────────────────
port_forward() {
    local svc=$1 port=$2
    info "Port-forwarding $svc:$port ..."
    kubectl -n "$NAMESPACE" port-forward "svc/$svc" "$port:$port" &
    local pf_pid=$!
    sleep 2
    echo "$pf_pid"
}

health_check() {
    local url=$1 label=${2:-health}
    if curl -sf --max-time "$TIMEOUT" "$url" > /dev/null 2>&1; then
        pass "$label: $url"
        return 0
    else
        fail "$label: $url"
        return 1
    fi
}

# ── WebSocket JSON-RPC test (uses python3) ────────────────────────────────────
ws_rpc_call() {
    local endpoint=$1 method=$2 params=${3:-'{}'}
    python3 -c "
import asyncio, json, sys
try:
    import websockets
except ImportError:
    print('SKIP: websockets not installed (pip install websockets)')
    sys.exit(2)

async def call():
    try:
        async with websockets.connect('$endpoint', open_timeout=$TIMEOUT) as ws:
            req = {
                'jsonrpc': '2.0',
                'id': 1,
                'method': '$method',
                'params': $params,
            }
            await ws.send(json.dumps(req))
            resp = await asyncio.wait_for(ws.recv(), timeout=$TIMEOUT)
            result = json.loads(resp)
            if 'error' in result:
                print(f'RPC_ERROR: {result[\"error\"]}')
                sys.exit(1)
            print(f'OK: {json.dumps(result.get(\"result\", {}))[:200]}')
    except Exception as e:
        print(f'CONNECT_ERROR: {e}')
        sys.exit(1)

asyncio.run(call())
" 2>&1
}

# ── CLI args ──────────────────────────────────────────────────────────────────
TARGET=""
MODE="auto"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -H|--host) TARGET="$2"; shift 2 ;;
        -n|--namespace) NAMESPACE="$2"; shift 2 ;;
        --control-plane) MODE="cp" ;;
        --data-plane) MODE="dp" ;;
        --all) MODE="all" ;;
        -h|--help)
            sed -n '2,/^$/p' "$0"
            echo ""
            echo "Options:"
            echo "  -H, --host HOST:PORT   Direct endpoint (skips kubectl)"
            echo "  -n, --namespace NS     Kubernetes namespace (default: vol-agent-system)"
            echo "  --control-plane        Test control plane only"
            echo "  --data-plane           Test data plane only"
            echo "  --all                  Test all components"
            exit 0
            ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
    shift
done

# ── Main ──────────────────────────────────────────────────────────────────────
echo "============================================"
echo "  Smoke Test — vol-agent-server"
echo "  Namespace: $NAMESPACE"
echo "  Time: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "============================================"
echo ""

# Health check (HTTP)
echo "── Health Checks ──"

if [ -n "$TARGET" ]; then
    # Direct endpoint mode
    BASE="http://${TARGET}"
    health_check "$BASE/health" "agent-server"
else
    # kubectl mode
    for svc in agent-server agent-server-dp agent-server-dingtalk; do
        if kubectl -n "$NAMESPACE" get svc "$svc" &>/dev/null; then
            # Try port-forward + health check
            port=$(kubectl -n "$NAMESPACE" get svc "$svc" -o jsonpath='{.spec.ports[0].port}' 2>/dev/null || echo "")
            if [ -n "$port" ]; then
                pf_pid=$(port_forward "$svc" "$((port + 10000))")
                health_check "http://localhost:$((port + 10000))/health" "$svc"
                kill "$pf_pid" 2>/dev/null || true
            fi
        fi
    done

    # Check pod status
    echo ""
    info "Pod status:"
    kubectl -n "$NAMESPACE" get pods -l app.kubernetes.io/part-of=vol-agent -o wide 2>/dev/null || true
fi

# WebSocket JSON-RPC test
echo ""
echo "── JSON-RPC WebSocket Tests ──"

if [ -n "$TARGET" ]; then
    WS_URL="ws://${TARGET}/ws"
    info "Testing $WS_URL ..."
    result=$(ws_rpc_call "$WS_URL" "agent.list" '{}')
    case "$result" in
        OK:*) pass "agent.list: $result" ;;
        SKIP:*) info "agent.list: $result" ;;
        *) fail "agent.list: $result" ;;
    esac
else
    info "Skipping WebSocket tests (use -H to specify endpoint)"
    info "Example: $0 -H localhost:3001"
fi

# ── Dependent services check ──────────────────────────────────────────────────
echo ""
echo "── Dependent Services ──"

# Check MCP services if accessible
for mcp_svc in docs-rs-mcp cli-tools-mcp; do
    if kubectl -n "$NAMESPACE" get svc "$mcp_svc" &>/dev/null 2>&1; then
        info "$mcp_svc service exists"
    fi
done 2>/dev/null || true

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
TOTAL=$((PASS + FAIL))
echo "  Results: $PASS passed, $FAIL failed, $TOTAL total"
if [ "$FAIL" -eq 0 ]; then
    echo -e "  ${GREEN}All checks passed${NC}"
else
    echo -e "  ${RED}$FAIL check(s) failed${NC}"
fi
echo "============================================"

exit "$FAIL"
