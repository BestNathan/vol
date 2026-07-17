#!/usr/bin/env bash
# Check that no crate has active doc tests (they should be proper tests).
# Doc tests are forbidden — write #[cfg(test)] unit tests or tests/ instead.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

python3 - "$@" << 'PYEOF'
import sys, re, glob

violations = []

for path in glob.glob('crates/**/*.rs', recursive=True):
    if '/target/' in path:
        continue
    with open(path) as f:
        lines = f.readlines()

    in_doc_comment = False
    in_code_block = False
    fence_line = 0

    for i, line in enumerate(lines, 1):
        stripped = line.lstrip()
        is_doc = stripped.startswith('///') or stripped.startswith('//!')

        if is_doc and not in_doc_comment:
            in_doc_comment = True

        if not is_doc:
            in_doc_comment = False
            in_code_block = False
            continue

        # Extract content after /// or //!
        comment = '///' if stripped.startswith('///') else '//!'
        content = stripped[len(comment):].lstrip()

        # Check for code fence
        if content.startswith('```'):
            fence = content[3:].strip()  # everything after ```

            if not in_code_block:
                # Opening fence — check if it would be compiled as Rust
                # Active doc tests: bare ```, ```rust, ```no_run, ```rust,no_run
                # Inactive: ```text, ```ignore, ```json, ```toml, ```bash, ```rust,ignore
                is_active = (
                    fence == '' or
                    fence == 'rust' or
                    fence == 'no_run' or
                    fence == 'rust,no_run'
                )
                if is_active:
                    violations.append(f"{path}:{i}:{line.rstrip()}")
                in_code_block = True
                fence_line = i
            else:
                # Closing fence
                in_code_block = False
        elif in_code_block:
            # Content inside code block — fine
            pass

if violations:
    print(f"\033[0;31m✗ Found {len(violations)} active doc test(s) — doc tests are forbidden\033[0m")
    print(f"\033[0;31m  Convert them to #[cfg(test)] unit tests or tests/ integration tests\033[0m")
    print(f"\033[0;31m  Or use ```text for documentation-only code blocks\033[0m")
    for v in violations:
        print(f"  {v}")
    sys.exit(1)

print("\033[0;32m✓ No active doc tests\033[0m")
PYEOF
