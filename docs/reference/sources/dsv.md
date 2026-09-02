# dsv

`dsv(delim)` is stdin as rows of fields: each raw line, read the way
[`lines`](lines.md) reads one, is split on the delimiter into a `Vec<Str>`, so the
source's type is `Vec<Vec<Str>>`. It is the parameterized member of the sources
family -- `csv` and `tsv` are the same source with the delimiter already fixed to `,`
and a tab (gh:88's ruling, built as gh:136).

```case
dsv_rows
```

The split is the literal one `str.split(delim)` gives on every backend: no quoting, no
escaping, no header handling -- "DSV" as opposed to CSV's quoting rules. A line with no
delimiter is one whole field; a trailing delimiter is a trailing empty field; a blank
line is a line, so it splits into one empty field, matching `lines`'s "blank ones
included".

```case
dsv_trailing_delimiter
```

```case
dsv_blank_line_is_an_empty_field
```

`csv` and `tsv` are predefined partial applications of `dsv`, available to every
program the way the prelude's names are. `dsv` itself takes the delimiter as a string
literal argument, so a program with a less common separator writes it out rather than
learning a new keyword.

```case
csv_partial
```

```case
tsv_partial
```

Being eager like [`input`](input.md), `dsv` needs no `collect` -- it is already a value
-- but it flows through the same `map`/`jsonlines` shapes, since those distribute over a
`Vec` as well as a stream.

```case
jsonlines_of_dsv
```

`dsv` reads the same real stdin as [`input`](input.md), [`inputs`](inputs.md), and
[`lines`](lines.md), so a program uses at most one of the four, and `dsv` is read at
most once. The empty delimiter is refused: every backend's split on an empty separator
is its own undefined behaviour.
