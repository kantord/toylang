"""Tests for land-patch dropping stale GENERATED hunks (2026-09-19).

Two GREEN patches (select-lazy-materialization-build-js and -lua) each
touched `site/public/corpus.json`. That file is generated and main rewrites
it on nearly every landing, so a plain `git am` refused with
`patch failed: site/public/corpus.json:14`, each failure burned a retry,
and the retry cap eventually left a land-failed marker -- for real edits
that were fine. A generated file must never be able to fail a landing.

The fix: `cmd_land_patch` applies the patch with one `--exclude=<path>` per
GENERATED entry (git am forwards these to git apply, dropping the hunks),
then regenerates the generated files in the lane worktree after a
successful am. This test exercises the factored-out
`apply_patch_excluding_generated` helper directly, without cargo.

Run: uv run --project .claude/scripts --group dev pytest .claude/scripts/tests"""
import importlib.util
import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


land_lane = load("land_lane")


def _git(cwd, *args):
    return subprocess.run(["git", "-C", str(cwd), *args],
                          capture_output=True, text=True)


def test_apply_patch_excluding_generated_drops_stale_generated_hunk(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    _git(repo, "init", "-q")
    _git(repo, "config", "user.email", "test@example.com")
    _git(repo, "config", "user.name", "test")

    corpus = repo / "site" / "public" / "corpus.json"
    corpus.parent.mkdir(parents=True)
    other = repo / "other.txt"
    corpus.write_text("A\n")
    other.write_text("base\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "base")

    # A branch that edits both a GENERATED file and a normal file.
    _git(repo, "checkout", "-q", "-b", "feature")
    corpus.write_text("B\n")
    other.write_text("patched\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "edit generated + normal")

    patch_file = tmp_path / "feature.patch"
    patch_file.write_text(_git(repo, "format-patch", "-1", "--stdout").stdout)

    # Advance main so the corpus.json hunk no longer applies (context line
    # changed from "A" to "A2"), while other.txt stays at base so its hunk
    # still applies cleanly.
    _git(repo, "checkout", "-q", "main")
    corpus.write_text("A2\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "advance main")
    pre_corpus = corpus.read_text()

    # Lane worktree: a fresh branch off the (advanced) main.
    _git(repo, "checkout", "-q", "-b", "lane")
    am_log = tmp_path / "am.log"
    with open(am_log, "w") as f:
        ok = land_lane.apply_patch_excluding_generated(repo, patch_file, f)

    # A plain `git am` (no --exclude) would have failed on the stale
    # corpus.json hunk; the excluded version succeeds and applies the rest.
    assert ok, am_log.read_text()
    assert other.read_text() == "patched\n"
    # The dropped corpus.json hunk leaves the file exactly as the (advanced)
    # base had it -- never the patch's stale "B", never a conflict marker.
    assert corpus.read_text() == pre_corpus == "A2\n"
