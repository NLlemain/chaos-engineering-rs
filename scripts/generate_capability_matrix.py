#!/usr/bin/env python3
"""Generate the README capability matrix from the runtime injector registry."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys

BEGIN = "<!-- BEGIN GENERATED CAPABILITY MATRIX -->"
END = "<!-- END GENERATED CAPABILITY MATRIX -->"


def load_registry(repository: pathlib.Path) -> list[dict[str, object]]:
    process = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "-p",
            "chaos_cli",
            "--",
            "list",
            "--json",
        ],
        cwd=repository,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    registry = json.loads(process.stdout)
    if not isinstance(registry, list):
        raise ValueError("chaos list --json did not return an array")
    return registry


def render(registry: list[dict[str, object]]) -> str:
    lines = [
        BEGIN,
        "| Injector | Status | Required capabilities |",
        "|---|---|---|",
    ]
    for injector in sorted(registry, key=lambda item: str(item["name"])):
        capabilities = injector.get("required_capabilities")
        if not isinstance(capabilities, list):
            raise ValueError(f"{injector.get('name')}: required_capabilities is not an array")
        requirement = ", ".join(str(value) for value in capabilities) or "None"
        lines.append(
            f"| `{injector['name']}` | {injector['status']} | {requirement} |"
        )
    lines.append(END)
    return "\n".join(lines)


def replace_block(readme: str, generated: str) -> str:
    start = readme.find(BEGIN)
    end = readme.find(END)
    if start < 0 or end < 0 or end < start:
        raise ValueError("README capability matrix markers are missing or out of order")
    return readme[:start] + generated + readme[end + len(END) :]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail when README is stale")
    args = parser.parse_args()

    repository = pathlib.Path(__file__).resolve().parent.parent
    readme_path = repository / "README.md"
    current = readme_path.read_text(encoding="utf-8")
    expected = replace_block(current, render(load_registry(repository)))
    if current == expected:
        print("README capability matrix is current.")
        return 0
    if args.check:
        print(
            "README capability matrix is stale; run scripts/generate_capability_matrix.py",
            file=sys.stderr,
        )
        return 1
    readme_path.write_text(expected, encoding="utf-8", newline="\n")
    print("Updated README capability matrix from the runtime registry.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
