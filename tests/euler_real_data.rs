//! The opt-in check that verifies Euler 8, 11, 13 and 18 against real puzzle data.
//!
//! Each problem reads a blob of problem-given data that cannot live in this repo (#39), so the
//! docs pages run the programs below on small synthetic data instead and point here for the
//! real-sized check. This is where the published answers are actually confirmed: the same
//! programs, run against a contributor's own copies of the real texts, failing loudly on a wrong
//! answer rather than skipping.
//!
//! `#[ignore]` keeps it out of `just test`; `just euler-data DIR` runs it with DIR holding your
//! own copies of the four raw data texts, copied from projecteuler.net:
//!
//! - `euler08.txt`: the thousand-digit number, whitespace allowed.
//! - `euler11.txt`: twenty lines, twenty integers each.
//! - `euler13.txt`: a hundred lines, fifty digits each.
//! - `euler18.txt`: fifteen lines, line `i` holding `i + 1` integers.

mod support;

use std::env;
use std::path::Path;

use serde_json::{Value, json};

const PROGRAM_8: &str = r#"
fn window(p: {v: Vec<Int>, i: Int, k: Int}) -> Int64 =
    p | .k == 0 -> 1 or i64(p.v[p.i + p.k - 1]!) * window({v: p.v, i: p.i, k: p.k - 1})

fn max2(p: {a: Int64, b: Int64}) -> Int64 = p | .a > .b -> p.a or p.b

fn best(p: {v: Vec<Int>, lo: Int, hi: Int}) -> Int64 =
    p | .hi - .lo == 1 -> window({v: p.v, i: p.lo, k: 13}) or
        max2(
            {
                a: best({v: p.v, lo: p.lo, hi: (p.lo + p.hi) / 2}),
                b: best({v: p.v, lo: (p.lo + p.hi) / 2, hi: p.hi})
            }
        )

best({v: input, lo: 0, hi: length(input) - 12})
"#;

const PROGRAM_11: &str = r#"
fn get(p: {g: Vec<Vec<Int>>, r: Int, c: Int}) -> Int = p.g[p.r]![p.c]!

fn four(p: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int =
    get({g: p.g, r: p.r, c: p.c}) * get({g: p.g, r: p.r + p.dr, c: p.c + p.dc}) *
        get({g: p.g, r: p.r + 2 * p.dr, c: p.c + 2 * p.dc}) *
        get({g: p.g, r: p.r + 3 * p.dr, c: p.c + 3 * p.dc})

fn row_products(p: {g: Vec<Vec<Int>>, r: Int, dr: Int, dc: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    collect(range(p.cmax))
        | select(. >= p.cmin)
        | map(four({g: p.g, r: p.r, c: ., dr: p.dr, dc: p.dc}))

fn direction(p: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    flatten(
        collect(range(p.rmax))
            | map(
                  row_products(
                      {
                          g: p.g,
                          r: .,
                          dr: p.dr,
                          dc: p.dc,
                          cmin: p.cmin,
                          cmax: p.cmax
                      }
                  )
              )
    )

fn maximum_of(p: {v: Vec<Int>, i: Int, best: Int}) -> Int =
    p | .i >= length(.v) -> p.best or
        maximum_of(
            {
                v: p.v,
                i: p.i + 1,
                best: p | .v[.i]! > .best -> p.v[p.i]! or p.best
            }
        )

fn maximum(v: Vec<Int>) -> Int = maximum_of({v: v, i: 1, best: v[0]!})

fn largest_product(g: Vec<Vec<Int>>) -> Int =
    maximum(
        direction(
            {
                g: g,
                dr: 0,
                dc: 1,
                rmax: length(g),
                cmin: 0,
                cmax: length(g[0]!) - 3
            }
        ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: 0,
                    rmax: length(g) - 3,
                    cmin: 0,
                    cmax: length(g[0]!)
                }
            ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: 1,
                    rmax: length(g) - 3,
                    cmin: 0,
                    cmax: length(g[0]!) - 3
                }
            ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: -1,
                    rmax: length(g) - 3,
                    cmin: 3,
                    cmax: length(g[0]!)
                }
            )
    )

largest_product(input)
"#;

const PROGRAM_13: &str = r#"
fn empty() -> Vec<Int> = []

fn col_sum(p: {nums: Vec<Vec<Int>>, i: Int, k: Int}) -> Int =
    p | .i >= length(.nums) -> 0 or
        p.nums[p.i]![p.k]! + col_sum({nums: p.nums, i: p.i + 1, k: p.k})

fn column_total(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int}) -> Int =
    col_sum({nums: p.nums, i: 0, k: p.k}) + p.carry

fn emit_carry(p: {carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    p | .carry == 0 -> p.acc or
        emit_carry({carry: p.carry / 10, acc: [p.carry % 10] + p.acc})

fn add_digits(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    p | .k < 0 -> emit_carry({carry: p.carry, acc: p.acc}) or
        add_digits(
            {
                nums: p.nums,
                k: p.k - 1,
                carry: column_total({nums: p.nums, k: p.k, carry: p.carry}) / 10,
                acc: [column_total({nums: p.nums, k: p.k, carry: p.carry}) % 10] +
                    p.acc
            }
        )

fn first_ten(v: Vec<Int>) -> Vec<Int> = collect(range(10)) | map(v[.]!)

fn leading_digits(nums: Vec<Vec<Int>>) -> Vec<Int> =
    first_ten(
        add_digits(
            {nums: nums, k: length(nums[0]!) - 1, carry: 0, acc: empty()}
        )
    )

leading_digits(input)
"#;

const PROGRAM_18: &str = r#"
fn combine(p: {row: Vec<Int>, below: Vec<Int>, i: Int}) -> Int =
    p.row[p.i]! +
        (
            p | .below[.i]! > .below[.i + 1]! -> p.below[p.i]! or p.below[p.i + 1]!
        )

fn merge_row(p: {row: Vec<Int>, below: Vec<Int>}) -> Vec<Int> =
    collect(range(length(p.row))) | map(combine({row: p.row, below: p.below, i: .}))

fn collapse(p: {rows: Vec<Vec<Int>>, i: Int, acc: Vec<Int>}) -> Int =
    p | .i < 0 -> p.acc[0]! or
        collapse(
            {
                rows: p.rows,
                i: p.i - 1,
                acc: merge_row({row: p.rows[p.i]!, below: p.acc})
            }
        )

fn triangle_max(rows: Vec<Vec<Int>>) -> Int =
    collapse({rows: rows, i: length(rows) - 2, acc: rows[length(rows) - 1]!})

triangle_max(input)
"#;

/// A number split into its digits, one JSON integer per digit.
fn digits(text: &str, what: &str) -> Value {
    Value::Array(
        text.chars()
            .filter(|c| c.is_ascii_digit())
            .map(|c| {
                json!(i64::from(
                    c.to_digit(10)
                        .unwrap_or_else(|| panic!("{what}: not a digit: {c:?}"))
                ))
            })
            .collect(),
    )
}

/// Each line split into its digits, one JSON integer per digit.
fn digit_rows(text: &str, what: &str) -> Value {
    Value::Array(
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                Value::Array(
                    line.trim()
                        .chars()
                        .map(|c| {
                            json!(i64::from(
                                c.to_digit(10)
                                    .unwrap_or_else(|| panic!("{what}: not a digit: {c:?}"))
                            ))
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

/// Each line split into whitespace-separated integers.
fn int_rows(text: &str, what: &str) -> Value {
    Value::Array(
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                Value::Array(
                    line.split_whitespace()
                        .map(|tok| {
                            json!(
                                tok.parse::<i64>()
                                    .unwrap_or_else(|_| panic!("{what}: not an integer: {tok:?}"))
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

/// One problem's fixed program, data file name, parser, and published answer.
type Case = (
    &'static str,
    &'static str,
    &'static str,
    fn(&str, &str) -> Value,
    &'static str,
);

/// Runs the four Euler programs against the contributor's own copies of the real puzzle data and
/// checks every backend against the published answer. It fails loudly rather than skipping: an
/// unset `EULER_DATA`, a missing or unparseable data file, or a wrong answer all turn red.
#[test]
#[ignore]
fn euler_real_data() {
    let dir = env::var("EULER_DATA").unwrap_or_else(|_| {
        panic!(
            "EULER_DATA is unset: point it at your own copies of the Euler 8/11/13/18 data \
             (see the module docs for the file names) and run `just euler-data`"
        )
    });
    let dir = Path::new(&dir);

    let cases: [Case; 4] = [
        ("8", PROGRAM_8, "euler08.txt", digits, "23514624000"),
        ("11", PROGRAM_11, "euler11.txt", int_rows, "70600674"),
        (
            "13",
            PROGRAM_13,
            "euler13.txt",
            digit_rows,
            "[5,5,3,7,3,7,6,2,3,0]",
        ),
        ("18", PROGRAM_18, "euler18.txt", int_rows, "1074"),
    ];

    for (problem, program, file, parse, want) in cases {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{file}: cannot read the Euler {problem} data from {}: {e}",
                path.display()
            )
        });
        let input = parse(&text, file).to_string();
        let failures = support::agreement_failures(
            &format!("euler/{problem}"),
            program,
            Some(&input),
            &support::Expect::Output(format!("{want}\n")),
        );
        assert!(failures.is_empty(), "{file}: {}", failures.join("\n"));
    }
}
