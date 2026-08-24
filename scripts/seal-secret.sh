#!/usr/bin/env bash
# Seal a plaintext Kubernetes Secret using the cluster's sealed-secrets
# controller public key. Prints the SealedSecret YAML to stdout.
#
# Usage:
#   ./scripts/seal-secret.sh secret.yaml              # strict scope (default)
#   ./scripts/seal-secret.sh --namespace-wide secret.yaml
#   ./scripts/seal-secret.sh --cluster-wide secret.yaml
#   echo "..." | ./scripts/seal-secret.sh -           # read from stdin
#
# Requirements:
#   - kubectl access to the cluster running sealed-secrets-controller
#   - python3 with `cryptography` and `pycryptodome` packages
#
# The plaintext file is NEVER written to disk by this script —
# pipe sensitive input via stdin (`-`) when possible.
set -euo pipefail

SCOPE="strict"
INPUT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --namespace-wide) SCOPE="namespace"; shift ;;
    --cluster-wide)   SCOPE="cluster";   shift ;;
    -)                INPUT="-";         shift ;;
    -*)               echo "unknown flag: $1" >&2; exit 2 ;;
    *)                INPUT="$1";        shift ;;
  esac
done

if [ -z "$INPUT" ]; then
  echo "usage: $0 [--namespace-wide|--cluster-wide] <secret.yaml | ->" >&2
  exit 2
fi

# Read plaintext YAML
if [ "$INPUT" = "-" ]; then
  PLAINTEXT=$(cat)
else
  [ -f "$INPUT" ] || { echo "file not found: $INPUT" >&2; exit 1; }
  PLAINTEXT=$(cat "$INPUT")
fi

# Fetch the sealed-secrets controller's public cert from the cluster.
CERT_PEM=$(kubectl get secret -n kube-system \
  -l sealedsecrets.bitnami.com/sealed-secrets-key \
  -o jsonpath='{.items[0].data.tls\.crt}' | base64 -d)

if [ -z "$CERT_PEM" ]; then
  echo "error: could not fetch sealed-secrets cert from cluster" >&2
  echo "  check: kubectl get secret -n kube-system -l sealedsecrets.bitnami.com/sealed-secrets-key" >&2
  exit 1
fi

# Parse metadata from the plaintext Secret.
NAME=$(echo "$PLAINTEXT" | python3 -c "import sys,yaml; print(yaml.safe_load(sys.stdin)['metadata']['name'])")
NAMESPACE=$(echo "$PLAINTEXT" | python3 -c "import sys,yaml; d=yaml.safe_load(sys.stdin)['metadata']; print(d.get('namespace','default'))")

# Delegate the actual encryption to Python.
echo "$PLAINTEXT" | python3 - "$CERT_PEM" "$NAME" "$NAMESPACE" "$SCOPE" << 'PYEOF'
import sys, base64, struct, os, yaml
from cryptography import x509
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

cert_pem, name, namespace, scope = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
secret = yaml.safe_load(sys.stdin)

# Compute the encryption label based on scope.
if scope == "cluster":
    label = b""
elif scope == "namespace":
    label = namespace.encode()
else:  # strict (default)
    label = f"{namespace}/{name}".encode()

pub_key = x509.load_pem_x509_certificate(cert_pem.encode()).public_key()

# Encrypt each data / stringData entry.
plain_data = dict(secret.get("data") or {})
# stringData is plaintext; data is base64-encoded — decode both to raw bytes.
for k, v in (secret.get("stringData") or {}).items():
    plain_data[k] = base64.b64encode(v.encode()).decode()

encrypted_data = {}
for key, b64val in plain_data.items():
    plaintext = base64.b64decode(b64val)
    session_key = os.urandom(32)

    # RSA-OAEP with SHA-256 + scope label (matches sealed-secrets v0.27+).
    encrypted_key = pub_key.encrypt(
        session_key,
        padding.OAEP(
            mgf=padding.MGF1(algorithm=hashes.SHA256()),
            algorithm=hashes.SHA256(),
            label=label,
        ),
    )

    # AES-GCM with zero nonce (session key used only once per entry).
    aesgcm = AESGCM(session_key)
    ct_tag = aesgcm.encrypt(b"\x00" * 12, plaintext, None)

    # Binary blob: RSA_len(2) || RSA_OAEP || AES_GCM(ct+tag)
    blob = struct.pack(">H", len(encrypted_key)) + encrypted_key + ct_tag
    encrypted_data[key] = base64.b64encode(blob).decode()

# Build annotations for scope.
annotations = dict(secret.get("metadata", {}).get("annotations") or {})
if scope == "namespace":
    annotations["sealedsecrets.bitnami.com/namespace-wide"] = "true"
elif scope == "cluster":
    annotations["sealedsecrets.bitnami.com/cluster-wide"] = "true"

sealed = {
    "apiVersion": "bitnami.com/v1alpha1",
    "kind": "SealedSecret",
    "metadata": {
        "name": name,
        "namespace": namespace,
        "labels": secret.get("metadata", {}).get("labels"),
        "annotations": annotations or None,
    },
    "spec": {
        "encryptedData": encrypted_data,
        "template": {
            "metadata": {
                "name": name,
                "namespace": namespace,
                "labels": secret.get("metadata", {}).get("labels"),
            },
            "type": secret.get("type", "Opaque"),
        },
    },
}
# Strip None values for clean output.
sealed["metadata"] = {k: v for k, v in sealed["metadata"].items() if v}

print(yaml.dump(sealed, default_flow_style=False, sort_keys=False, allow_unicode=True), end="")
PYEOF
