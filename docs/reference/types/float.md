# Float

An IEEE 754 binary64, JavaScript's number
([ADR 0007](../../adr/0007-float-is-javascripts-double.md)). A literal with a decimal point
or an exponent is a `Float`; one without is an [Int](int.md), and the two never meet in one
operator, so `1 + 1.5` is refused rather than promoted.

```case
float_arith
```

Integral values print without a trailing `.0`, and the rest print the shortest digits that
read back to the same double, in fixed notation from `1e-7` up to `1e21` and exponential
outside it -- ECMA-262's `Number::toString`, byte for byte, on every backend:

```toylang
1e21
```

```output
1e+21
```

```toylang
1e-7
```

```output
1e-7
```

Float arithmetic is total. Where an `Int` divisor of zero is the one way integer arithmetic
fails, a `Float` divisor of zero is the IEEE answer, and `NaN` and the two infinities are
values a `Float` holds and prints by name (kantord/toylang#145):

```case
float_div_by_zero
```

```case
float_nan
```

That output is not JSON, which is the price of admitting the values rather than failing on
the operation that produced them or mapping them to `null` at the boundary. Comparison
follows IEEE too: `NaN` is not equal to itself, and neither less nor greater than anything.

```case
float_nan_self_comparison
```

Input is the other way a `Float` enters. A JSON number already is the double a `Float` names,
so `parse` reads it without a conversion step:

```case
float_input
```

A `Float` inside a `Vec`, a record, or an enum payload prints the same way, and so do the
non-finite values, on every backend:

```case
float_nested_non_finite
```

What a `Float` cannot do yet: `str` takes an `Int` only; `sum` and `max` take `Int` or
`Int64`; and the ordering builtins (`sort`, `sort_by`, `max_by`) take `Int`, `Int64`, `Str`,
or `Char`. All of them refuse a `Vec<Float>`, since `NaN` has no place in a total order:

```toylang
sort([2.5, 1.5])
```

```error
`sort` needs a Vec of Int, Int64, Str, or Char, found Vec<Float> (at byte 5)
```
