# Match

Consuming an [enum](../types/enum.md) is a match: the subject arrives through a pipe, arms
chain with `//`, the first arm that matches wins, and the set is closed-world -- every
variant handled, or a final `any()` for the rest.

```case
enum_match
```

Three arm shapes:

- `point -> ...`: a unit variant, by name.
- `circle{r} -> r * r`: a payload variant with a record pattern, binding fields fresh.
- `text -> .body`: a bare payload arm; `.` rebinds to the payload, so a scalar payload is
  used whole and a record payload is projected into.

`any() -> ...` stands in for every variant nothing named:

```case
enum_match_default
```

`//` exists only between match arms; it is not division (that is `/`) and not a comment
(that is `#`). The spelling came from jq's alternative operator, reading the arm chain as
alternatives tried left to right.

Missing a variant without `any()` is a compile error naming what is missing; the
[enum page](../types/enum.md) shows it. Matching is decoding, not control flow bolted on: a
match is how a value whose shape is unknowable until runtime becomes typed data again.
