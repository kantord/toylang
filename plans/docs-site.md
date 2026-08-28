# The docs site: four page types, none of which can lie

Generalizes the corpus-browser site into real documentation. Decided in a grilling session
(2026-08-28); this records the decisions and orders the build.

## The decisions

**Every fragment is a tested artifact.** A code fragment on any page is a real program the
suite runs: a `toylang` fence, an optional `input` fence, and an `output` (or `refuses`)
fence form a fragment, and a docs harness extracts every one and pushes it through the same
seven-backend agreement check the corpus uses. A docs fragment is a corpus case defined in
prose. Drift fails `just test`; in-page runnability comes free because every fragment is a
real program. A page may embed an existing corpus case by id instead of repeating it.
Rejected: write-time-only verification (silent rot), one-backend verification (docs would
claim less than the corpus does).

**Diataxis navigation.** Four sections, each with one job: Tutorial (one linear course),
Guides (one page per feature, task-oriented), Reference (every builtin, type, and operator),
Examples (the existing corpus browser, unchanged, as one section). Rejected: blending
teaching and task-solving into "per-feature tutorials"; a fifth Design section surfacing
draft.md (not presentation-ready; revisit later).

**Pages are markdown in-repo.** The site (Vite/React) builds them; content lives with the
code so the harness and the corpus can see it.

**Launch = skeleton plus a complete reference.** All four sections exist from day one.
Reference is complete for everything implemented -- checkably: the harness insists every
builtin has a page, the same way tag_corpus insists on node_types. Tutorial ships its first
chapters (values, records, pipe/select/map, enums, streams). Guides seeds two pages (enums,
streams). Examples is the current browser relocated.

## Build order

1. The fence harness (`tests/docs.rs`-shaped): extract fragments from `docs/**/*.md`, run
   them as corpus cases, plus the every-builtin-has-a-reference-page completeness check.
2. Site navigation and page types: four sections, markdown rendering, the corpus browser
   moved under Examples.
3. Content: complete reference, tutorial chapters, two guides -- every fragment written
   against the real compiler, since the harness will not have it any other way.

Footprint warning for the drive loop: the harness touches `tests/` and test runs regenerate
`site/public/corpus.json`, so this build serializes behind any delegation that adds corpus
cases (scalar payloads, at the time of writing).
