# str

`str(n)`, of type `Int -> Str`: renders an integer the way the printer does, but reachable
from inside a program, where a number needs to take part in string concatenation.

```toylang
"the answer is " + str(42)
```

```output
the answer is 42
```

Negative numbers render with the sign, exactly as a top-level `Int` result would print:

```toylang
str(-7)
```

```output
-7
```

`str` takes only `Int`. There is no generic to-string: a `Str` is already one, and what a
record or a `Vec` should look like as text is the printer's decision, not a value the program
can get its hands on.
