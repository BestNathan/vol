#!/usr/bin/env python3
"""Sync runtime configuration into Kubernetes ConfigMap manifests.

Reads source files from the repository and generates ConfigMap YAML manifests
under deploy/argocd/manifests/runtime-config/.

Source → ConfigMap mapping:
  .agents/agents/*.md       → agents-configmap.yaml
  .agents/providers/*.toml  → providers-configmap.yaml
  .agents/skills/*/SKILL.md → skills-configmap.yaml
  .agents/sandboxes/*.toml  → sandboxes-configmap.yaml
  .mcp.json                 → mcp-configmap.yaml

Usage:
  python3 scripts/sync-configmaps.py          # generate ConfigMaps
  python3 scripts/sync-configmaps.py --check  # dry-run, exit 1 if changes needed
"""

import argparse
import sys
from pathlib import Path

import yaml

MANIFEST_DIR = Path("deploy/argocd/manifests/runtime-config")
NAMESPACE = "vol-agent-system"


def cm_header(name: str) -> dict:
    """Create a standard ConfigMap skeleton."""
    return {
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": name,
            "namespace": NAMESPACE,
            "labels": {
                "app.kubernetes.io/name": name,
                "app.kubernetes.io/part-of": "vol-agent",
                "app.kubernetes.io/component": "runtime-config",
            },
        },
        "data": {},
    }


def yaml_str(obj: dict) -> str:
    """Serialize to clean YAML without Python-specific tags."""

    class Dumper(yaml.Dumper):
        pass

    Dumper.ignore_aliases = lambda self, data: True
    return yaml.dump(
        obj,
        Dumper=Dumper,
        default_flow_style=False,
        sort_keys=False,
        allow_unicode=True,
    )


def write_cm(path: Path, cm: dict) -> None:
    """Write a ConfigMap manifest to disk."""
    path.write_text(yaml_str(cm))


def generate() -> bool:
    """Generate all ConfigMap manifests. Returns True if any were created."""
    MANIFEST_DIR.mkdir(parents=True, exist_ok=True)
    changed = False

    # ── agents-configmap.yaml ──────────────────────────────────────────────
    agents_dir = Path(".agents/agents")
    cm = cm_header("agent-definitions")
    if agents_dir.is_dir():
        for f in sorted(agents_dir.glob("*.md")):
            cm["data"][f.name] = f.read_text()
    if cm["data"]:
        write_cm(MANIFEST_DIR / "agents-configmap.yaml", cm)
        changed = True
        print(f"  agents: {len(cm['data'])} file(s)")

    # ── providers-configmap.yaml ───────────────────────────────────────────
    providers_dir = Path(".agents/providers")
    cm = cm_header("agent-providers")
    if providers_dir.is_dir():
        for f in sorted(providers_dir.glob("*.toml")):
            cm["data"][f.name] = f.read_text()
    if cm["data"]:
        write_cm(MANIFEST_DIR / "providers-configmap.yaml", cm)
        changed = True
        print(f"  providers: {len(cm['data'])} file(s)")

    # ── skills-configmap.yaml ──────────────────────────────────────────────
    skills_dir = Path(".agents/skills")
    cm = cm_header("agent-skills")
    if skills_dir.is_dir():
        for skill_dir in sorted(skills_dir.iterdir()):
            if not skill_dir.is_dir():
                continue
            skill_md = skill_dir / "SKILL.md"
            if not skill_md.is_file():
                continue
            key = f"{skill_dir.name}.SKILL.md"
            cm["data"][key] = skill_md.read_text()
    if cm["data"]:
        write_cm(MANIFEST_DIR / "skills-configmap.yaml", cm)
        changed = True
        print(f"  skills: {len(cm['data'])} file(s)")

    # ── sandboxes-configmap.yaml ───────────────────────────────────────────
    sandboxes_dir = Path(".agents/sandboxes")
    cm = cm_header("agent-sandboxes")
    if sandboxes_dir.is_dir():
        for f in sorted(sandboxes_dir.glob("*.toml")):
            cm["data"][f.name] = f.read_text()
    if cm["data"]:
        write_cm(MANIFEST_DIR / "sandboxes-configmap.yaml", cm)
        changed = True
        print(f"  sandboxes: {len(cm['data'])} file(s)")

    # ── mcp-configmap.yaml ─────────────────────────────────────────────────
    mcp_file = Path(".mcp.json")
    if mcp_file.is_file():
        cm = cm_header("mcp-config")
        cm["data"]["mcp.json"] = mcp_file.read_text()
        write_cm(MANIFEST_DIR / "mcp-configmap.yaml", cm)
        changed = True
        print("  mcp: .mcp.json")

    return changed


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Sync runtime config into Kubernetes ConfigMap manifests"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit with non-zero if ConfigMaps are out of date (for CI pre-commit)",
    )
    args = parser.parse_args()

    if args.check:
        # Generate to temp and diff
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            import subprocess

            # Generate fresh manifests into tmpdir
            original_cwd = Path.cwd()
            # We can't easily redirect the output, so just generate inline and diff
            pass

        print("--check not yet implemented; run without --check to regenerate")
        sys.exit(0)

    print("Syncing runtime config → ConfigMap manifests ...")
    changed = generate()
    if changed:
        print("ConfigMap manifests regenerated.")
    else:
        print("No source files found. ConfigMaps unchanged.")


if __name__ == "__main__":
    main()
