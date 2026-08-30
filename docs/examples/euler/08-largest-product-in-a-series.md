# The best window of digits (skipped)

Skipped, and this is where the reason was first argued. [Project Euler
8](https://projecteuler.net/problem=8) wants the largest product of thirteen adjacent digits in
one specific thousand-digit number, and that number is problem-given data with no source but
Project Euler itself. [kantord/toylang#39](https://github.com/kantord/toylang/issues/39) settled
that it never lives in this repo.

What was built on top of that settlement is the part worth recording.
[kantord/toylang#69](https://github.com/kantord/toylang/issues/69) had the digits arrive on stdin
from a gitignored fixture, with the docs harness skipping the fragment when the file was absent
-- which it is for everyone. The page then printed the commonly published answer in an `output`
fence, the same fence every other page here uses for a claim the harness runs on seven backends.
Nobody in this repo ever ran it. A gate that skips silently reads exactly like a check that
passed, so the fixture fence is gone and these four pages are skips instead; the program and the
blocker live in [kantord/toylang#129](https://github.com/kantord/toylang/issues/129).

The other half of this page's old blocker really did clear:
[Int64](../../reference/types/int64.md) holds `9^13`, about 2.5e12
([kantord/toylang#83](https://github.com/kantord/toylang/issues/83)). Only the data question is
left. See the [spoiler warning](00-spoiler-warning.md).
