# `$`

`$` is a call's deferred parameter: written inside the sole argument of a function call, it
turns that whole call into a closure over whatever `$` stands for, rather than evaluating the
call directly. It exists only there -- a function-parameter-expression is either an ordinary
expression, or one containing `$`, and nothing else in the grammar accepts `$` at all, the
same way `.` is refused wherever nothing bound it.

Landed on the Rust, Go, JS, Python, Lua and jq backends so far (closures-first-class-functions-design,
2026-09-23). The native backend has no representation for a stored closure value yet, so a
program using `$` refuses cleanly there rather than emit something wrong.

jq has no function values, so a closure there is a JSON object, `{__closure: id, captures:
[...]}`, and applying one calls a single generated `tl_apply` filter that switches on the id and
runs that site's body. Whatever the body reads from outside is copied into `captures` when the
closure is built. One program shape jq refuses: a closure body that calls a function which itself
applies a closure, because `tl_apply` and that function would each have to be defined before the
other and jq's `def` has no forward declaration.

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
