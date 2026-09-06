# Recursive descent order: what other languages actually promise, and what it costs

Research spike for [Q7](questions.md#q7-does--promise-depth-first-order-or-only-the-set-of-nodes),
requested to reevaluate the provisional ruling (`..` promises depth-first order, jq-compatible)
against real alternatives before it hardens further. Three prior dispatch attempts on this row
died without producing anything: their event logs (`~/.cache/toylang-drive/opencode/202609*-issue-recursive-descent-order-research.jsonl`)
show `webfetch` erroring out immediately on every attempt, which is structural for a dispatched
sandbox worker in this environment, not a brief or capability problem -- the survey below was
done directly by the coordinator, verified against real behavior wherever a real interpreter was
available, rather than trusting a single web source (one search result claimed jq's `..` is
breadth-first; it is not, confirmed empirically below).

## jq: depth-first, pre-order (verified empirically, not just documented)

`..` is `recurse`, defined in the jq source as `def recurse(f): def r: ., (f | select(. != null)
| r); r;` -- visit `.` itself, then recurse into `f`'s output *before* returning to whatever
called `r`. That reads as depth-first pre-order, and running it confirms it:

```
$ echo '{"a": {"x": {"deep":1}, "y": 2}, "b": {"z": 3}}' | jq -c '[..]'
[{"a":{"x":{"deep":1},"y":2},"b":{"z":3}},{"x":{"deep":1},"y":2},{"deep":1},1,2,{"z":3},3]
```

The order is root, then fully into `a` (`a` → `a.x` → `a.x.deep`) before ever reaching `a.y`,
then only after all of `a` is exhausted does it move to sibling `b`. A breadth-first traversal
would have interleaved `a`, `b` before either's children; this doesn't. (jq 1.8.2, tested
directly -- a web search actually turned up one source claiming `..` is breadth-first, which
this empirical check refutes.)

## XPath: depth-first, document order, standardized

The `descendant` axis is a *forward axis*: it returns nodes in **document order**, and the XPath
data model defines document order as exactly a depth-first, left-to-right tree traversal. This
is a hard requirement in the spec, not an implementation choice -- two conformant XPath engines
cannot disagree on `//*`'s order.

## JSONPath (RFC 9535): depth-first pre-order mandated, but object-key order explicitly punted

RFC 9535 §2.5.2.2 (the descendant segment, `..`) is the most directly relevant precedent, because
it is the newest of the three and had the chance to design this deliberately rather than inherit
it. It **mandates depth-first pre-order traversal** ("nodes are visited before their
descendants") and **mandates array order is preserved** when visiting array children -- but
explicitly leaves **object member visitation order unspecified**, with the RFC's own reasoning
being that JSON objects are unordered data by definition. This is a real, standardized precedent
for splitting the question exactly the way toylang's own design might want to: the *shape* of the
traversal (depth vs. breadth) is pinned down as a real semantic guarantee, while *iteration order
within an unordered collection* is explicitly left free. For toylang this maps cleanly: a Vec's
element order is already meaningful (it's a sequence), but if some future toylang collection is
genuinely map-like/unordered, RFC 9535 is precedent for `..` still promising DFS pre-order *shape*
while not promising anything about iteration order *within* that collection.

## Columnar/vectorized engines: unordered by default, order is a real, visible, paid-for operation

DuckDB (representative of the vectorized/columnar family, and the direct analogue of the
"flat columnar layout, every node at every depth" framing in Q7's own text) **never guarantees
row order unless `ORDER BY` is explicit** -- the SQL standard itself doesn't define one either.
DuckDB will actively reorder unordered results across its parallel worker threads (gated by the
`preserve_insertion_order` setting) specifically because holding a stable order costs
synchronization between workers. `ORDER BY` is documented as a distinct, late-stage output
modifier -- logically the last step before `LIMIT`/`OFFSET` -- meaning: getting order back is not
free, it is a real, visible sort operation the engine performs *after* the embarrassingly-parallel
unordered work, with its own cost and its own line in the query plan.

This is the concrete cost model Q7 was asking for: an unordered `..` fast path isn't "the same
answer, computed faster" -- it's a genuinely different, weaker contract (a set/bag, not a
sequence), and a caller who needs the jq-compatible sequence back pays for a real, separate sort
step to reconstruct it, exactly the way DuckDB callers pay for `ORDER BY`. Whether that sort can
be cheap depends on what depth-first order actually requires reconstructing: DFS pre-order is
recoverable from a flat columnar layout if each node carries its own depth and parent-pointer (or
an equivalent tree-position key) — sorting by that key restores exactly the DFS order — so the
"a flat buffer must be scanned depth-first to be correct" framing in Q7 is not quite the full
picture: it's "either scan it in order, or carry enough position metadata to sort it into order
later," and the second option is what makes the fast path actually fast (existing columnar
positions/indices are reused, not a separate expensive tree walk).

## Answering Q7 directly

Every real-world precedent surveyed (jq, XPath, JSONPath/RFC 9535) treats depth-first pre-order
as the load-bearing semantic of a recursive-descent operator, not an incidental implementation
detail -- it's what `..`/`descendant`/`$..` *mean*. The provisional ruling (keep `..` promising
depth-first order, jq-compatible) matches every language actually in this space; nothing
surveyed does anything else. RFC 9535's specific carve-out (mandate the traversal shape, leave
unordered-collection iteration order free) is the one nuance worth carrying forward: if toylang
ever adds a genuinely unordered/map-like collection type, that specific sub-question (not the
top-level `..` order) is where "we don't promise order" has real precedent -- not for `..`
itself. Recommend closing Q7 as ratified with no reevaluation needed, and independently note the
columnar-engine finding (order comes from a separate sort against carried position metadata, not
from scanning in order) if `..`'s implementation strategy is ever revisited for performance,
since it means an unordered internal execution strategy remains compatible with the guaranteed
external order, at the cost of one sort keyed on tree position.

Sources:
- [jq 1.8 Manual](https://jqlang.org/manual/) (recurse/`..` definition)
- [XML Path Language (XPath) 2.0](https://www.w3.org/2003/02/DIFF-xpath20.html) (document order, forward axes)
- [RFC 9535: JSONPath: Query Expressions for JSON](https://datatracker.ietf.org/doc/html/rfc9535) (§2.5.2.2, descendant segment)
- [ORDER BY Clause – DuckDB](https://duckdb.org/docs/lts/sql/query_syntax/orderby)
- [DuckDB quacks Arrow: A Zero-Copy Data Integration between Apache Arrow and DuckDB](https://duckdb.org/2021/12/03/duck-arrow)

Human-authored: none. Derived: the jq empirical test (run directly, 2026-09-07) and the survey
above, from the cited primary sources rather than the one contradicting secondary source found
along the way.
