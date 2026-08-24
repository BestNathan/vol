#!/usr/bin/env bash
# Seal a plaintext Kubernetes Secret using the real kubeseal binary.
#
# Guarantees version parity with the cluster's sealed-secrets controller
# by reading the image tag from the kube-system deployment. If kubeseal
# is missing or the version doesn't match, downloads the right binary
# into a local cache (no sudo required).
#
# Usage:
#   ./scripts/seal-secret.sh secret.yaml                 # strict scope (default)
#   ./scripts/seal-secret.sh --namespace-wide secret.yaml
#   ./scripts/seal-secret.sh --cluster-wide secret.yaml
#   cat secret.yaml | ./scripts/seal-secret.sh --output sealed.yaml
#   cat secret.yaml | ./scripts/seal-secret.sh -o sealed.yaml -
#
# Environment:
#   KUBESEAL_MIRROR  Override the download base URL. Defaults to GitHub.
#                    Example: https://ghfast.top/https://github.com
#   KUBESEAL_CACHE   Where to store downloaded binaries.
#                    Default: $HOME/.cache/vol/kubeseal
set -euo pipefail

# ── Args ──────────────────────────────────────────────────────────────
SCOPE_FLAG=""
INPUT=""
OUTPUT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --namespace-wide) SCOPE_FLAG="--scope namespace-wide"; shift ;;
    --cluster-wide)   SCOPE_FLAG="--scope cluster-wide";   shift ;;
    -o|--output)      OUTPUT="$2"; shift 2 ;;
    -)                INPUT="-"; shift ;;
    -*)               echo "unknown flag: $1" >&2; exit 2 ;;
    *)                INPUT="$1"; shift ;;
  esac
done

if [ -z "$INPUT" ]; then
  cat >&2 <<'EOF'
usage: seal-secret.sh [--namespace-wide|--cluster-wide] [-o out.yaml] <secret.yaml | ->

  Reads a plaintext Kubernetes Secret and prints the SealedSecret YAML.
  Downloads the right kubeseal version automatically if missing.

  Examples:
    ./scripts/seal-secret.sh secret.yaml > sealed.yaml
    ./scripts/seal-secret.sh --namespace-wide -o sealed.yaml secret.yaml
    cat secret.yaml | ./scripts/seal-secret.sh -
EOF
  exit 2
fi

# ── Read plaintext Secret ─────────────────────────────────────────────
if [ "$INPUT" = "-" ]; then
  PLAINTEXT=$(cat)
else
  [ -f "$INPUT" ] || { echo "file not found: $INPUT" >&2; exit 1; }
  PLAINTEXT=$(cat "$INPUT")
fi

# ── Detect required kubeseal version from the cluster ─────────────────
detect_version() {
  local image
  image=$(kubectl get deployment sealed-secrets-controller -n kube-system \
    -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null) || {
    echo "error: cannot read sealed-secrets-controller deployment" >&2
    echo "  check: kubectl get deployment -n kube-system sealed-secrets-controller" >&2
    exit 1
  }
  # Extract the tag. Handles:
  #   bitnami/sealed-secrets-controller:0.27.1
  #   registry.example.com/path/sealed-secrets-controller:v0.27.1
  #   image@sha256:deadbeef...  (digest — fall back to error)
  if [[ "$image" == *"@sha256:"* ]]; then
    echo "error: controller image uses a digest pin ($image)" >&2
    echo "  cannot determine kubeseal version; set it via \$KUBESEAL_VERSION" >&2
    exit 1
  fi
  local tag="${image##*:}"
  # Strip leading 'v' if present.
  echo "${tag#v}"
}

VERSION="${KUBESEAL_VERSION:-$(detect_version)}"
echo "sealed-secrets controller version: $VERSION" >&2

# ── Resolve a kubeseal binary of the right version ────────────────────
CACHE_DIR="${KUBESEAL_CACHE:-$HOME/.cache/vol/kubeseal}"
MIRROR="${KUBESEAL_MIRROR:-https://github.com}"

detect_platform() {
  local os arch
  case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=darwin ;;
    *)      echo "error: unsupported OS: $(uname -s)" >&2; exit 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64)   arch=amd64 ;;
    arm64|aarch64)  arch=arm64 ;;
    *)              echo "error: unsupported arch: $(uname -m)" >&2; exit 1 ;;
  esac
  echo "$os-$arch"
}

download_kubeseal() {
  local platform="$1"
  local dest="$2"
  local url="${MIRROR}/bitnami-labs/sealed-secrets/releases/download/v${VERSION}/kubeseal-${VERSION}-${platform}.tar.gz"

  echo "downloading kubeseal v${VERSION} for ${platform} ..." >&2
  echo "  from: $url" >&2

  local tmpdir
  tmpdir=$(mktemp -d)

  if ! curl -fsSL --connect-timeout 15 --retry 3 -o "$tmpdir/kubeseal.tgz" "$url"; then
    echo "" >&2
    echo "error: download failed. Common fixes:" >&2
    echo "  - Network unreachable from China? Set KUBESEAL_MIRROR:" >&2
    echo "      export KUBESEAL_MIRROR=https://ghfast.top/https://github.com" >&2
    echo "      export KUBESEAL_MIRROR=https://gh-proxy.com/https://github.com" >&2
    echo "  - Version v${VERSION} doesn't publish ${platform} binaries." >&2
    echo "    Check: ${MIRROR}/bitnami-labs/sealed-secrets/releases/tag/v${VERSION}" >&2
    rm -rf "$tmpdir"
    exit 1
  fi

  tar xzf "$tmpdir/kubeseal.tgz" -C "$tmpdir" kubeseal
  chmod +x "$tmpdir/kubeseal"
  mv "$tmpdir/kubeseal" "$dest"
  rm -rf "$tmpdir"
}

resolve_kubeseal() {
  # 1. PATH binary — use if version matches.
  if command -v kubeseal >/dev/null 2>&1; then
    local path_ver
    path_ver=$(kubeseal --version 2>/dev/null | grep -oE 'v?[0-9]+\.[0-9]+\.[0-9]+' | head -1 | sed 's/^v//')
    if [ "$path_ver" = "$VERSION" ]; then
      echo "kubeseal v${path_ver} found in PATH ✓" >&2
      echo "kubeseal"
      return
    fi
    echo "PATH kubeseal is v${path_ver:-unknown}, need v${VERSION} — using cache" >&2
  fi

  # 2. Cache.
  local platform
  platform=$(detect_platform)
  local cached="$CACHE_DIR/v${VERSION}/kubeseal"
  if [ -x "$cached" ]; then
    echo "kubeseal v${VERSION} found in cache ✓" >&2
    echo "$cached"
    return
  fi

  # 3. Download.
  mkdir -p "$CACHE_DIR/v${VERSION}"
  download_kubeseal "$platform" "$cached"
  echo "kubeseal v${VERSION} installed to $cached ✓" >&2
  echo "$cached"
}

KUBESEAL=$(resolve_kubeseal)

# ── Seal the Secret ───────────────────────────────────────────────────
# `kubeseal` reads the plaintext Secret from stdin and writes the
# SealedSecret to stdout. --format yaml forces YAML output (default is
# JSON).
RESULT=$(echo "$PLAINTEXT" | "$KUBESEAL" --format yaml $SCOPE_FLAG \
  --cert <(kubectl get secret -n kube-system \
    -l sealedsecrets.bitnami.com/sealed-secrets-key \
    -o jsonpath='{.items[0].data.tls\.crt}' | base64 -d) \
  2>&1) || {
  echo "kubeseal failed:" >&2
  echo "$RESULT" >&2
  exit 1
}

if [ -n "$OUTPUT" ]; then
  echo "$RESULT" > "$OUTPUT"
  echo "sealed → $OUTPUT" >&2
else
  echo "$RESULT"
fi
