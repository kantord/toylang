"""Tests for the time- and process-dependent board-lint checks.

Run: uv run --project .claude/scripts python .claude/scripts/test_board_lint.py

The schema checks are verified by running the linter against the real
board; these two checks can't be, because their verdict depends on the
clock and on which processes exist, so both are injected here. The
incident these guard against: 12 rows sat `delegated` for two days with a
terminal STUCK log row each (2026-09-13), and one round file grew to seven
escalation questions about one harness bug.
"""
import importlib.util
import tempfile
import unittest
from datetime import UTC, datetime, timedelta
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "board_lint", Path(__file__).with_name("board-lint.py"))
board_lint = importlib.util.module_from_spec(spec)
spec.loader.exec_module(board_lint)

NOW = datetime(2026, 9, 14, 12, 0, tzinfo=UTC)
HEADER = "run_id,row_id,model,start_time,end_time,duration_s,status,cost_usd,patch_path\n"


def log_line(row_id, status, ended_ago, run_id="r1", extra=""):
    end = NOW - ended_ago
    start = end - timedelta(minutes=5)
    return (f"{run_id},{row_id},m,{start.isoformat()},{end.isoformat()},300.0,"
            f"{status},0.01,{extra}\n")


class StaleDelegated(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.log = Path(self.tmp.name) / "dispatch-log.csv"

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, rows, lines, live=()):
        self.log.write_text(HEADER + "".join(lines))
        return board_lint.lint_stale_delegated(
            rows, self.log, live_ids=lambda: set(live), now=NOW)

    def test_terminal_row_older_than_window_with_nothing_live_is_an_error(self):
        errs = self.check([{"id": "a", "status": "delegated"}],
                          [log_line("a", "STUCK", timedelta(hours=7))])
        self.assertEqual(len(errs), 1)
        self.assertIn("a: delegated but its last dispatch ended STUCK", errs[0])
        self.assertIn("reset to todo or archive", errs[0])

    def test_live_process_excuses_the_row(self):
        errs = self.check([{"id": "a", "status": "delegated"}],
                          [log_line("a", "STUCK", timedelta(hours=7))], live=["a"])
        self.assertEqual(errs, [])

    def test_recent_terminal_row_is_still_within_the_window(self):
        errs = self.check([{"id": "a", "status": "delegated"}],
                          [log_line("a", "STUCK", timedelta(hours=5))])
        self.assertEqual(errs, [])

    def test_latest_row_by_start_time_wins_not_file_order(self):
        # The stale run is appended AFTER the fresh one; a check that took
        # the last line in the file would wrongly flag this row.
        errs = self.check([{"id": "a", "status": "delegated"}],
                          [log_line("a", "STUCK", timedelta(hours=1), run_id="new"),
                           log_line("a", "STUCK", timedelta(days=2), run_id="old")])
        self.assertEqual(errs, [])

    def test_delegated_with_no_log_row_at_all_is_an_error(self):
        errs = self.check([{"id": "a", "status": "delegated"}], [])
        self.assertEqual(errs, [f"{board_lint.BOARD}: a: delegated with no dispatch recorded"])

    def test_extra_trailing_columns_are_tolerated(self):
        errs = self.check([{"id": "a", "status": "delegated"}],
                          [log_line("a", "GREEN", timedelta(hours=8), extra="p.patch,ended_by,3")])
        self.assertEqual(len(errs), 1)

    def test_non_delegated_rows_are_ignored(self):
        errs = self.check([{"id": "a", "status": "todo"}, {"id": "b", "status": "done"}],
                          [log_line("a", "STUCK", timedelta(days=3))])
        self.assertEqual(errs, [])

    def test_process_scan_is_skipped_when_nothing_looks_stale(self):
        def boom():
            raise AssertionError("live_ids() called with no stale candidate")
        self.log.write_text(HEADER + log_line("a", "STUCK", timedelta(hours=1)))
        errs = board_lint.lint_stale_delegated(
            [{"id": "a", "status": "delegated"}], self.log, live_ids=boom, now=NOW)
        self.assertEqual(errs, [])


class EscalationCap(unittest.TestCase):
    def round_file(self, flows):
        tmp = tempfile.NamedTemporaryFile("w", suffix=".round.yaml", delete=False)
        self.addCleanup(Path(tmp.name).unlink)
        tmp.write("intro: |\n  x\nquestions:\n")
        for i, flow in enumerate(flows):
            tmp.write(f"  - id: q{i}\n    flow: {flow}\n    question: |\n      why\n")
        tmp.close()
        return Path(tmp.name)

    def test_more_than_cap_escalations_is_an_error(self):
        errs = board_lint.lint_round_escalations(self.round_file(["escalation"] * 7))
        self.assertEqual(len(errs), 1)
        self.assertIn("7 escalation questions in one round", errs[0])
        self.assertIn("plans/dispatch-self-healing-plan.md", errs[0])

    def test_exactly_cap_is_allowed(self):
        errs = board_lint.lint_round_escalations(self.round_file(["escalation"] * 4))
        self.assertEqual(errs, [])

    def test_other_flows_do_not_count(self):
        errs = board_lint.lint_round_escalations(
            self.round_file(["escalation"] * 4 + ["question"] * 5))
        self.assertEqual(errs, [])


class MemoryPool(unittest.TestCase):
    """The pool's scope rule is the kind enum; these pin what it accepts.
    The seeded real file is covered by running the linter on the repo."""

    def pool(self, slots, cap=8):
        tmp = tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False)
        self.addCleanup(Path(tmp.name).unlink)
        tmp.write(f"version: 1\ncap: {cap}\nslots:{' []' if not slots else ''}\n")
        for s in slots:
            tmp.write("  - " + "\n    ".join(f"{k}: {v!r}" for k, v in s.items()) + "\n")
        tmp.close()
        return board_lint.lint_memory(tmp.name)

    WATCH = {"id": "w", "kind": "watch", "condition": "if x then y",
             "observation": "2026-09-14 seen once; see plans/x.md",
             "source": "plans/x.md", "written": "2026-09-14", "confirmed": 0}
    FACT = {"id": "f", "kind": "fact-check", "summary": "safe_signal is at drive_tick.py:147",
            "source": "drive_tick.py:147", "written": "2026-09-14", "confirmed": 2}

    def test_watch_and_fact_check_are_valid(self):
        self.assertEqual(self.pool([self.WATCH, self.FACT]), [])

    def test_watch_needs_condition_and_observation_not_summary(self):
        errs = self.pool([{**self.WATCH, "condition": "", "observation": ""}])
        self.assertEqual(len(errs), 2)
        self.assertTrue(all("condition" in e or "observation" in e for e in errs))
        self.assertEqual(self.pool([{**self.FACT, "summary": ""}])[0].count("summary"), 1)

    def test_incident_narrative_kind_is_rejected(self):
        # The design's scope rule: this is what keeps stall-diagnosis prose
        # in plans/simple-dispatch-design.md and out of the pool.
        errs = self.pool([{**self.FACT, "kind": "stall-diagnosis"}])
        self.assertEqual(len(errs), 1)
        self.assertIn("plans/simple-dispatch-design.md", errs[0])

    def test_over_cap_is_an_error(self):
        slots = [{**self.FACT, "id": f"f{i}"} for i in range(3)]
        errs = self.pool(slots, cap=2)
        self.assertEqual(len(errs), 1)
        self.assertIn("3 slots over cap 2", errs[0])

    def test_cap_above_lint_constant_is_rejected(self):
        errs = self.pool([], cap=board_lint.MEMORY_CAP + 1)
        self.assertEqual(len(errs), 1)
        self.assertIn("cap must be", errs[0])


if __name__ == "__main__":
    unittest.main()
