---
type: Lesson
calendar:
  - 2026-08-24
title: A config field that is ignored is a check that did not run
description: Giving corpus cases optional configuration created a new way to fail silently, since a misspelt key asks for nothing and a case that asks for nothing is indistinguishable from one that passes.
tags:
  - testing
  - agreement-harness
timestamp: 2026-08-24T00:00:00Z
---

The corpus was three files per case tied together by a shared stem: `adults.toy`, `adults.out`,
`adults.in.json`. Merging them into one YAML file was mostly a readability change, and it made
room for the thing that mattered: a case can now ask for checks beyond running on every backend
and agreeing.

The first such field is `snapshot`, naming the backends whose emitted code gets pinned as well.
It is for the claims the output cannot carry -- that Go declares a struct per record type, that
Python needs no declarations at all -- which until now lived as three copies of the same program
in three test files.

**Adding configuration added a failure mode the format did not have before.** `snapshots:` for
`snapshot:` asks for nothing. `snapshot: [javascript]` asks for nothing. Neither is an error in
any obvious sense; both leave a case that runs, passes, and quietly checks less than it says it
does. That is
[a test that cannot fail](a-test-that-cannot-fail-is-worse-than-no-test.md) arriving through the
fixture rather than through the assertion, and it is worse here because the file *looks* like it
is asking for something.

So the loader rejects unknown keys and unknown backend names, and both were checked by breaking
them and reading the message rather than by assuming serde was configured right. The general
form: **anywhere a test reads configuration, the parser has to be strict, because the failure
mode of a permissive one is silence.**

The migration itself followed the same rule. The YAML was generated, then checked field by field
against the files it replaced with an independent parser, then checked again with the one the
tests actually use, and the check was broken on purpose to confirm it could go red -- all before
the originals were deleted. Two cases whose expected output is a bare newline turned out not to
survive a block scalar, which clips a block of nothing but empty lines to the empty string. They
are quoted instead, with a comment saying why.
