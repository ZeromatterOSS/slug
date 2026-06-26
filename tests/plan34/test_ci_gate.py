from pathlib import Path

import yaml


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github/workflows/build-and-test.yml"
SETUP_ACTION = REPO_ROOT / ".github/actions/setup_plan34_nativelink/action.yml"


def _load_yaml(path: Path) -> dict:
    with path.open(encoding="utf-8") as f:
        loaded = yaml.safe_load(f)
    assert isinstance(loaded, dict)
    return loaded


def _step_uses(job: dict) -> list[str]:
    steps = job.get("steps")
    assert isinstance(steps, list)
    uses: list[str] = []
    for step in steps:
        assert isinstance(step, dict)
        value = step.get("uses")
        if isinstance(value, str):
            uses.append(value)
    return uses


def test_linux_ci_provisions_nativelink_before_plan34_smoke() -> None:
    workflow = _load_yaml(WORKFLOW)
    linux_job = workflow["jobs"]["linux-build-and-test"]

    uses = _step_uses(linux_job)
    setup_index = uses.index("./.github/actions/setup_plan34_nativelink")
    test_index = uses.index("./.github/actions/run_test_py")

    assert setup_index < test_index


def test_plan34_nativelink_setup_action_exports_smoke_binary() -> None:
    action = _load_yaml(SETUP_ACTION)
    steps = action["runs"]["steps"]

    assert action["inputs"]["nativelink_ref"]["default"] == "v1.5.2"
    assert (
        action["inputs"]["nativelink_expected_sha"]["default"]
        == "6e63ef9a567ac49c77ab258f3af9331336868bb0"
    )

    build_steps = [
        step
        for step in steps
        if isinstance(step, dict) and step.get("name") == "Build NativeLink binary"
    ]
    assert len(build_steps) == 1
    run_script = build_steps[0]["run"]

    assert (
        "cargo +stable build --bin nativelink --profile=smol --locked" in run_script
    )
    assert "target/smol/nativelink" in run_script
    assert "SLUG_PLAN34_NATIVELINK_BIN=$bin" in run_script
    assert ">> \"$GITHUB_ENV\"" in run_script
