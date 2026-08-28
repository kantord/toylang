# Type aliases

`type Name = ...` abbreviates a type. An alias is not a new type: the program is the same
program as one with the shapes written out, and every backend emits the same bytes for both.

```case
type_alias
```

An alias name starts with a capital letter, like every type name, and cannot rebuild a
built-in (`Int`, `Vec`, ...) or be defined twice. An alias written in terms of itself is
refused rather than expanded forever; there is no indirection for a recursive type to hide
behind yet, and the same holds for an enum whose payload mentions itself.
