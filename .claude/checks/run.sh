#!/usr/bin/env bash
#
# Reports code-style findings for the files this session touched.
#
# Run from a Stop or SubagentStop hook: an agent that is about to finish is
# told what it left behind, and pointed at the lesson for each finding. See
# .claude/skills/code-style/SKILL.md.
#
# Adapted from kantord/toy-browser's .claude/checks/run.sh. The structural
# change is what "touched" means: there it is whatever sits uncommitted at
# stop time, which matches a flow where work waits for a human to commit.
# Here AGENTS.md has sessions committing per step, so a session's tree is
# clean at Stop and that definition would report nothing, every time. Touched
# here is everything changed since `merge-base main HEAD`, plus anything
# uncommitted, and "before" for the caused/inherited split is the merge-base
# blob, not HEAD.
#
# Exit 0 = nothing to say. Exit 2 = findings, reported on stderr, which the
# harness feeds back to the agent.

set -uo pipefail

root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0

# The harness sets this when it is re-firing after a block. Stopping twice for
# the same thing would spin forever on a finding the agent cannot fix.
if [ -t 0 ]; then
  input=""
else
  input=$(cat)
fi
if [ "$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null)" = "true" ]; then
  exit 0
fi

limit=$(grep -oE '^max_file_lines[[:space:]]*=[[:space:]]*[0-9]+' .claude/checks/limits.toml 2>/dev/null | grep -oE '[0-9]+')
: "${limit:=1000}"

# Everything the repo owns and a person wrote: code, prose, scripts. Prose is
# held to the same budget as code, which is what stops a document growing into
# a wall nobody reads. Corpus YAML stays out: case shape is already enforced
# harder than a hook could (unknown keys are errors, and tag_corpus.rs
# rewrites node_types on every run).
readonly OWNED=('*.rs' '*.md' '*.sh')

# AGENTS.md has sessions commit their own work; ending a turn with a diff
# still sitting in the tree is the politeness stall (issue #18), not a
# decision anyone needs to make. Same reasoning as the rustfmt hook: one
# right answer, so the finding is the fix itself, no lesson to look up.
uncommitted=$(git status --porcelain -- "${OWNED[@]}" 2>/dev/null)
if [ -n "$uncommitted" ]; then
  {
    echo "Uncommitted work at Stop:"
    echo
    echo "$uncommitted"
    echo
    echo "Commit it before ending the turn -- per AGENTS.md, agents commit their own work here. This is not a code-style finding: there is no lesson to read, committing is the fix."
  } >&2
  exit 2
fi

# Files this session touched: committed on this branch since it left main,
# plus anything uncommitted. Renames in status report as "old -> new", so the
# last field is the path that exists now.
base=$(git merge-base main HEAD 2>/dev/null) || base=""
mapfile -t touched < <(
  {
    [ -n "$base" ] && git diff --name-only "$base" -- "${OWNED[@]}" 2>/dev/null
    git status --porcelain -- "${OWNED[@]}" 2>/dev/null | awk '{print $NF}'
  } | sort -u
)

# draft.md is excluded by name. Not because it is fine -- at ~2900 lines it is
# exactly the wall the budget exists to prevent -- but because its shape is an
# open decision (the board's draft-split-decision row), not something a
# line-count hook should force on whichever session touches the draft next.
kept=()
for file in "${touched[@]}"; do
  [ "$file" = "draft.md" ] && continue
  kept+=("$file")
done
touched=("${kept[@]}")
[ "${#touched[@]}" -eq 0 ] && exit 0

# finding lines are "kind<TAB>path<TAB>detail"
findings=""

# Inline tests are measured apart from everything else. A file that inlines
# its tests is not simpler than one that does not, so moving `#[cfg(test)]`
# out to tests/ must not read as a split. Body and tests each get the full
# budget, which caps the whole file at twice it without needing a third rule.
# (Dormant today: no src/ file inlines tests. Harmless to keep.)
inline_test_lines() {
  awk '
    /^[[:space:]]*#\[cfg\(test\)\]/ && !intest { intest = 1; depth = 0; opened = 0 }
    intest {
      count++
      opens = gsub(/\{/, "{"); closes = gsub(/\}/, "}")
      depth += opens - closes
      if (opens > 0) opened = 1
      if (opened && depth <= 0) intest = 0
    }
    END { print count + 0 }
  ' "$1"
}

# Body/test line split for one file, per inline_test_lines above. Shared by the touched-file
# loop and file_too_long_hit's whole-tree re-check below, so the budget math lives in one place.
line_counts() {
  local path=$1 lines tests
  lines=$(wc -l < "$path" | tr -d ' ')
  case "$path" in
    *.rs) tests=$(inline_test_lines "$path") ;;
    *) tests=0 ;;
  esac
  : "${tests:=0}"
  printf '%s\t%s\n' "$((lines - tests))" "$tests"
}

for file in "${touched[@]}"; do
  [ -f "$file" ] || continue
  IFS=$'\t' read -r body tests < <(line_counts "$file")

  # Was it already over before this branch? Debt you inherited is reported
  # differently from debt you just created -- see the lesson.
  before=$(git show "${base:-HEAD}:$file" 2>/dev/null | wc -l | tr -d ' ')
  if [ -z "$before" ] || [ "$before" = "0" ]; then
    origin="new file"
  elif [ "$before" -gt "$limit" ]; then
    origin="inherited, already $before at merge-base"
  else
    origin="caused, was $before at merge-base"
  fi

  if [ "$body" -gt "$limit" ]; then
    findings+="file-too-long	$file	$body lines excluding inline tests, budget is $limit ($origin)"$'\n'
  fi
  if [ "$tests" -gt "$limit" ]; then
    findings+="file-too-long	$file	$tests lines of inline tests, budget is $limit ($origin)"$'\n'
  fi
done

# Lessons and research-log notes are Open Knowledge Format bundles, and the
# only thing that makes a document conformant is YAML frontmatter carrying a
# `type`. Checking it here is what keeps "valid OKF" automatic rather than
# remembered.
#
# Scoped to the lints bundle itself, not every skill: a case pattern's `*`
# matches `/`, so `.claude/skills/*.md` used to also catch every
# SKILL.md (#42) -- those carry Claude Code's own name/description
# frontmatter, a different schema, and were never meant to have a `type`.
for file in "${touched[@]}"; do
  case "$file" in
    .claude/skills/code-style/lints/*.md | research-log/*.md) ;;
    *) continue ;;
  esac
  [ -f "$file" ] || continue
  if [ "$(head -n 1 "$file")" != "---" ]; then
    findings+="okf-invalid	$file	no YAML frontmatter; OKF needs at least a type"$'\n'
  elif ! awk 'NR>1 && /^---$/ {exit} NR>1' "$file" | grep -qE '^type:[[:space:]]*\S'; then
    findings+="okf-invalid	$file	frontmatter has no type; OKF requires it"$'\n'
  fi
done

# Clippy compiles the whole workspace, so its findings are gathered over all of it, not just
# touched files, keeping `clippy_workspace` (kind, path, function, message) around for the
# sinkhole's freeloader check below. Reporting still filters down to touched files. Warnings
# only: a tree that does not compile is a different problem, reported elsewhere. The `function`
# field is a best-effort read of the source line clippy points at (`fn NAME`, if that line has
# one) -- empty when the span does not land on a signature line, which just means no
# function-scoped sinkhole entry can match that finding.
cargo_ran=0
clippy_workspace=""
if command -v cargo > /dev/null 2>&1; then
  cargo_ran=1
  clippy_raw=$(cargo clippy --workspace --all-targets --message-format=json -q 2>/dev/null \
    | jq -r 'select(.reason == "compiler-message")
             | .message
             | select(.level == "warning")
             | select(.code.code != null)
             | [(.code.code | sub("^clippy::"; "") | gsub("_"; "-")),
                (.spans[0].file_name // ""),
                (.spans[0].line_start // 0),
                .message]
             | @tsv' 2>/dev/null)

  while IFS=$'\t' read -r kind path lineno detail; do
    [ -n "${path:-}" ] || continue
    func=""
    if [ -f "$path" ] && [ "${lineno:-0}" -gt 0 ] 2>/dev/null; then
      func=$(sed -n "${lineno}p" "$path" | grep -oE '\bfn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*' | awk '{print $2}' | head -n1)
    fi
    detail_out="$detail"
    [ -n "$func" ] && detail_out="fn $func: $detail"
    clippy_workspace+="$kind	$path	$func	$detail_out"$'\n'
  done <<< "$clippy_raw"
  clippy_workspace=$(printf '%s' "$clippy_workspace" | grep -v '^$')

  while IFS=$'\t' read -r kind path func detail; do
    [ -n "${path:-}" ] || continue
    for file in "${touched[@]}"; do
      if [ "$file" = "$path" ]; then
        findings+="$kind	$path	$detail"$'\n'
        break
      fi
    done
  done <<< "$clippy_workspace"
fi

# `#[allow]`/`#![allow]` beside the code it excuses is itself a finding -- the sinkhole
# (.claude/checks/sinkhole.toml) is the only sanctioned home for a justified exemption. Anchored
# at line start (after whitespace) so this does not fire on emit_rs.rs's two string literals,
# which emit these tokens into *generated* Rust as `format!` arguments and never start a line
# with them themselves. Also matches an allow riding in on cfg_attr (`#[cfg_attr(..., allow(...))]`),
# which is a bare allow wearing a condition, not an exemption.
bare_allow_pattern='^[[:space:]]*#!?\[(allow\(|cfg_attr\(.*[,( ]allow\()'
for file in "${touched[@]}"; do
  case "$file" in
    *.rs) ;;
    *) continue ;;
  esac
  [ -f "$file" ] || continue
  while IFS=: read -r lineno rest; do
    [ -n "${lineno:-}" ] || continue
    findings+="bare-allow	$file	line $lineno: $(printf '%s' "$rest" | sed -E 's/^[[:space:]]+//')"$'\n'
  done < <(grep -nE "$bare_allow_pattern" "$file" 2>/dev/null)
done

# `--all-targets` compiles lib and test targets separately, so the same
# warning arrives once per target.
findings=$(printf '%s' "$findings" | grep -v '^$' | sort -u)

# The sinkhole: structured, justified exemptions (.claude/checks/sinkhole.toml). Consulting it
# means two things -- an entry suppresses the finding it excuses from blocking the agent, and
# every entry is re-checked here, against the whole tree rather than just files this session
# touched, so an exemption nobody needs any more (the function shrank, the file split) is itself
# a finding instead of quietly outliving its reason.
sinkhole_path=.claude/checks/sinkhole.toml
if [ -f "$sinkhole_path" ] && command -v python3 > /dev/null 2>&1; then
  entries=$(python3 - "$sinkhole_path" <<'PY' 2>/dev/null
import sys, tomllib
with open(sys.argv[1], "rb") as f:
    data = tomllib.load(f)
for e in data.get("exemption", []):
    print("\t".join([e.get("kind", ""), e.get("path", ""), e.get("function", "")]))
PY
  )

  # Whole-tree re-check, one function per entry kind. file-too-long re-derives via the same
  # line_counts helper the touched-file loop uses. bare-allow re-runs the same pattern the
  # finding loop above matched with, straight against the path -- no touched-file restriction.
  # Everything else (too-many-lines and any bare clippy lint name) is matched against
  # clippy_workspace by kind and path, scoped to a function when the entry names one; a
  # too-many-lines entry is matched by function name, read off the source line clippy points at
  # (the span lands exactly on the `fn` line for this lint, not a wrapped signature line).
  file_too_long_hit() {
    local path=$1 body tests
    [ -f "$path" ] || return 1
    IFS=$'\t' read -r body tests < <(line_counts "$path")
    [ "$body" -gt "$limit" ] || [ "$tests" -gt "$limit" ]
  }
  bare_allow_hit() {
    local path=$1
    [ -f "$path" ] || return 1
    grep -qE "$bare_allow_pattern" "$path" 2>/dev/null
  }
  clippy_lint_hit() {
    local kind=$1 path=$2 func=$3
    [ "$cargo_ran" -eq 1 ] || return 0 # cargo unavailable: cannot re-check, do not evict on faith
    printf '%s\n' "$clippy_workspace" | awk -F'\t' -v k="$kind" -v p="$path" -v fn="$func" \
      '$1 == k && $2 == p && (fn == "" || $3 == fn) { found=1 } END { exit !found }'
  }

  remaining=""
  if [ -n "$findings" ]; then
    while IFS=$'\t' read -r kind path detail; do
      matched=0
      while IFS=$'\t' read -r ekind epath efunc; do
        [ -n "${ekind:-}" ] || continue
        [ "$kind" = "$ekind" ] && [ "$path" = "$epath" ] || continue
        if [ -n "$efunc" ]; then
          case "$detail" in
            "fn $efunc: "*) ;;
            *) continue ;;
          esac
        fi
        matched=1
        break
      done <<< "$entries"
      [ "$matched" -eq 0 ] && remaining+="$kind	$path	$detail"$'\n'
    done <<< "$findings"
  fi
  findings=$(printf '%s' "$remaining" | grep -v '^$' | sort -u)

  while IFS=$'\t' read -r ekind epath efunc; do
    [ -n "${ekind:-}" ] || continue
    live=0
    case "$ekind" in
      file-too-long) file_too_long_hit "$epath" && live=1 ;;
      bare-allow) bare_allow_hit "$epath" && live=1 ;;
      *) clippy_lint_hit "$ekind" "$epath" "$efunc" && live=1 ;;
    esac
    if [ "$live" -eq 0 ]; then
      target="$epath"
      [ -n "$efunc" ] && target="$epath ($efunc)"
      findings+=$'\n'"stale-sinkhole-entry	$sinkhole_path	entry for $ekind on $target no longer fires -- the exemption has nothing left to excuse, remove it"
    fi
  done <<< "$entries"
  findings=$(printf '%s' "$findings" | grep -v '^$' | sort -u)
fi

[ -z "$findings" ] && exit 0

count=$(printf '%s\n' "$findings" | wc -l | tr -d ' ')
{
  echo "Code style: $count finding(s) in files this session touched."
  echo
  while IFS=$'\t' read -r kind path detail; do
    lesson=".claude/skills/code-style/lints/$kind.md"
    echo "  $kind  $path"
    echo "    $detail"
    if [ -f "$lesson" ]; then
      echo "    lesson: $lesson"
    else
      echo "    lesson: $lesson  (MISSING -- nobody has decided how to handle this)"
    fi
    echo
  done <<< "$findings"
  echo "Read .claude/skills/code-style/SKILL.md before acting on any of these."
} >&2

exit 2
