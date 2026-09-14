"""Tests for the 2026-09-14 dispatch fixes (plans/dispatch-self-healing-plan.md).
Run: uv run --project .claude/scripts --group dev pytest .claude/scripts/tests"""
import csv
import importlib.util
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


agent_loop = load("agent_loop")
dispatch_state = load("dispatch_state")
simple_dispatch = load("simple_dispatch")


def test_dedup_never_fires_after_edits():
    # tensor-transpose-build 36d59b9d: three attempts of real edits, the same
    # environmental tsc RED each time, classified STUCK. Never again.
    seen = ["FAIL tsc_accepts_the_declaration_and_consumer"]
    assert agent_loop.is_repeat_without_edits(True, seen[0], seen) is False
    assert agent_loop.is_repeat_without_edits(False, seen[0], seen) is True
    assert agent_loop.is_repeat_without_edits(False, "something new", seen) is False


def test_no_progress_cutoff_defaults_off():
    # The cutoff at 12 ended 46 of 54 runs mid-exploration. Default must equal --max-turns.
    src = (SCRIPTS / "agent_loop.py").read_text()
    assert '"--max-turns", type=int, default=30' in src
    assert '"--max-turns-without-progress", type=int, default=30' in src


def test_parse_self_report_json_and_prose():
    r = agent_loop.parse_self_report(
        'Sure. {"blocker_kind": "harness_cutoff", "narrower_would_succeed": false, '
        '"explanation": "I was still reading emit_go.rs when the attempt ended."}')
    assert r["blocker_kind"] == "harness_cutoff"
    assert r["narrower_would_succeed"] is False
    assert r["explanation"].startswith("I was still reading")
    prose = agent_loop.parse_self_report("I was still in the exploration phase.")
    assert prose == {"blocker_kind": "unclear", "narrower_would_succeed": None,
                     "explanation": "I was still in the exploration phase."}
    bad_kind = agent_loop.parse_self_report('{"blocker_kind": "banana", "explanation": "x"}')
    assert bad_kind["blocker_kind"] == "unclear"


def test_health_alarm_on_harness_streak_and_zero_edits():
    def row(i, ended_by, edits, status="STUCK"):
        return {"run_id": f"r{i}", "row_id": f"row{i}", "start_time": f"2026-09-13T1{i:02d}",
                "status": status, "ended_by": ended_by, "edits": str(edits)}
    healthy = [row(i, "model_done", 3, "GREEN") for i in range(5)]
    h = dispatch_state.health_from_rows(healthy, None, last=20)
    assert h["alarm"] is False and h["green"] == 5
    streak = healthy + [row(10 + i, "no_progress_cutoff", 0) for i in range(3)]
    h = dispatch_state.health_from_rows(streak, None, last=20)
    assert h["alarm"] is True and h["harness_streak"] == 3
    # An ack drawn after the streak silences it; the old rows are history, not signal.
    h = dispatch_state.health_from_rows(streak, {"time": "2026-09-13T199", "note": "fixed"}, last=20)
    assert h["alarm"] is False and h["window"] == 0
    # A GREEN run breaks the streak even if its last attempt hit max_turns.
    broken = streak + [row(20, "max_turns", 5, "GREEN")]
    h = dispatch_state.health_from_rows(broken, None, last=20)
    assert h["harness_streak"] == 0 and h["alarm"] is False
    # Unrecorded rows never count as healthy.
    unrecorded = [dict(row(i, "", 0), ended_by="") for i in range(6)]
    h = dispatch_state.health_from_rows(unrecorded, None, last=20)
    assert h["alarm"] is False and h["unrecorded"] == 6 and h["recorded"] == 0


def test_infer_ending_from_old_log():
    log = ("== attempt 1/3 ==\n  turn 1/30: prompt=1 completion=1\n  turn 2/30: prompt=1 completion=1\n"
           "  no repo changes for 12 consecutive turns, ending this attempt early\n== verify: RED ==\n"
           "(no changes, no verify run)\n== attempt 2/3 ==\n  turn 1/30: prompt=1 completion=1\n"
           "(no changes, no verify run)\nSTUCK: verify output matches a previous attempt, not retrying further\n")
    assert dispatch_state.infer_ending_from_log(log) == ("dedup", 0, 3)
    green = "== attempt 1/3 ==\n  turn 1/30: prompt=1 completion=1\n== verify: GREEN ==\nVERIFIED_GREEN\n"
    assert dispatch_state.infer_ending_from_log(green) == ("model_done", 1, 1)


def test_dispatch_log_header_migration(tmp_path, monkeypatch):
    csv_path = tmp_path / "dispatch-log.csv"
    csv_path.write_text("run_id,row_id,status\nabc,row1,STUCK\n")
    monkeypatch.setattr(simple_dispatch, "DISPATCH_LOG_PATH", csv_path)
    simple_dispatch.migrate_dispatch_log_header()
    with open(csv_path, newline="") as f:
        rows = list(csv.DictReader(f))
    assert list(rows[0].keys()) == simple_dispatch.DISPATCH_LOG_FIELDS
    assert rows[0]["run_id"] == "abc" and rows[0]["ended_by"] == ""
    # Idempotent.
    before = csv_path.read_text()
    simple_dispatch.migrate_dispatch_log_header()
    assert csv_path.read_text() == before


def test_require_green_preflight(tmp_path, monkeypatch):
    monkeypatch.setattr(simple_dispatch, "PREFLIGHT_DIR", tmp_path)
    ok, msg = simple_dispatch.require_green_preflight("snap-x")
    assert ok is False and "no preflight record" in msg
    (tmp_path / "snap-x.json").write_text('{"ok": false, "time": "t", "tail": "FAILED tsc RC=1"}')
    ok, msg = simple_dispatch.require_green_preflight("snap-x")
    assert ok is False and "baseline RED" in msg
    (tmp_path / "snap-x.json").write_text('{"ok": true, "time": "t", "commit": "abcdef1234"}')
    ok, msg = simple_dispatch.require_green_preflight("snap-x")
    assert ok is True


def test_truncated_empty_reply_is_not_model_done():
    # 35f62caa: content empty, finish_reason length, 4096 reasoning tokens -- recorded as model_done.
    assert agent_loop.is_truncated_empty_reply({"role": "assistant", "content": None}, "length")
    assert agent_loop.is_truncated_empty_reply({"role": "assistant", "content": "  "}, "length")
    assert not agent_loop.is_truncated_empty_reply({"role": "assistant", "content": "DONE: ok"}, "length")
    assert not agent_loop.is_truncated_empty_reply({"role": "assistant", "content": ""}, "stop")
    assert not agent_loop.is_truncated_empty_reply(
        {"role": "assistant", "content": "", "tool_calls": [{"id": "x"}]}, "length")
    assert "reasoning_exhausted" in dispatch_state.HARNESS_ENDINGS
    # A run that used its whole turn budget is a convergence problem, not a harness ending.
    assert "max_turns" not in dispatch_state.HARNESS_ENDINGS
    assert "dedup" not in dispatch_state.HARNESS_ENDINGS
