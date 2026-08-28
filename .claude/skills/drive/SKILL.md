---
name: drive
description: Drive development autonomously from plans/board.yaml - the ordered task board with dependencies. Use when the user says "drive", "keep going", "work the board", asks what's next, or wants autonomous development to continue while they only do grilling and goal-setting.
---

# Drive the board

`plans/board.yaml` is the single source of truth: an ordered list where position is priority.
Each entry: `id`, `title`, `kind: build | decide`, `needs: [ids]`, `status: todo | delegated |
done`, optionally `issue: gh:N`. The maintainer's role is decide-tasks and goal-setting;
everything else is yours to drive. Never invent tasks: new work enters the board through a
grilling/planning session or an explicit user request, and gets a row before it gets a branch.

## The loop

1. **Read the board.** Unblocked = `status: todo` and every id in `needs` is `done`, taken in
   list order. Derive, never store, blockedness.
2. **Deadlock check, before anything else.** Two shapes, both reported to the user
   immediately rather than worked around: a cycle in `needs` (topological sort fails), and
   exhaustion (todo entries remain but nothing is unblocked, and nothing is delegated or
   awaiting review). A third, operational one: a delegated session with no commits and no
   transcript activity for ~30 minutes -- go read its state (worktree diff, last transcript
   entry) and either finish its work by hand, relaunch it, or escalate; do not just wait.
3. **Dispatch the top unblocked task of each kind.**
   - `decide`: queue it for the user. These are the only things worth interrupting them for
     when no build work is ready; otherwise present them at the next natural report.
   - `build`: make sure a GitHub issue carries the spec (file one if the row has none), then
     delegate via the `enwiro-delegate` skill and set `status: delegated`. The session runs
     on sonnet unless the row says `model: fable` (design-heavy or cross-cutting work only). More than one
     build task may run concurrently ONLY if their likely file footprints do not overlap --
     judge from the plans and the diff history; when in doubt, serialize. Priority order
     breaks ties, not the other way around.
4. **Monitor and land.** Watch delegated work (a cron tick per active delegation is enough);
   when a session finishes, run the `land-delegated-work` skill: suite, code-review,
   style-review, fix-or-file, merge locally. Then set the row `done`, commit the board change
   with the merge, and go to step 1.
5. **Report once per landing or decision-point,** per the standing protocol: what landed,
   what the reviews found, what is now unblocked, and which decide-tasks are waiting.
   No play-by-play.

## Board hygiene

- Review follow-ups become new rows (usually `build`, sometimes a `decide` + `build` pair
  when a finding needs a design call first), placed by priority judgment, linked to their
  filed issue.
- Reordering rows IS reprioritizing; do it when the user says so, or propose it in a report
  when the order has stopped matching reality.
- The board is committed like any other file (AGENTS.md rules apply). Keep rows terse; the
  linked issue and plans/*.md carry the detail.
