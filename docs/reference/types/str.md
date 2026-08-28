# Str

A string. Literals are double-quoted with C-style escapes; `+` concatenates two of them
(see [+ and the comparisons](../operators/arithmetic.md) for what `+` does elsewhere).

```toylang
"say \"hi\"" + "\n" + "tab\there"
```

```output
say "hi"
tab	here
```

A top-level `Str` result prints raw, as above. A `Str` anywhere inside the result -- an entry
of a `Vec`, a field of a record -- prints as JSON, quoted and escaped:

```toylang
["say \"hi\"", "a\\b"]
```

```output
["say \"hi\"","a\\b"]
```

There is no string length, splitting, or indexing: a `Str` has no dimensions, so `extent` and
the index specs do not apply. What exists today is concatenation, equality, and ordering.
Ordering (`<` and friends) compares by Unicode codepoint, pinned across every backend including
past the astral plane:

```toylang
"Ａ" < "😀"
```

```output
true
```

`str(n)` renders an `Int` as a `Str`; there is no conversion in the other direction.
