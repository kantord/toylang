# Sundays on the first of the month, 1901-2000

Solves [Project Euler 19](https://projecteuler.net/problem=19). See the
[spoiler warning](00-spoiler-warning.md).

No date library exists, so this carries the calendar itself: `month_advance` steps one month
at a time, threading the running weekday of the 1st (0 for Sunday) forward by however many
days `days_in_month` says the current month has, and counting a hit whenever that weekday was
Sunday and the year is in range. `run_months` drives it through all 1212 months from January
1900, a Monday, to the end of 2000 in one walk; the walk is a self-tail-call, which every
backend runs in constant stack
([kantord/toylang#141](https://github.com/kantord/toylang/issues/141)).

```toylang
type State = { month: Int, year: Int, weekday: Int, count: Int }

fn is_leap(year: Int) -> Bool =
  year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)

fn days_in_month({ month, year }: { month: Int, year: Int }) -> Int =
  month == 2 and is_leap year
  | . -> 29 or
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1]!

fn month_advance({ month, year, weekday, count }: State) -> State =
  {
    month: month == 12 | . -> 1 or month + 1,
    year: month == 12 | . -> year + 1 or year,
    weekday:
      (weekday + days_in_month { month: month, year: year }) % 7,
    count: count + (year >= 1901 and weekday == 0 | . -> 1 or 0)
  }

fn run_months({ state, left }: { state: State, left: Int }) -> Int =
  left
  | . == 0 -> state.count or
    run_months { state: month_advance state, left: left - 1 }

run_months(
  {
    state: { month: 1, year: 1900, weekday: 1, count: 0 },
    left: 1212
  }
)
```

```output
171
```
