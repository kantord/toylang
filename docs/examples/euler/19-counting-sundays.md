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
    0 if year % 4 != 0 else
    (1 if year % 400 == 0 else
    (0 if year % 100 == 0 else 1))

fn days_in_month(p: {month: Int, year: Int}) -> Int =
    31 if p.month == 1 else
    (28 + is_leap(p.year) if p.month == 2 else
    (31 if p.month == 3 else
    (30 if p.month == 4 else
    (31 if p.month == 5 else
    (30 if p.month == 6 else
    (31 if p.month == 7 else
    (31 if p.month == 8 else
    (30 if p.month == 9 else
    (31 if p.month == 10 else
    (30 if p.month == 11 else 31))))))))))

fn month_advance(s: {month: Int, year: Int, weekday: Int, count: Int}) -> {month: Int, year: Int, weekday: Int, count: Int} =
    {
        month: 1 if s.month == 12 else s.month + 1,
        year: (s.year + 1) if s.month == 12 else s.year,
        weekday: (s.weekday + days_in_month({month: s.month, year: s.year})) % 7,
        count: s.count + ((1 if s.weekday == 0 else 0) if s.year >= 1901 else 0)
    }

fn run_months(p: {state: {month: Int, year: Int, weekday: Int, count: Int}, left: Int}) -> {month: Int, year: Int, weekday: Int, count: Int} =
    p.state if p.left == 0 else run_months({state: month_advance(p.state), left: p.left - 1})

fn run_years(p: {state: {month: Int, year: Int, weekday: Int, count: Int}, years_left: Int}) -> Int =
    p.state.count if p.years_left == 0 else
    run_years({state: run_months({state: p.state, left: 12}), years_left: p.years_left - 1})

run_years({state: {month: 1, year: 1900, weekday: 1, count: 0}, years_left: 101})
```

```output
171
```
