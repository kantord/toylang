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

for file in "${touched[@]}"; do
  [ -f "$file" ] || continue
  lines=$(wc -l < "$file" | tr -d ' ')

  case "$file" in
    *.rs) tests=$(inline_test_lines "$file") ;;
    *) tests=0 ;;
  esac
  : "${tests:=0}"
  body=$((lines - tests))

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
for file in "${touched[@]}"; do
  case "$file" in
    .claude/skills/*.md | research-log/*.md) ;;
    *) continue ;;
  esac
  [ -f "$file" ] || continue
  if [ "$(head -n 1 "$file")" != "---" ]; then
    findings+="okf-invalid	$file	no YAML frontmatter; OKF needs at least a type"$'\n'
  elif ! awk 'NR>1 && /^---$/ {exit} NR>1' "$file" | grep -qE '^type:[[:space:]]*\S'; then
    findings+="okf-invalid	$file	frontmatter has no type; OKF requires it"$'\n'
  fi
done

# Clippy compiles the whole workspace, so its findings are filtered down to
# the touched files rather than narrowed up front. Warnings only: a tree that
# does not compile is a different problem, reported elsewhere.
if command -v cargo > /dev/null 2>&1; then
  clippy=$(cargo clippy --workspace --all-targets --message-format=json -q 2>/dev/null \
    | jq -r 'select(.reason == "compiler-message")
             | .message
             | select(.level == "warning")
             | select(.code.code != null)
             | [(.code.code | sub("^clippy::"; "") | gsub("_"; "-")),
                (.spans[0].file_name // ""),
                .message]
             | @tsv' 2>/dev/null)

  while IFS=$'\t' read -r kind path detail; do
    [ -n "${path:-}" ] || continue
    for file in "${touched[@]}"; do
      if [ "$file" = "$path" ]; then
        findings+="$kind	$path	$detail"$'\n'
        break
      fi
    done
  done <<< "$clippy"
fi

# `--all-targets` compiles lib and test targets separately, so the same
# warning arrives once per target.
findings=$(printf '%s' "$findings" | grep -v '^$' | sort -u)
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
