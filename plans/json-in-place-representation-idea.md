# In-place JSON: working over the serialized bytes, not a deserialized tree

Idea capture, not a proposal. Daniel: use a data structure that holds a value as regular
serialized JSON in memory, so reading and writing it never needs a deserialize/reserialize
round-trip the way building a tree of typed nodes does.

## The precedent

Derived from public knowledge of two real Rust crates, not invented here:

- **`simd-json`**'s tape representation. Parsing builds a flat, indexed structure alongside
  the original byte buffer rather than a tree of owned nodes; navigating a value means
  walking the tape and slicing the original bytes, not allocating strings and containers.
- **`serde_json::value::RawValue`**. A narrower, more standard version of the same idea:
  defers parsing of a subtree entirely, holding the original slice of the source text so a
  value can be passed through (or reserialized) byte-for-byte without ever being parsed at
  all, when nothing needs to inspect it.

The general shape both share: the serialized bytes *are* the working representation, with an
index or a deferred pointer standing in for the parse a tree-building deserializer would do
up front.

## Where this might matter in toylang, if it goes anywhere

Not evaluated, just named. `src/input.rs` currently parses stdin fully into a
`serde_json::Value` tree before `validate` walks it against the declared type -- exactly the
shape this idea would replace for the parts of a value the program never inspects (an
unvalidated Str payload just passing through, say). More generally, any path where a value
is read and written without the program actually computing over its shape (a pass-through
`Vec<Json>`-style field, once [Q25](questions.md#q25-does-the-language-have-union-types)'s
union-type gap or the `Json` escape hatch draft.md:28 already names is in scope) is a
candidate: today that round-trips through a full parse and a full reserialize for no
benefit.

## Status

Unscheduled, no urgency -- same treatment as [bigint tracking](bigint-tracking-research.md):
recorded so the idea survives, not committed to. Nothing here proposes a change; whether it
is worth building depends on evidence this note doesn't have (a measured parse/reserialize
cost that actually matters, and a concrete place in the runtime that would use it).
