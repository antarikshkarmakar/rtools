#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
capability_doc="$repo_root/docs/operations/capabilities.md"
doctor_json="$(mktemp)"
trap 'rm -f -- "$doctor_json"' EXIT

(
    cd -- "$repo_root"
    cargo run --locked --quiet -p rtools-cli -- --output-format json doctor
) >"$doctor_json"

python3 - "$capability_doc" "$doctor_json" <<'PY'
import json
import re
import sys
from pathlib import Path

doc_path = Path(sys.argv[1])
doctor_path = Path(sys.argv[2])
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

print(f"verified {len(runtime_rows)} sorted capability rows")
PY
