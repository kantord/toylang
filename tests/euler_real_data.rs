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
fn product(v: Vec<Int>) -> Int64 =
    length(v) == 0 | . -> 1 or i64(v[0]!) * product(tail(v)!)

fn windows(v: Vec<Int>) -> Vec<Int64> =
    collect(range(length(v) - 12)) | map(product(v[.:. + 13]))

max(windows(parse(stdin)))!
"#;

const PROGRAM_11: &str = r#"
fn get({g, r, c}: {g: Vec<Vec<Int>>, r: Int, c: Int}) -> Int = g[r]![c]!

fn four({g, r, c, dr, dc}: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int =
    get({g: g, r: r, c: c}) * get({g: g, r: r + dr, c: c + dc}) *
        get({g: g, r: r + 2 * dr, c: c + 2 * dc}) *
        get({g: g, r: r + 3 * dr, c: c + 3 * dc})

fn row_products({g, r, dr, dc, cmin, cmax}: {g: Vec<Vec<Int>>, r: Int, dr: Int, dc: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    collect(range(cmax))
        | select(. >= cmin)
        | map(four({g: g, r: r, c: ., dr: dr, dc: dc}))

fn direction({g, dr, dc, rmax, cmin, cmax}: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    flatten(
        collect(range(rmax))
            | map(
                  row_products(
                      {g: g, r: ., dr: dr, dc: dc, cmin: cmin, cmax: cmax}
                  )
              )
    )

fn largest_product(g: Vec<Vec<Int>>) -> Int =
    let rows = length(g)
    let cols = length(g[0]!)
    let right = direction({g: g, dr: 0, dc: 1, rmax: rows, cmin: 0, cmax: cols - 3})
    let down = direction({g: g, dr: 1, dc: 0, rmax: rows - 3, cmin: 0, cmax: cols})
    let diagonal = direction({g: g, dr: 1, dc: 1, rmax: rows - 3, cmin: 0, cmax: cols - 3})
    let antidiagonal = direction({g: g, dr: 1, dc: -1, rmax: rows - 3, cmin: 3, cmax: cols})
    max(flatten([right, down, diagonal, antidiagonal]))!

largest_product(parse(stdin))
"#;

const PROGRAM_13: &str = r#"
fn column_total({nums, k, carry}: {nums: Vec<Vec<Int>>, k: Int, carry: Int}) -> Int =
    sum(nums | map(.[k]!)) + carry

fn emit_carry({carry, acc}: {carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    carry == 0
        | . -> acc or emit_carry({carry: carry / 10, acc: [carry % 10] + acc})

fn add_digits({nums, k, carry, acc}: {nums: Vec<Vec<Int>>, k: Int, carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    let total = column_total({nums: nums, k: k, carry: carry})
    k == 0
        | . -> emit_carry({carry: total / 10, acc: [total % 10] + acc}) or
              add_digits({nums: nums, k: k - 1, carry: total / 10, acc: [total % 10] + acc})

fn leading_digits(nums: Vec<Vec<Int>>) -> Vec<Int> =
    add_digits({nums: nums, k: length(nums[0]!) - 1, carry: 0, acc: []})[0:10]

leading_digits(parse(stdin))
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

triangle_max(parse(stdin))
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
