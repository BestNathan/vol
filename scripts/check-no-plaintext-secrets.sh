#!/usr/bin/env bash
# Reject plaintext Kubernetes Secrets in staged files.
#
# Secrets in this repo MUST be sealed with Bitnami sealed-secrets
# (kind: SealedSecret) before being committed. Plain `kind: Secret`
# files leak credentials into git history even if later deleted.
#
# Exemptions:
#   - `kind: SealedSecret`                    (the correct form)
#   - Files matching *.example.yaml / *.template.yaml  (documentation only)
#   - Files outside deploy/ and k8s/ (tests, docs, etc.)
#
# Usage:
#   ./scripts/check-no-plaintext-secrets.sh              # check staged files
#   ./scripts/check-no-plaintext-secrets.sh --all        # check whole repo
set -euo pipefail

# Collect files to inspect: staged by default, whole repo with --all.
if [ "${1:-}" = "--all" ]; then
  FILES=$(find deploy k8s -type f \( -name '*.yaml' -o -name '*.yml' \) 2>/dev/null || true)
else
  FILES=$(git diff --cached --name-only --diff-filter=ACM 2>/dev/null | \
          grep -E '\.(ya?ml)$' | \
          grep -E '^(deploy|k8s)/' || true)
fi

if [ -z "$FILES" ]; then
  exit 0
fi

VIOLATIONS=()
EXAMPLE_PATTERN='\.example\.ya?ml$|\.template\.ya?ml$'

while IFS= read -r file; do
  [ -f "$file" ] || continue

  # Skip example / template files (documentation, placeholders only).
  if echo "$file" | grep -qE "$EXAMPLE_PATTERN"; then
    continue
  fi

  # Detect `kind: Secret` — but NOT `kind: SealedSecret`.
  # `grep -P '^\s*kind:\s*Secret\s*$'` matches the exact word "Secret"
  # without catching SealedSecret (the `P` regex has no word-boundary
  # issue because the `$` anchor forces the line to end after "Secret").
  if grep -P '^\s*kind:\s*Secret\s*$' "$file" >/dev/null 2>&1; then
    VIOLATIONS+=("$file")
  fi
done <<< "$FILES"

if [ ${#VIOLATIONS[@]} -gt 0 ]; then
  echo ""
  echo -e "\033[0;31m✗ Plaintext Kubernetes Secret(s) detected:\033[0m"
  echo ""
  for f in "${VIOLATIONS[@]}"; do
    echo "  • $f"
  done
  echo ""
  echo "Plain 'kind: Secret' files are not allowed in git — they leak"
  echo "credentials into history even if later deleted."
  echo ""
  echo "Fix: encrypt with sealed-secrets before committing."
  echo "  1. Write the plaintext Secret to a temp file"
  echo "  2. kubeseal --format yaml < secret.yaml > sealed.yaml"
  echo "     (or use scripts/seal-secret.sh if available)"
  echo "  3. Commit the SealedSecret, not the plaintext"
  echo ""
  echo "Documentation placeholders are OK if named *.example.yaml"
  echo "or *.template.yaml — those are exempt from this check."
  exit 1
fi

echo "no plaintext secrets detected ✓"
