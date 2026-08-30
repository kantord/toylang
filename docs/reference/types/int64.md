# Int64

A signed 64-bit integer that wraps on overflow: [Int](int.md)'s rules at twice the width
([ADR 0010](../../adr/0010-int64-is-a-second-integer-type.md)). It exists for values that are
carried more than computed -- millisecond timestamps, Euler-sized factors -- which `Int`'s
2.1 billion ceiling turns away.

Literals carry no suffix. One that fits `Int` is `Int`, and any literal resolves as `Int64`
wherever one is expected -- the same rule that gives `[]` its element type. So the wide
literal below is legal only because the annotation says what it is:

```toylang
fn big() -> Int64 = 600851475143

big()
```

```output
600851475143
```

A wide literal with no such position stays an error; nothing guesses:

```toylang
600851475143
```

```error
integer `600851475143` does not fit in Int, which is 32 bits; only a position that expects Int64 can hold it (at byte 0)
```

Nothing widens implicitly. `Int` and `Int64` never meet in one operator, and the error names
[i64](../builtins/i64.md), the one bridge:

```toylang
fn big() -> Int64 = 5

1 + big()
```

```error
`+` cannot mix Int and Int64; widen the Int side with `i64(...)` (at byte 27)
```

A bare literal on either side of an `Int64` operator needs no bridge -- its position already
says which width it has:

```toylang
fn big() -> Int64 = 1234567890123456

big() + 1
```

```output
1234567890123457
```

Arithmetic and comparison work exactly as on `Int`: division truncates toward zero, the
remainder takes the dividend's sign, a zero divisor is the only failure, and everything else
wraps, `MIN / -1` included. A top-level `Int64` result prints as the bare number, and one
inside a record or Vec prints as an ordinary JSON number.

Two boundaries are part of the contract rather than hidden behind it:

- `input` cannot be read as `Int64` anywhere in its shape. An `Int64` result prints fine, but
  reading one back is codec design nobody has done -- JavaScript parses JSON numbers into
  doubles and is off past 2^53 -- so the read side is refused until it can be done honestly,
  the same reversible direction [Opt](opt.md) takes.
- The jq backend computes `Int64` arithmetic in IEEE doubles, which are exact only within
  +/-2^53 (about 9.0e15). Inside that envelope jq agrees with the other six backends
  exactly; a program whose values leave it diverges on jq -- in particular the wrap at 2^63
  never happens there, because no double can carry the wrapped value back. The other six
  backends are exact at the full width, and their agreement on the wrapping edges is pinned
  in tests/int64.rs.
