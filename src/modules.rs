//! `@(path)` module routing (gh:162, gh:167): a program names another file of declarations,
//! and `@(path)` applies that module's `handle` to `.`. This is the loader. It resolves each
//! path, parses the file as a module, and merges its declarations into the program the way
//! `prelude::inject` merges the prelude's, tagged `Origin::Module(path)` so the checker's
//! per-call-site visibility rule keeps a module's private helpers to that module. The four
//! rulings this implements (board row module-routing-semantics-build, ruled 2026-09-07): the
//! entry function is always named `handle`; the call is a fully strict typed call against
//! `handle`'s declared signature (`check::routing`); every def merges, prelude-style, with
//! visibility by origin; and a module's enum variants stay qualified, never joining the bare
//! variant lookup (`check::resolve_defs`).
//!
//! Every module has a `handle`, so that one name cannot merge bare: each module's entry is
//! renamed to `handle::<path>` (`entry_name`) before merging, and `File::routes` records which
//! internal name each `@(path)` reaches. `::` is what an impl method's synthesized name already
//! uses, for the same reason: no user-written identifier can contain it, and every backend
//! escapes it (`tir::escape_name`). The rename also means `handle` is reachable only through
//! `@(path)`, never by its bare name, `pub` or not.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast::{File, Origin, RouteRef};
use crate::error::Error;

/// Loads every module `file` routes to, transitively, resolving each `@(path)` against the
/// directory of the file it was written in (`dir` for the program itself), and merges their
/// declarations in. Prepended like the prelude, so a program that redefines a module's name is
/// the one flagged as the duplicate: its span is in the file the error is read against.
pub fn inject(file: &mut File, dir: &Path) -> Result<(), Error> {
    // Canonical path -> entry name, so a module routed twice, under two spellings or from two
    // files, is loaded and merged once. A module routing to its own file lands here too.
    let mut loaded: HashMap<PathBuf, String> = HashMap::new();
    let mut pending: Vec<(Origin, PathBuf, RouteRef)> = file
        .route_refs
        .drain(..)
        .map(|r| (Origin::Program, dir.to_path_buf(), r))
        .collect();
    while let Some((from, base, r)) = pending.pop() {
        let path = base.join(&r.path);
        let (canonical, src) = match path
            .canonicalize()
            .and_then(|c| Ok((c, std::fs::read_to_string(&path)?)))
        {
            Ok(read) => read,
            Err(e) => {
                return Err(Error::new(
                    r.span,
                    format!("cannot read module `{}`: {e}", r.path),
                ));
            }
        };
        if let Some(entry) = loaded.get(&canonical) {
            file.routes.insert((from, r.path), entry.clone());
            continue;
        }
        // A parse error's span is a byte offset into the module, not the program, so the
        // message says which file it is counting in.
        let mut module = crate::parse::parse_module(&src)
            .map_err(|e| Error::new(r.span, format!("in module `{}`: {e}", r.path)))?;
        let Some(handle) = module.defs.iter_mut().find(|d| d.name == "handle") else {
            return Err(Error::new(
                r.span,
                format!("module `{}` has no `handle` function to route to", r.path),
            ));
        };
        // The origin shows the path relative to the program's directory rather than the
        // canonical one: it is what error messages print, and a canonical path names the
        // machine the program was compiled on.
        let shown = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
        let entry = entry_name(&shown);
        handle.name = entry.clone();
        let origin = Origin::Module(shown);
        for def in &mut module.defs {
            def.origin = origin.clone();
        }
        for imp in &mut module.impls {
            imp.origin = origin.clone();
        }
        for e in &mut module.enums {
            e.origin = origin.clone();
        }
        let module_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        for nested in module.route_refs.drain(..) {
            pending.push((origin.clone(), module_dir.clone(), nested));
        }
        loaded.insert(canonical, entry.clone());
        file.routes.insert((from, r.path), entry);

        module.defs.append(&mut file.defs);
        file.defs = module.defs;
        module.aliases.append(&mut file.aliases);
        file.aliases = module.aliases;
        module.enums.append(&mut file.enums);
        file.enums = module.enums;
        module.traits.append(&mut file.traits);
        file.traits = module.traits;
        module.impls.append(&mut file.impls);
        file.impls = module.impls;
    }
    Ok(())
}

/// The internal name a module's `handle` merges under: `handle::` plus the module's path with
/// every character an identifier cannot hold replaced by `_`, so `routes/greet.toy` becomes
/// `handle::routes_greet_toy`. Two distinct paths that flatten to the same name collide in
/// `check::signatures` as a definition made twice, loudly rather than silently.
fn entry_name(shown: &Path) -> String {
    let ident: String = shown
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("handle::{ident}")
}
