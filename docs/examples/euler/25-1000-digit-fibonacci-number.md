# When Fibonacci reaches a thousand digits

Solves [Project Euler 25](https://projecteuler.net/problem=25). See the
[spoiler warning](00-spoiler-warning.md).

Unlike [problem 29](29-distinct-powers.md), there is no factoring trick here: consecutive
Fibonacci terms share only an additive recurrence, so the digits have to exist somewhere for
the length check to mean anything. Somewhere is a `Vec<Int>` of base-10^8 limbs, least
significant first, the representation [problem 16](16-power-digit-sum.md) uses with a
larger radix: eight digits per limb means a 1000-digit term is 125 entries rather than a
thousand, and adding two limbs plus a carry stays under 2 * 10^8, well inside `Int`.
`add_limbs` adds one column at a time, `limb` reading a zero past the shorter term, and
appends the final carry (at most 1) as a new top limb. `digits_of` counts eight per limb below
the top one and then the top limb's own digits, so no term is ever printed or held as a
number. `first_with` walks the recurrence from `F(2)` until the count reaches 1000 and answers
with the term's index.

```toylang
fn limb({ v, i }: { v: Vec<Int>, i: Int }) -> Int =
  i < length(v) | . -> v[i]! or 0

fn add_limbs(
  { a, b, i, carry, acc }: {
    a: Vec<Int>,
    b: Vec<Int>,
    i: Int,
    carry: Int,
    acc: Vec<Int>
  }
) -> Vec<Int> =
  let total = limb({ v: a, i: i }) + limb({ v: b, i: i }) + carry
  i == length(a)
  | . -> (carry == 0 | . -> acc or acc + [carry]) or
    add_limbs(
      {
        a: a,
        b: b,
        i: i + 1,
        carry: total / 100000000,
        acc: acc + [total % 100000000]
      }
    )

fn digit_count(n: Int) -> Int =
  n < 10 | . -> 1 or 1 + digit_count(n / 10)

fn digits_of(v: Vec<Int>) -> Int =
  (length(v) - 1) * 8 + digit_count(v[-1]!)

fn first_with(
  { prev, cur, n, want }: {
    prev: Vec<Int>,
    cur: Vec<Int>,
    n: Int,
    want: Int
  }
) -> Int =
  digits_of(cur) >= want
  | . -> n or
    first_with(
      {
        prev: cur,
        cur: add_limbs({ a: cur, b: prev, i: 0, carry: 0, acc: [] }),
        n: n + 1,
        want: want
      }
    )

first_with({ prev: [1], cur: [1], n: 2, want: 1000 })
```

```output
4782
```
