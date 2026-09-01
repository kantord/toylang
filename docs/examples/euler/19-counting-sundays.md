# Sundays on the first of the month, 1901-2000

Solves [Project Euler 19](https://projecteuler.net/problem=19). See the
[spoiler warning](00-spoiler-warning.md).

No date library exists, so this carries the calendar itself: `month_advance` steps one month
at a time, threading the running weekday of the 1st (0 for Sunday) forward by however many
days `days_in_month` says the current month has, and tallies a hit whenever that weekday was
Sunday and the year is in range. Walking all 1212 months from January 1900 in one flat
recursion would again risk a backend's call-stack limit (see [counting
letters](17-number-letter-counts.md)), so `run_years` calls `run_months` once per year instead
of recursing over months directly, which keeps every stack shallow.

```toylang
fn is_leap(year: Int) -> Int =
    year | . % 4 != 0 -> 0 or . % 400 == 0 -> 1 or . % 100 == 0 -> 0 or 1

fn days_in_month(p: {month: Int, year: Int}) -> Int =
    p
        | .month == 1 -> 31 or
              .month == 2 -> 28 + is_leap(.year) or
              .month == 3 -> 31 or
              .month == 4 -> 30 or
              .month == 5 -> 31 or
              .month == 6 -> 30 or
              .month == 7 -> 31 or
              .month == 8 -> 31 or
              .month == 9 -> 30 or
              .month == 10 -> 31 or
              .month == 11 -> 30 or
              31

fn month_advance(s: {month: Int, year: Int, weekday: Int, count: Int}) -> {month: Int, year: Int, weekday: Int, count: Int} =
    {
        month: s | .month == 12 -> 1 or s.month + 1,
        year: s | .month == 12 -> s.year + 1 or s.year,
        weekday: (s.weekday + days_in_month({month: s.month, year: s.year})) % 7,
        count: s.count + (s | .year >= 1901 -> (s | .weekday == 0 -> 1 or 0) or 0)
    }

fn run_months(p: {state: {month: Int, year: Int, weekday: Int, count: Int}, left: Int}) -> {month: Int, year: Int, weekday: Int, count: Int} =
    p
        | .left == 0 -> .state or
              run_months({state: month_advance(.state), left: .left - 1})

fn run_years(p: {state: {month: Int, year: Int, weekday: Int, count: Int}, years_left: Int}) -> Int =
    p
        | .years_left == 0 -> .state.count or
              run_years({state: run_months({state: .state, left: 12}), years_left: .years_left - 1})

run_years(
    {state: {month: 1, year: 1900, weekday: 1, count: 0}, years_left: 101}
)
```

```output
171
```
