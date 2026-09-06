# Escalation: `just check` cannot go green as root

## Question

The `search-layer-build` brief requires `just check` green. Two pre-existing tests,
`tests/fmt_project.rs::unreadable_directory_is_reported_and_walk_continues` and
`::unreadable_file_is_reported_and_walk_continues`, cannot pass when the suite runs as root:
they chmod a directory or file to `0o000` and assert that the `toylang fmt` walker reports a
permission error. Root bypasses permission bits, so `read_dir`/`read` succeed and the refusal
never happens.

## Evidence it is environmental, not caused by this branch

- The failing assertion is exactly `read_dir` on a `0o000` directory. Verified in this sandbox:
  as root, `ls` reads the directory; as `nobody` (via `setpriv`), it gets `Permission denied`.
- Running the already-built `fmt_project` test binary as `nobody` passes both tests unchanged.
- CI (`.github/workflows/ci.yml`) runs on `ubuntu-latest`, a non-root user, where both pass.

None of this branch's changes (the `first`/`any`/`all` builtins) touch file permissions or the
formatter.

## Alternatives

1. **Skip the two tests when running as root.** Minimal, transparent, keeps the assertions
   fully active for the non-root case that CI and normal development run. The chosen path.
2. **Run the whole suite as a non-root user in this sandbox.** Requires giving a non-root user
   write access to `/repo` (the suite rewrites `tests/corpus/*.yaml` and
   `site/public/corpus.json` in place) and a copy of `CARGO_HOME`; invasive and fragile.
3. **Leave `just check` red and document only.** Fails the brief's explicit definition of done.

## Decision

Took alternative 1: a `running_as_root()` guard (`id -u` reports 0) at the top of the two
tests, with a comment explaining why the assertion is unobservable as root. The guard does not
silently cover a broken walker: for any non-root user the tests run exactly as before. Revert
this if the maintainer prefers a different accommodation.
