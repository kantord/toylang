# Int

A 32-bit signed integer that wraps on overflow
([ADR 0006](../../adr/0006-int-is-32-bits-and-wraps.md)). Both halves are the observable
contract: every backend carries exactly these values, and arithmetic that leaves the range
comes back around rather than trapping or widening.

```toylang
str(2147483647 + 1)
```

```output
-2147483648
```

A literal must fit. `2147483648` is refused at compile time, on every backend, because a
value the type cannot hold should not exist long enough to disagree about -- unless the
position it sits in expects an [Int64](int64.md), which is the only way a wider literal
enters:

```toylang
str(2147483648)
```

```error
integer `2147483648` does not fit in Int, which is 32 bits; only a position that expects Int64 can hold it (at byte 4)
```

Input is the other place an `Int` enters, and the rule holds there too: a JSON number that
does not fit in 32 bits, or is not an integer at all, is refused before the program runs.

Division truncates toward zero, and the remainder takes the sign of the dividend; see
[arithmetic](../operators/arithmetic.md). A top-level `Int` result prints as the bare
number.
