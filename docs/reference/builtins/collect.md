# collect

`collect(s)`, of type `Stream<T> -> Vec<T>`: the one place a stream stops being a stream.
What comes back is an ordinary value, exactly as sized as it needs to be, with no trace of
how it arrived.

A `Stream` is born at a source (`inputs`, `lines`), consumed exactly once, and cannot be
printed, stored in a record, or indexed; `collect` is the explicit boundary where it becomes
a `Vec` that can do all of those things. The cost is equally explicit: `collect` waits for
the whole stream, so nothing downstream of it starts until stdin closes.

```toylang
unlines(collect(lines))
```

```input
ada
bo
cy
```

```output
ada
bo
cy
```

Counting a stream is the shape that shows why the boundary is spelled out: the count cannot
exist until everything has been read.

```toylang
fn total(nums: Vec<Int>) -> Int = extent(nums)

total(collect(inputs))
```

```input
1
2
3
4
```

```output
4
```

A pipeline that never needs the whole value at once should stay a stream and end at a sink
([`jsonlines`](jsonlines.md)) instead: that shape processes one entry at a time, printing as
it goes.
