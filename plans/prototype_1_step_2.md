# Step 2: real parser

```
"hello " + "world"
```

Step 1 accepts exactly one literal, so its parser can be a single token match. This step removes
that: a Pratt loop with a precedence table, and `+` on `Str`.

The table has one entry. It exists now because step 4 adds `|`, `,` and comparison at three
different levels, and retrofitting precedence into a hand-written parser that grew without it is
the usual way these get rewritten.

## Done when

The same binary compiles step 1's program and this one, with a snapshot for each.
