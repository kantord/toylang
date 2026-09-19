# transpose

`v | transpose`, of type `Vec<Vec<T>> -> Vec<Vec<T>>`: the rows and columns of the rectangular
`v` are swapped, so the entry at row `r`, column `c` of the input is the entry at row `c`,
column `r` of the result (gh:181). A `Vec` of rows only, never a stream -- the same blocking
`flatten` and `reverse` use.

The subject must be a rectangular `Vec<Vec<T>>`: every row has to have the same length. The
checker cannot see lengths, so a ragged input -- a `Vec` whose rows are not all the same length
-- is refused at runtime the same way every other hard failure is.

Built on the Go, Rust, JS, Python, and Lua backends so far; the native and jq backends have no
emitter arm yet, so a program using it there is refused with `` `transpose` has no native
backend yet; today it runs on go and rust and js and py and lua ``, this page carries no
runnable fragment, and there is no corpus case until they do (the
`transpose-remaining-backends` row in plans/board.yaml).
