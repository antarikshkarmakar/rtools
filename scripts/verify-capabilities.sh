#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
capability_doc="${RTOOLS_CAPABILITY_DOC:-$repo_root/docs/operations/capabilities.md}"
mcp_doc="${RTOOLS_MCP_DOC:-$repo_root/docs/MCP.md}"
doctor_json="$(mktemp)"
mcp_contract_json="$(mktemp)"
trap 'rm -f -- "$doctor_json" "$mcp_contract_json"' EXIT

(
    cd -- "$repo_root"
    cargo run --locked --quiet -p rtools-cli -- --output-format json doctor
) >"$doctor_json"

(
    cd -- "$repo_root"
    cargo run --locked --quiet -p rtools-mcp -- --print-contracts
) >"$mcp_contract_json"

python3 - "$capability_doc" "$doctor_json" "$mcp_doc" "$mcp_contract_json" <<'PY'
import json
import re
import sys
from pathlib import Path

doc_path = Path(sys.argv[1])
doctor_path = Path(sys.argv[2])
mcp_path = Path(sys.argv[3])
mcp_contract_path = Path(sys.argv[4])
row_pattern = re.compile(
    r"^\|\s*`(?P<operation>[a-z0-9._-]+)`\s*"
    r"\|\s*`(?P<state>available|experimental|unavailable)`\s*\|"
)


def fail(message: str) -> None:
    raise SystemExit(f"capability verification failed: {message}")


try:
    report = json.loads(doctor_path.read_text(encoding="utf-8"))
    capabilities = report["result"]["capabilities"]
except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
    fail(f"doctor did not emit the expected JSON report: {error}")

runtime_rows = []
for capability in capabilities:
    try:
        runtime_rows.append((capability["operation_id"], capability["state"]))
    except (KeyError, TypeError) as error:
        fail(f"doctor capability is malformed: {error}")

try:
    doc_rows = [
        (match.group("operation"), match.group("state"))
        for line in doc_path.read_text(encoding="utf-8").splitlines()
        if (match := row_pattern.match(line))
    ]
except OSError as error:
    fail(f"cannot read {doc_path}: {error}")

for label, rows in (("doctor JSON", runtime_rows), ("documentation", doc_rows)):
    operation_ids = [operation_id for operation_id, _ in rows]
    if not operation_ids:
        fail(f"{label} contains no capability rows")
    if operation_ids != sorted(operation_ids):
        fail(f"{label} operation IDs are not sorted")
    duplicates = sorted(
        operation_id
        for operation_id in set(operation_ids)
        if operation_ids.count(operation_id) > 1
    )
    if duplicates:
        fail(f"{label} contains duplicate operation IDs: {', '.join(duplicates)}")

runtime = dict(runtime_rows)
documented = dict(doc_rows)
missing = sorted(runtime.keys() - documented.keys())
extra = sorted(documented.keys() - runtime.keys())
misclassified = sorted(
    operation_id
    for operation_id in runtime.keys() & documented.keys()
    if runtime[operation_id] != documented[operation_id]
)

problems = []
if missing:
    problems.append(f"missing from documentation: {', '.join(missing)}")
if extra:
    problems.append(f"extra in documentation: {', '.join(extra)}")
if misclassified:
    details = ", ".join(
        f"{operation_id} (runtime={runtime[operation_id]}, docs={documented[operation_id]})"
        for operation_id in misclassified
    )
    problems.append(f"misclassified: {details}")
if problems:
    fail("; ".join(problems))

mcp_row_pattern = re.compile(
    r"^\|\s*`(?P<tool>[a-z0-9_]+)`\s*"
    r"\|\s*`(?P<operation>[a-z0-9._-]+)`\s*"
    r"\|\s*`(?P<state>available|experimental|unavailable)`\s*"
    r"\|(?P<contract>.*)\|$"
)
try:
    exported = json.loads(mcp_contract_path.read_text(encoding="utf-8"))
    if exported.get("version") != 1 or not isinstance(exported.get("tools"), list):
        fail("MCP runtime contract has an unsupported shape")
    runtime_mcp_rows = exported["tools"]
except (OSError, json.JSONDecodeError, AttributeError) as error:
    fail(f"cannot read MCP runtime contract: {error}")

runtime_mcp_names = []
for row in runtime_mcp_rows:
    try:
        tool = row["tool"]
        operation = row["operation_id"]
        state = row["state"]
        adapter_contract = row["adapter_contract"]
        structured_errors = row["structured_errors"]
    except (KeyError, TypeError) as error:
        fail(f"MCP runtime contract row is malformed: {error}")
    if not all(isinstance(value, str) and value for value in (tool, operation, state, adapter_contract)):
        fail("MCP runtime contract contains an empty or non-string field")
    if state not in {"available", "experimental", "unavailable"}:
        fail(f"MCP runtime contract has invalid state for {tool}: {state}")
    if structured_errors is not True:
        fail(f"MCP runtime contract disables structured errors for {tool}")
    runtime_mcp_names.append(tool)

runtime_mcp_duplicates = sorted(
    tool for tool in set(runtime_mcp_names) if runtime_mcp_names.count(tool) > 1
)
if runtime_mcp_duplicates:
    fail(f"MCP runtime contract contains duplicate tools: {', '.join(runtime_mcp_duplicates)}")

try:
    documented_mcp_rows = [
        (
            match.group("tool"),
            match.group("operation"),
            match.group("state"),
            match.group("contract"),
        )
        for line in mcp_path.read_text(encoding="utf-8").splitlines()
        if (match := mcp_row_pattern.match(line))
    ]
except OSError as error:
    fail(f"cannot read {mcp_path}: {error}")

documented_mcp_names = [tool for tool, _, _, _ in documented_mcp_rows]
documented_mcp_duplicates = sorted(
    tool for tool in set(documented_mcp_names) if documented_mcp_names.count(tool) > 1
)
if documented_mcp_duplicates:
    fail(f"MCP documentation contains duplicate tools: {', '.join(documented_mcp_duplicates)}")
if documented_mcp_names != runtime_mcp_names:
    fail("MCP adapter contract tools differ from the verified tool set")

for runtime_row, documented_row in zip(runtime_mcp_rows, documented_mcp_rows, strict=True):
    tool, operation, state, contract = documented_row
    expected_operation = runtime_row["operation_id"]
    if operation != expected_operation:
        fail(f"MCP tool {tool} maps to {operation}, expected {expected_operation}")
    if state != runtime_row["state"]:
        fail(f"MCP tool {tool} state {state} differs from runtime MCP state {runtime_row['state']}")
    if documented.get(operation) != state:
        fail(f"MCP tool {tool} state {state} differs from capability {documented.get(operation)}")
    normalized_contract = contract.replace("`", "").replace("\\|", "|").strip()
    expected_contract = f"{runtime_row['adapter_contract']}; structured_errors=true"
    if normalized_contract != expected_contract:
        fail(
            f"MCP tool {tool} contract differs from runtime "
            f"(runtime={expected_contract!r}, docs={normalized_contract!r})"
        )

print(
    f"verified {len(runtime_rows)} sorted capability rows and "
    f"{len(runtime_mcp_rows)} runtime-derived MCP adapter contracts"
)
PY

(
    cd -- "$repo_root"
    cargo test --locked -p rtools-mcp mcp_contract
    cargo test --locked -p rtools-api recognized_but_unavailable_options_return_structured_501
)
