# `$`

`$` is a call's deferred parameter: written inside the sole argument of a function call, it
turns that whole call into a closure over whatever `$` stands for, rather than evaluating the
call directly. It exists only there -- a function-parameter-expression is either an ordinary
expression, or one containing `$`, and nothing else in the grammar accepts `$` at all, the
same way `.` is refused wherever nothing bound it.

Runs on all seven backends (closures-first-class-functions-design, 2026-09-23). Each represents
the closure its own way: a function value where the target has one, `Rc<dyn Fn>` on Rust, and
on the native backend a heap record holding the address of the closure's compiled function
followed by the values its body reads from outside, called through that address.

jq has no function values, so a closure there is a JSON object, `{__closure: id, captures:
[...]}`, and applying one calls a single generated `tl_apply` filter that switches on the id and
runs that site's body. Whatever the body reads from outside is copied into `captures` when the
closure is built. A closure body that calls a function which itself applies a closure would need
`tl_apply` and that function each defined before the other, so such a function is emitted a
second time inside `tl_apply`, where the enclosing def is in scope.

Two things fall out of the one rule, not two mechanisms:

- **A predicate for a caller-supplied closure parameter.** A function whose own signature
  declares a parameter as a closure (`Int -> Bool`) can have that parameter built at the call
  site with `$` standing for the missing input:

  ```
  fn count_where({items, pred}: {items: Vec<Int>, pred: Int -> Bool}) -> Int =
      items | select(pred) | length(.);

  count_where({items: [1, 2, 3, 4], pred: $ > 2})
  ```

  ```
  2
  ```

  `select` itself did not change: it still accepts an inline `.`-expression exactly as
  before, and now also accepts a genuine closure value handed to it as an ordinary
  parameter -- `pred` above, applied to each element instead of `.`-rebinding one.
  [`map`](../builtins/map.md), [`sort_by`](../builtins/sort_by.md) and
  [`max_by`](../builtins/max_by.md) read a closure the same way, so a key or mapper can be
  a parameter too:

  ```
  fn best({items, key}: {items: Vec<Int>, key: Int -> Int}) -> Opt<Int> =
      items | max_by key;

  best({items: [3, 8, 5, 6, 1], key: $ % 5})
  ```

  ```
  3
  ```

  The reading is a syntactic peek: only a bare name already bound to a function is applied
  per element. Any other argument (`max_by(.age)`, or a name that holds an Int) keeps
  meaning a `.`-rebinding expression. A closure that takes another type than the elements,
  or returns one the operator cannot use (a non-scalar key, a non-Bool predicate), is
  refused at the name.

- **Partial application.** A bare `$` as one field's value in a record literal defers that
  field: `join({with: ", ", over: $})` is a function from the `over` field's type to
  `join`'s own result, built by leaving that one field unfilled. The residual is always
  unary, over exactly the one deferred field's type -- at most one field may be bare `$`
  per call, so there is no second, record-shaped residual to define.

`$` may be referenced only once inside the expression it appears in; a lambda needing `$`
more than once (`$ * $` to square, say) is not yet spellable this way -- write a named
function instead (`fn f(x: Int) -> Int = x * x`) until that terseness lands as its own row.

A closure never captures a `Stream` or a `Sink`: the checker refuses one reached from inside
`$`'s body the same way it refuses one read inside a `map`/`select` body, since either might
run more than once. This is a deliberately temporary restriction, not a permanent design
limit -- revisit once a real program needs a closure that captures a stream.

A closure also never outlives the call it was built for: nothing stores one in a `let`, a
`Vec`, or a return type, so a partial application cannot be built once and reused from two
different call sites. Both restrictions come from the same rule -- `$` only exists inside a
function-parameter-expression -- rather than being checked separately.
