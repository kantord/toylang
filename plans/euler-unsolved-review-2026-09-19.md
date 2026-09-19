# Euler review: what is not solved, and what is solved smaller, 2026-09-19

Every page under docs/examples/euler/ was run and its output compared with the accepted
Project Euler answer. The question was which pages solve the real problem, which solve a
reduced instance, and what blocks the rest. Timings are measured today on this machine,
not copied from the pages.

## The count

| class | pages |
|---|---|
| Full, real bound, accepted answer, every-fragment suite | 1, 2, 3, 4, 5, 6, 7, 9, 12, 15, 17, 19, 21, 24, 26, 28, 29, 30 |
| Full, under the `slow` tier | 10, 14 |
| Reduced in the docs, real data opt-in | 8, 11, 13, 18 |
| Skipped | 16, 20, 22, 23, 25, 27 |

Eighteen full, two slow, four opt-in, six skipped, before today's changes.

## Skipped pages whose reason no longer holds

**16, 20, 25 say they need bigints. They do not.** Each problem needs the digits of a large
number, never the number as a value, and page 13 already carries a bignum as a `Vec<Int>`
of digits. Programs written that way today print the accepted answers on all seven
backends: 1366 for 2^1000 (jq 2.3s), 648 for 100! (jq 0.15s), and 4782 for the first
1000-digit Fibonacci term using base-10^8 limbs (jq 3.7s). All three are cheaper than pages
4, 7, 12, 21 and 30, which already run in the every-fragment suite. Bigints as a type (#112)
would only add the ability to print such a number as a number; that stays unscheduled.

**22 says it needs a Char-to-Int conversion and a sorted Vec.** `sort` orders `Vec<Str>` by
codepoint, and a letter's value is the length of the alphabet prefix up to it, computed with
`chars` and `select`, so the whole program works today: 612 on a four-name sample, 0.4s on
5163 names. What it cannot do is read the 46KB names file: toylang reads stdin only, and the
data cannot enter the repo (#39). That is exactly what the opt-in real-data protocol of pages
8, 11, 13 and 18 exists for (`tests/euler_real_data.rs`, `just euler-data DIR`), and it only
needs a fifth case with the Rust side splitting the comma-separated quoted names.

All four are being written now, as full fences for 16, 20 and 25 and as an opt-in page for
22. Page 24 is folded to print `2783915460` as an `Int64` instead of its digit vector.

## Skipped pages that are correct but slow

**23** prints 4179871 at the full bound. A `Vec<Bool>` abundance table for O(1) membership
and an early-exit recursion instead of `any` over a materialized list bring it to Go 1.0s
and Lua 14s, but jq still takes 51s. The specific missing pieces are a set or dictionary
type and mutation, so the sum-of-two-abundants test is a linear scan per candidate. It is
an honest `slow` fence today and cannot be an every-fragment one.

**27** prints -59231. With b restricted to the 168 primes below 1000, a to odd values, and
the `{a, b, len}` winner packed into one `Int` key so plain `max` reduces it (`max_by` is
Go and Rust only), it measures Go 0.2s, Lua 3.5s, jq 22s, against the 39s the page records.
That is cheaper than page 30, which runs on every `just test` at 30s on jq. Either 27 joins
the every-fragment suite as written, or 30 belongs under `slow`; the tier rule is measured
cost, and the two pages disagree with it in opposite directions.

Both stay untouched here: the ruling on #93 (2026-09-01) hands them to a privileged manual
session, and `euler-slow-fragments-2` carries that. The measurements above belong in that
session's prompt.

## Pages solved at a reduced size

**8, 11, 13, 18** run on tiny synthetic inputs in the docs and on the real data through the
opt-in test, which asserts the accepted answers 23514624000, 70600674, 5537376230 and 1074
on every backend and panics rather than skips when the data is absent. The programs in the
test are the pages' programs verbatim as of today's sync. The reduction is policy, not
language: the data cannot be committed, and toylang has no file input but stdin. Two
caveats. Problem 11 has no recorded real-data run since the Python recursion limit was
raised, so its pass is unconfirmed. Problem 13's digit-vector answer fits in `Int64` and
could print as a number like page 24 now does.

## Full pages that know the answer

**7 and 12** search a `range` whose bound was chosen because the answer is known (104744
and 12376). The pages say so. The missing capability is a take-while, or "stop at the nth
hit", on a stream; `first` is Vec-only, so the search cannot stop itself.

## What is still genuinely load-bearing

- File input other than stdin, and string splitting: 8, 11, 13, 18, 22 all parse on the
  Rust side of a test because toylang cannot.
- A set or dictionary type: 23.
- `sort_by` and `max_by` on the other five backends: 26 and 27 hand-roll a packed-key or
  divide-and-conquer maximum. The rows exist under #177.
- A stream take-while: 7 and 12.
- jq's per-iteration cost: 4, 7, 12, 21, 23, 27 and 30 are all seconds to a minute there and
  instant elsewhere. Nothing in the language fixes that; the tier does.
