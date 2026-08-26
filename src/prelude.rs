//! The rudimentary beginning of a module system: one file, `prelude.toy`, whose `pub`
//! definitions are always available to every program. There is no import statement yet -- a
//! program does not name what it wants, it just gets all of it -- and no way for a program's own
//! file to be imported by another. What already exists: `pub fn` marks a definition as part of
//! that always-available set; a non-`pub` one is parsed but never included in any compiled
//! program, so it cannot yet serve as a private helper for a `pub` one either.
//!
//! `unlines` used to be a `tir::Builtin`, needing its own codegen in all six backends. Written as
//! ordinary toylang source instead, it is checked and compiled exactly the way a program's own
//! functions are, so getting it right once in the checker is getting it right everywhere.
//!
//! Every `pub` definition is always merged in, whether the program uses it or not; what keeps an
//! unused one from cluttering output and `tags::node_types` is `check::check`'s reachability
//! pass, which prunes any function -- prelude or the program's own -- that the program's body
//! never calls, directly or transitively.

use crate::ast::Def;

const PRELUDE_SRC: &str = include_str!("../prelude.toy");

/// Every `pub` definition in `prelude.toy`.
pub fn defs() -> Vec<Def> {
    let defs = crate::parse::parse_module(PRELUDE_SRC).expect("prelude.toy is valid toylang");
    defs.into_iter().filter(|d| d.is_pub).collect()
}

/// Prepends the prelude's definitions to `file`, so a program can call any of them. Prepended
/// rather than appended, so a program that redefines the same name is the one flagged as the
/// duplicate -- its span is the one worth showing.
pub fn inject(file: &mut crate::ast::File) {
    let mut merged = defs();
    merged.append(&mut file.defs);
    file.defs = merged;
}
