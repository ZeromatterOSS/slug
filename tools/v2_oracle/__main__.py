#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.v2_oracle_lib.compare import compare_result, load_expected, write_expected, write_failure_artifacts
from tools.v2_oracle_lib.evidence import validate_evidence
from tools.v2_oracle_lib.fixture import discover_fixtures, find_fixture
from tools.v2_oracle_lib.runner import RunOptions, ToolConfig, default_run_root, run_fixture

DEFAULT_FIXTURES_ROOT = REPO_ROOT / "tests" / "v2_oracle" / "fixtures"


def _path_arg(value: str | None) -> Path | None:
    if not value:
        return None
    return Path(value)


def _selected_tools(args: argparse.Namespace) -> list[ToolConfig]:
    bazel = _path_arg(args.bazel or os.environ.get("BAZEL_BIN"))
    slug = _path_arg(args.slug or os.environ.get("SLUG_V2_BIN"))
    tools: list[ToolConfig] = []
    if args.tool in ("auto", "both", "bazel") and bazel is not None:
        tools.append(ToolConfig("bazel", bazel))
    if args.tool in ("auto", "both", "slug") and slug is not None:
        tools.append(ToolConfig("slug", slug))
    if args.tool == "bazel" and bazel is None:
        raise SystemExit("--tool bazel requires --bazel or BAZEL_BIN")
    if args.tool == "slug" and slug is None:
        raise SystemExit("--tool slug requires --slug or SLUG_V2_BIN")
    if args.tool == "both" and (bazel is None or slug is None):
        raise SystemExit("--tool both requires Bazel and Slug binaries")
    if not tools:
        raise SystemExit("no tool selected; pass --bazel, --slug, BAZEL_BIN, or SLUG_V2_BIN")
    return tools


def cmd_list(args: argparse.Namespace) -> int:
    fixtures = discover_fixtures(Path(args.fixtures_root))
    if args.json:
        print(json.dumps([{"name": fixture.name, "description": fixture.description} for fixture in fixtures], indent=2))
    else:
        for fixture in fixtures:
            print(fixture.name)
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    fixture = find_fixture(Path(args.fixtures_root), args.fixture)
    run_root = Path(args.run_root) if args.run_root else default_run_root()
    options = RunOptions(run_root=run_root, timeout_seconds=args.timeout)
    tools = _selected_tools(args)
    expected = load_expected(fixture.expected_oracle)
    failures: list[str] = []
    summaries: list[dict[str, object]] = []

    for tool in tools:
        result = run_fixture(fixture, tool, options)
        if args.update_expected and tool.name == "bazel":
            write_expected(fixture.expected_oracle, result)
            expected = load_expected(fixture.expected_oracle)
        tool_failures = compare_result(fixture, result, expected)
        if tool_failures:
            artifact = write_failure_artifacts(Path(result["run_dir"]), fixture, tool_failures, result, expected)
            failures.extend(f"{tool.name}: {failure}" for failure in tool_failures)
            summaries.append({"tool": tool.name, "status": "failed", "artifact": str(artifact)})
        else:
            summaries.append({"tool": tool.name, "status": "ok", "run_dir": result["run_dir"]})

    print(json.dumps({"fixture": fixture.name, "results": summaries}, indent=2, sort_keys=True))
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0



def cmd_validate_evidence(args: argparse.Namespace) -> int:
    failures = validate_evidence(Path(args.path))
    if failures:
        print(json.dumps({"status": "failed", "failures": failures}, indent=2, sort_keys=True))
        return 1
    print(json.dumps({"status": "ok", "path": args.path}, indent=2, sort_keys=True))
    return 0

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="v2_oracle", description="Run Slug V2 oracle fixtures")
    subcommands = parser.add_subparsers(dest="command", required=True)

    list_parser = subcommands.add_parser("list", help="list fixture names")
    list_parser.add_argument("--fixtures-root", default=str(DEFAULT_FIXTURES_ROOT))
    list_parser.add_argument("--json", action="store_true")
    list_parser.set_defaults(func=cmd_list)

    run_parser = subcommands.add_parser("run", help="run one fixture")
    run_parser.add_argument("--fixture", required=True)
    run_parser.add_argument("--fixtures-root", default=str(DEFAULT_FIXTURES_ROOT))
    run_parser.add_argument("--bazel", help="path to upstream Bazel 9 binary")
    run_parser.add_argument("--slug", help="path to Slug V2 binary; defaults to SLUG_V2_BIN")
    run_parser.add_argument("--tool", choices=("auto", "bazel", "slug", "both"), default="auto")
    run_parser.add_argument("--update-expected", action="store_true")
    run_parser.add_argument("--run-root", help="artifact root; defaults to temp/slug-v2-oracle")
    run_parser.add_argument("--timeout", type=int, default=120)
    run_parser.set_defaults(func=cmd_run)

    evidence_parser = subcommands.add_parser("validate-evidence", help="validate REAPI evidence JSONL")
    evidence_parser.add_argument("path")
    evidence_parser.set_defaults(func=cmd_validate_evidence)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
