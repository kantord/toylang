Board row `benchmark-fannkuch-redux-build`: build the CLBG fannkuch-redux benchmark task in
toylang (plans/benchmark-plan.md Build status). It is fully numeric (permutations of
Vec<Int>), so it needs no Str slicing or Char -> Str conversion -- unlike
reverse-complement/k-nucleotide/regex-redux, which are blocked on those.

Follow the pattern already landed in `benches/programs/binary-trees.toy` and
`benches/programs/fasta.toy` (both done): a program under `benches/programs/`, correctness
pinned as a small-N case in `tests/corpus/`, and a larger fixture under `benches/inputs/` for
real timing via `just bench NAME` (src/bin/bench.rs).

The CLBG fannkuch-redux task: for a given N, generate all permutations of [1..N], and for
each permutation repeatedly flip the prefix of length equal to the first element until it is
1, counting flips; track the maximum flip count over all permutations and the running
checksum (sum of flip counts, alternating sign by permutation parity). Reference the
classic CLBG description/algorithm if useful, but express it in toylang idioms (the `|`
match-chain style already used by binary-trees.toy and fasta.toy), not a transliteration of
an imperative implementation.

Done-gate: `just check` passes, the new corpus test pins correctness on a small N, and
`benches/programs/fannkuch-redux.toy` runs across the backends that already support the
Vec<Int> operations it needs (check which backends currently refuse it and note why, same as
fasta.toy did for its own constraints).
