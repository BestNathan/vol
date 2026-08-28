#!/usr/bin/env python3
"""
Check that every *Operation variant defined in agent_server_protocol.rs
is registered in operation_codec.rs (method_to_operation) and has a
method_name() match arm.

This prevents the bug where a new operation variant is added but the
codec doesn't know how to resolve it from a method name string, causing
"unknown method" errors at runtime.

Usage:
    ./scripts/check-protocol-registration.sh

Exit codes:
    0 — all operations registered
    1 — missing registration(s) found
"""

import re
import sys
from pathlib import Path

PROTOCOL_FILE = Path("crates/vol-llm-agent-protocol/src/agent_server_protocol.rs")
CODEC_FILE = Path("crates/vol-llm-agent-protocol/src/operation_codec.rs")


def extract_operation_enums(content: str) -> dict[str, list[str]]:
    """Extract all *Operation enums that are referenced in the Operation enum."""
    # First find the Operation enum to see which *Operation types are used
    op_enum_pattern = re.compile(
        r"pub enum Operation\s*\{([^}]+)\}", re.MULTILINE | re.DOTALL
    )
    op_match = op_enum_pattern.search(content)
    if not op_match:
        return {}

    op_body = op_match.group(1)
    # Find referenced operation types like Sandbox(SandboxOperation)
    referenced = re.findall(r"\w+\((\w+Operation)\)", op_body)

    enums = {}
    for enum_name in referenced:
        # Match the specific enum definition
        enum_pattern = re.compile(
            rf"pub enum {re.escape(enum_name)}\s*\{{([^}}]+)\}}",
            re.MULTILINE | re.DOTALL,
        )
        match = enum_pattern.search(content)
        if match:
            body = match.group(1)
            # Extract variant names
            variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]+)", body, re.MULTILINE)
            enums[enum_name] = variants

    return enums


def extract_inner_method_names(content: str, enum_name: str) -> dict[str, str]:
    """Extract method_name() mappings from an inner *Operation enum's impl block."""
    # Find impl block for this enum
    impl_pattern = re.compile(
        rf"impl {re.escape(enum_name)}\s*\{{.*?pub fn method_name.*?\}}.*?\}}",
        re.MULTILINE | re.DOTALL,
    )
    match = impl_pattern.search(content)
    if not match:
        return {}

    impl_body = match.group(0)
    # Match Self::Variant => "method.name"
    pattern = re.compile(r'Self::(\w+)\s*=>\s*"([^"]+)"')
    return {m.group(1): m.group(2) for m in pattern.finditer(impl_body)}


def extract_method_name_mappings(content: str) -> dict[str, str]:
    """Extract method_name() mappings from Operation::Xxx(Yyy::Zzz) => "method.name"."""
    # Match patterns like:
    #   Operation::Sandbox(SandboxOperation::List) => "sandbox.list",
    #   Operation::Control(ControlOperation::CapabilitySnapshot) => {
    #       "control.capability_snapshot"
    #   }
    #   Self::Sandbox(op) => op.method_name(),

    # Single-line pattern
    pattern = re.compile(
        r'Operation::(\w+)\((\w+Operation::(\w+))\)\s*=>\s*"([^"]+)"',
        re.MULTILINE,
    )

    mappings = {}
    for match in pattern.finditer(content):
        operation_type = match.group(1)  # e.g. "Sandbox"
        enum_ref = match.group(2)  # e.g. "SandboxOperation::List"
        variant = match.group(3)  # e.g. "List"
        method_name = match.group(4)  # e.g. "sandbox.list"
        key = f"{operation_type}::{variant}"
        mappings[key] = method_name

    # Multi-line pattern for cases like:
    #   Operation::Control(ControlOperation::CapabilitySnapshot) => {
    #       "control.capability_snapshot"
    #   }
    multiline_pattern = re.compile(
        r'Operation::(\w+)\((\w+Operation::(\w+))\)\s*=>\s*\{\s*"([^"]+)"\s*\}',
        re.MULTILINE | re.DOTALL,
    )
    for match in multiline_pattern.finditer(content):
        operation_type = match.group(1)
        variant = match.group(3)
        method_name = match.group(4)
        key = f"{operation_type}::{variant}"
        mappings[key] = method_name

    # Also detect delegation patterns like:
    #   Self::Sandbox(op) => op.method_name(),
    # These mean the inner *Operation enum defines its own method_name()
    delegation_pattern = re.compile(
        r'Self::(\w+)\(op\)\s*=>\s*op\.method_name\(\)',
        re.MULTILINE,
    )
    delegated_types = set()
    for match in delegation_pattern.finditer(content):
        delegated_types.add(match.group(1))  # e.g. "Sandbox", "Control"

    return mappings, delegated_types


def extract_codec_registrations(content: str) -> set[str]:
    """Extract method name strings registered in method_to_operation."""
    # Match patterns like:
    #   "sandbox.list" => Ok(Operation::Sandbox(SandboxOperation::List)),
    #   "control.capability_snapshot" => {
    #       Ok(Operation::Control(ControlOperation::CapabilitySnapshot))
    #   }
    # Single-line
    pattern = re.compile(r'"([^"]+)"\s*=>\s*Ok\(Operation::(\w+)\((\w+Operation::(\w+))\)\)')

    registered = set()
    for match in pattern.finditer(content):
        method_name = match.group(1)
        registered.add(method_name)

    # Multi-line (with braces)
    multiline_pattern = re.compile(
        r'"([^"]+)"\s*=>\s*\{\s*Ok\(Operation::(\w+)\((\w+Operation::(\w+))\)\)\s*\}',
        re.MULTILINE | re.DOTALL,
    )
    for match in multiline_pattern.finditer(content):
        method_name = match.group(1)
        registered.add(method_name)

    return registered


def main():
    if not PROTOCOL_FILE.exists():
        print(f"ERROR: {PROTOCOL_FILE} not found", file=sys.stderr)
        sys.exit(1)

    if not CODEC_FILE.exists():
        print(f"ERROR: {CODEC_FILE} not found", file=sys.stderr)
        sys.exit(1)

    protocol_content = PROTOCOL_FILE.read_text()
    codec_content = CODEC_FILE.read_text()

    enums = extract_operation_enums(protocol_content)
    method_mappings, delegated_types = extract_method_name_mappings(protocol_content)
    codec_registrations = extract_codec_registrations(codec_content)

    errors = []

    for enum_name, variants in enums.items():
        # Convert enum name like "SandboxOperation" to "Sandbox"
        op_type = enum_name.replace("Operation", "")
        is_delegated = op_type in delegated_types

        # For delegated types, get method names from the inner enum's impl block
        inner_methods = {}
        if is_delegated:
            inner_methods = extract_inner_method_names(protocol_content, enum_name)

        for variant in variants:
            key = f"{op_type}::{variant}"

            # Check if method_name() is defined (either direct or delegated)
            if is_delegated:
                if variant not in inner_methods:
                    errors.append(
                        f"  {key}: missing method_name() arm in {enum_name} impl"
                    )
                    continue
                method_name = inner_methods[variant]
            else:
                if key not in method_mappings:
                    errors.append(
                        f"  {key}: missing method_name() match arm in agent_server_protocol.rs"
                    )
                    continue
                method_name = method_mappings[key]

            # Check if registered in codec
            if method_name not in codec_registrations:
                errors.append(
                    f"  {key} ({method_name}): missing registration in operation_codec.rs"
                )

    if errors:
        print("ERROR: Protocol operations not fully registered in codec\n")
        print("The following operations are missing registrations:\n")
        for err in errors:
            print(err)
        print("\nTo fix:")
        print("  1. Add method_name() arm in agent_server_protocol.rs")
        print("  2. Add reverse mapping in operation_codec.rs method_to_operation()")
        print("  3. Add payload decode branch in Payload::from_operation()")
        sys.exit(1)
    else:
        total = sum(len(v) for v in enums.values())
        print(f"OK: All {total} operation variants registered in codec")
        sys.exit(0)


if __name__ == "__main__":
    main()
