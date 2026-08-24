#!/usr/bin/env python3
"""Sync runtime configuration into Kubernetes ConfigMap manifests.

Reads source files from the repository and generates per-entity ConfigMap YAML
manifests under deploy/argocd/manifests/runtime-config/{category}/.

Source → ConfigMap mapping (one ConfigMap per entity):
  .agents/agents/*.md       → agents/agent-def-{name}.yaml       (key: {name}.md)
  .agents/providers/*.toml  → providers/provider-{name}.yaml     (key: {name}.toml)
  .agents/skills/*/SKILL.md → skills/skill-{name}.yaml           (key: SKILL.md)
  .agents/sandboxes/*.toml  → sandboxes/sandbox-{name}.yaml      (key: {name}.toml)
  .agents/cli-tools/*.toml  → cli-tools/cli-tool-{name}.yaml     (key: {name}.toml)
  .mcp.json                 → mcp-configmap.yaml                 (key: mcp.json)

Also writes runtime-config/.summary.json — a machine-readable manifest of all
generated ConfigMap names grouped by category. CI / developers use this to keep
workload projected-volume sources in sync.

Usage:
  python3 scripts/sync-configmaps.py          # generate ConfigMaps
  python3 scripts/sync-configmaps.py --check  # dry-run, exit 1 if changes needed
"""

import argparse
import json
import shutil
import sys
from pathlib import Path

import yaml

MANIFEST_DIR = Path("deploy/argocd/manifests/runtime-config")
NAMESPACE = "vol-agent-system"

COMPONENT_LABEL = "runtime-config"


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
                "app.kubernetes.io/component": COMPONENT_LABEL,
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
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(yaml_str(cm))


def _clean_category(category: str) -> None:
    """Remove all YAML files in a category subdirectory."""
    cat_dir = MANIFEST_DIR / category
    if cat_dir.is_dir():
        for f in cat_dir.glob("*.yaml"):
            f.unlink()


def generate() -> dict:
    """Generate all per-entity ConfigMap manifests.

    Returns a summary dict mapping category → list of ConfigMap names.
    """
    MANIFEST_DIR.mkdir(parents=True, exist_ok=True)

    summary: dict[str, list[str]] = {
        "agents": [],
        "sandboxes": [],
        "providers": [],
        "cli_tools": [],
        "skills": [],
        "mcp": [],
    }

    # ── agents ────────────────────────────────────────────────────────────
    _clean_category("agents")
    agents_dir = Path(".agents/agents")
    if agents_dir.is_dir():
        for f in sorted(agents_dir.glob("*.md")):
            name = f.stem
            cm_name = f"agent-def-{name}"
            cm = cm_header(cm_name)
            cm["data"][f"{name}.md"] = f.read_text()
            write_cm(MANIFEST_DIR / "agents" / f"{cm_name}.yaml", cm)
            summary["agents"].append(cm_name)

    # ── providers ─────────────────────────────────────────────────────────
    _clean_category("providers")
    providers_dir = Path(".agents/providers")
    if providers_dir.is_dir():
        for f in sorted(providers_dir.glob("*.toml")):
            name = f.stem
            cm_name = f"provider-{name}"
            cm = cm_header(cm_name)
            cm["data"][f"{name}.toml"] = f.read_text()
            write_cm(MANIFEST_DIR / "providers" / f"{cm_name}.yaml", cm)
            summary["providers"].append(cm_name)

    # ── skills ────────────────────────────────────────────────────────────
    _clean_category("skills")
    skills_dir = Path(".agents/skills")
    if skills_dir.is_dir():
        for skill_dir in sorted(skills_dir.iterdir()):
            if not skill_dir.is_dir():
                continue
            skill_md = skill_dir / "SKILL.md"
            if not skill_md.is_file():
                continue
            name = skill_dir.name
            cm_name = f"skill-{name}"
            cm = cm_header(cm_name)
            cm["data"]["SKILL.md"] = skill_md.read_text()
            write_cm(MANIFEST_DIR / "skills" / f"{cm_name}.yaml", cm)
            summary["skills"].append(cm_name)

    # ── sandboxes ─────────────────────────────────────────────────────────
    _clean_category("sandboxes")
    sandboxes_dir = Path(".agents/sandboxes")
    if sandboxes_dir.is_dir():
        for f in sorted(sandboxes_dir.glob("*.toml")):
            name = f.stem
            cm_name = f"sandbox-{name}"
            cm = cm_header(cm_name)
            cm["data"][f"{name}.toml"] = f.read_text()
            write_cm(MANIFEST_DIR / "sandboxes" / f"{cm_name}.yaml", cm)
            summary["sandboxes"].append(cm_name)

    # ── cli-tools ─────────────────────────────────────────────────────────
    _clean_category("cli-tools")
    cli_tools_dir = Path(".agents/cli-tools")
    if cli_tools_dir.is_dir():
        for f in sorted(cli_tools_dir.glob("*.toml")):
            name = f.stem
            cm_name = f"cli-tool-{name}"
            cm = cm_header(cm_name)
            cm["data"][f"{name}.toml"] = f.read_text()
            write_cm(MANIFEST_DIR / "cli-tools" / f"{cm_name}.yaml", cm)
            summary["cli_tools"].append(cm_name)

    # ── mcp (singleton — stays as one ConfigMap) ─────────────────────────
    # Remove old monolithic category ConfigMaps (superseded by per-entity files).
    # Only remove known generated files — never touch hand-maintained root files
    # like namespace.yaml, ghcr-image-pull-secret.yaml, etc.
    for stale in ("agents-configmap.yaml", "providers-configmap.yaml",
                  "skills-configmap.yaml", "sandboxes-configmap.yaml",
                  "cli-tools-configmap.yaml"):
        (MANIFEST_DIR / stale).unlink(missing_ok=True)

    mcp_file = Path(".mcp.json")
    if mcp_file.is_file():
        cm = cm_header("mcp-config")
        cm["data"]["mcp.json"] = mcp_file.read_text()
        write_cm(MANIFEST_DIR / "mcp" / "mcp-configmap.yaml", cm)
        summary["mcp"].append("mcp-config")

    # ── summary ───────────────────────────────────────────────────────────
    summary_path = MANIFEST_DIR / ".summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n")

    return summary


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Sync runtime config into per-entity ConfigMap manifests"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit with non-zero if ConfigMaps are out of date",
    )
    args = parser.parse_args()

    if args.check:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            # Snapshot current state
            tmp_path = Path(tmpdir)
            if MANIFEST_DIR.is_dir():
                shutil.copytree(MANIFEST_DIR, tmp_path / "current")
            # Generate fresh
            generate()
            # Compare
            import filecmp
            dc = filecmp.dircmp(str(tmp_path / "current"), str(MANIFEST_DIR))
            if dc.left_only or dc.right_only or dc.diff_files:
                print("ConfigMaps are out of date. Run without --check to regenerate.")
                print(f"  Only in current: {dc.left_only}")
                print(f"  Only in generated: {dc.right_only}")
                print(f"  Differ: {dc.diff_files}")
                sys.exit(1)
            print("ConfigMaps are up to date.")
            sys.exit(0)

    print("Syncing runtime config → per-entity ConfigMap manifests ...")
    summary = generate()
    for cat, names in summary.items():
        if names:
            print(f"  {cat}: {len(names)} ConfigMap(s) — {', '.join(names)}")
    total = sum(len(v) for v in summary.values())
    print(f"Generated {total} ConfigMap(s) + .summary.json")


if __name__ == "__main__":
    main()
