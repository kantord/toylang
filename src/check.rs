use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::ast::{
    Alias, BinOp, Def, EnumDecl, Expr, FieldsPattern, File, MatchArm, Pattern, Span, TypeExpr,
};
use crate::error::Error;
use crate::tir::{self, Kind, LocalId, Tir};
use crate::ty::{self, Sig, Type};

struct Ctx<'a> {
    sigs: &'a HashMap<String, Sig>,
    /// Every declared enum, resolved. Keyed by name, which is the identity.
    enums: &'a HashMap<String, Type>,
    /// Which enums declare each variant name, in declaration order. A bare variant use resolves
    /// through this while exactly one enum claims the name; two claimants are a loud error
    /// naming both, with `Shape.circle` as the qualified way out.
    variant_owners: &'a HashMap<String, Vec<String>>,
    /// Named bindings. At most one, since functions are unary and there is no `let`.
    scope: Vec<(String, Type)>,
    /// Names a match arm's record pattern bound, innermost last: reading one reads that field
    /// off the arm's payload local, so nothing beyond ordinary `Field` nodes reaches the
    /// backends. Each entry is the bound name, the payload's record type, and its local.
    arm_fields: Vec<(String, Type, LocalId)>,
    /// What `.` refers to here, if anything: its type and the local holding it.
    subject: Option<(Type, LocalId)>,
    /// The type `input` was checked against, filled in the first time it is used.
    input: &'a RefCell<Option<Type>>,
    /// The element type `inputs` was checked against, filled in the first time it is used. A
    /// separate cell from `input`'s: one records a whole value's type, the other one line's.
    inputs: &'a RefCell<Option<Type>>,
    /// Whether `lines` has already been read. There is only ever one real stdin, so a second
    /// read is refused rather than silently handed nothing, the way a second pass over an
    /// already-consumed iterator would be in a language that let it compile.
    lines_used: &'a Cell<bool>,
    /// Whether checking is inside a mapper's body (`map`'s, or `select`'s predicate), which
    /// runs once per element: a source read there would drain stdin on the first element and
    /// hand every later one nothing, so `lines` and `inputs` are refused in that position.
    in_mapper: bool,
    /// The name of the `fn` whose body is being checked; `None` in the program's own body. A
    /// source is legal only in the program body (see `source_in_fn`), so `lines` and `inputs`
    /// are refused whenever this is set.
    in_fn: Option<&'a str>,
    next_local: &'a Cell<LocalId>,
}

impl Ctx<'_> {
    fn with(&self, subject: Option<(Type, LocalId)>) -> Ctx<'_> {
        Ctx {
            sigs: self.sigs,
            enums: self.enums,
            variant_owners: self.variant_owners,
            scope: self.scope.clone(),
            arm_fields: self.arm_fields.clone(),
            subject,
            input: self.input,
            inputs: self.inputs,
            lines_used: self.lines_used,
            in_mapper: self.in_mapper,
            in_fn: self.in_fn,
            next_local: self.next_local,
        }
    }

    fn fresh(&self) -> LocalId {
        let id = self.next_local.get();
        self.next_local.set(id + 1);
        id
    }
}

pub fn check(file: &File) -> Result<tir::Program, Error> {
    let lines_used = Cell::new(false);
    let aliases = alias_map(&file.aliases)?;
    let env = TypeEnv {
        aliases,
        enums: enum_map(&file.enums)?,
    };
    for e in &file.enums {
        if env.aliases.contains_key(&e.name) {
            return Err(Error::new(
                e.span,
                format!("type `{}` is defined twice", e.name),
            ));
        }
    }
    // Resolved eagerly so a broken declaration is an error even when nothing uses it, and so a
    // cycle is found here rather than wherever it happened to be reached from.
    for a in &file.aliases {
        resolve(&a.ty, &env, &mut vec![a.name.clone()])?;
    }
    let mut enums: HashMap<String, Type> = HashMap::new();
    for e in &file.enums {
        enums.insert(e.name.clone(), resolve_enum(e, &env, &mut Vec::new())?);
    }
    let mut variant_owners: HashMap<String, Vec<String>> = HashMap::new();
    for e in &file.enums {
        for v in &e.variants {
            variant_owners
                .entry(v.name.clone())
                .or_default()
                .push(e.name.clone());
        }
    }
    let sigs = signatures(&file.defs, &env)?;
    let input = RefCell::new(None);
    let inputs = RefCell::new(None);
    let next_local = Cell::new(0);

    // Signatures are collected before any body is checked, so a definition may call one that
    // appears later in the file. This is also what recursion will need.
    let mut funcs = Vec::new();
    for def in &file.defs {
        let sig = &sigs[&def.name];
        let ctx = Ctx {
            sigs: &sigs,
            enums: &enums,
            variant_owners: &variant_owners,
            scope: vec![(def.param.name.clone(), sig.param.clone())],
            arm_fields: Vec::new(),
            subject: None,
            input: &input,
            inputs: &inputs,
            lines_used: &lines_used,
            in_mapper: false,
            in_fn: Some(&def.name),
            next_local: &next_local,
        };
        // The declared return type flows into the body, so a form whose type comes from its
        // position (`[]`, `input`, a variant-naming string) resolves against the annotation. A
        // body that synthesises instead is compared below, where the error can name the
        // function rather than just the two types.
        let body = match expect_inner(&ctx, &def.body, &sig.ret)? {
            Expected::Checked(body) | Expected::Synthesised(body) => body,
        };
        // A signature may spell Stream now, which un-does the trick the Lines design leaned on
        // (a return annotation could never match a body holding a stream), so what that trick
        // guaranteed for free is checked for real here.
        if matches!(sig.param, Type::Stream(_)) {
            check_linear(
                &body,
                &StreamBinding::Param(&def.param.name),
                &format!("`{}`", def.param.name),
                def.body.span(),
            )?;
        }
        if body.ty != sig.ret {
            return Err(Error::new(
                def.body.span(),
                format!(
                    "`{}` declares it returns {}, but its body is {}{}",
                    def.name,
                    sig.ret,
                    body.ty,
                    reordered_fields_hint(&sig.ret, &body.ty)
                ),
            ));
        }
        funcs.push(tir::Func {
            name: def.name.clone(),
            param: def.param.name.clone(),
            param_ty: sig.param.clone(),
            body,
        });
    }

    let ctx = Ctx {
        sigs: &sigs,
        enums: &enums,
        variant_owners: &variant_owners,
        scope: Vec::new(),
        arm_fields: Vec::new(),
        subject: None,
        input: &input,
        inputs: &inputs,
        lines_used: &lines_used,
        in_mapper: false,
        in_fn: None,
        next_local: &next_local,
    };
    let body = check_program_body(&ctx, &file.body)?;
    // A stream cannot be printed, having nothing to show: it is not a value, and collect() is
    // what turns it into one. A function body catches this for free, since its return
    // annotation can never spell Stream and so can never match a body that contains one; the
    // program's own result has no annotation to check against, so it needs asking directly.
    if body.ty.contains_stream() {
        return Err(Error::new(
            file.body.span(),
            "the program's result contains a stream, which has nothing to print; pass it to \
             `collect` first"
                .to_string(),
        ));
    }
    let input = input.into_inner();
    let inputs = inputs.into_inner();
    // Forced by the backends, not chosen: Python's `input` reads all of stdin to EOF before
    // parsing, leaving nothing for anything else to read afterward, and jq needs a different
    // invocation flag for raw lines (`-R -n`) than for parsed JSON values (`-n` alone) -- one
    // process cannot run with both. So all three ways of reading the same real stdin are
    // mutually exclusive, not just `input` and `lines` as before.
    if input.is_some() && lines_used.get() {
        return Err(Error::new(
            file.body.span(),
            "a program cannot use both `input` and `lines`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    if input.is_some() && inputs.is_some() {
        return Err(Error::new(
            file.body.span(),
            "a program cannot use both `input` and `inputs`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    if lines_used.get() && inputs.is_some() {
        return Err(Error::new(
            file.body.span(),
            "a program cannot use both `lines` and `inputs`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    Ok(tir::Program {
        funcs: prune_unreachable(funcs, &body),
        body,
        input,
        inputs,
        uses_lines: lines_used.get(),
    })
}

/// The program's own body, one call form special-cased ahead of `synth`: `jsonlines` is a sink,
/// legal only here, as the outermost expression, taking a Vec or a Stream and having no result
/// type at all, since nothing remains that could observe one. The Tir node still carries `Str`
/// -- under eager lowering the emitted expression genuinely is the joined string every backend
/// prints raw -- but no program can see that: `synth` refuses `jsonlines` everywhere else.
fn check_program_body(ctx: &Ctx, body: &Expr) -> Result<Tir, Error> {
    if let Expr::Call { func, arg, .. } = body
        && func == "jsonlines"
    {
        let arg_span = arg.span();
        let arg = synth(ctx, arg)?;
        if !matches!(arg.ty, Type::Vec(_) | Type::Stream(_)) {
            return Err(Error::new(
                arg_span,
                format!("`jsonlines` needs a Vec or a stream, found {}", arg.ty),
            ));
        }
        return Ok(Tir::new(
            Type::Str,
            Kind::Builtin {
                which: tir::Builtin::JsonLines,
                arg: Box::new(arg),
            },
        ));
    }
    synth(ctx, body)
}

/// What a linearity count is looking for: a stream-typed binding, named either by the source
/// (a function parameter) or by the checker (the local a `|` binds `.` to).
enum StreamBinding<'a> {
    Param(&'a str),
    Local(LocalId),
}

/// What broke the exactly-once rule, when a plain count cannot say it.
enum LinearViolation {
    /// A conditional's or match's paths disagree on how often they consume the binding.
    Branches { first: usize, second: usize },
    /// The binding is consumed inside a mapper's body, which runs once per element -- one
    /// spelled consumption, many runtime ones.
    InMapper,
}

/// How many times `t` consumes `binding`, counted along one evaluation path: a conditional
/// runs one branch, so its branches must agree with each other rather than being summed, and
/// a match's arms likewise. A mapper's body runs once per element, so any consumption there
/// is its own violation rather than a count.
fn stream_uses(t: &Tir, binding: &StreamBinding) -> Result<usize, LinearViolation> {
    let both = |a: &Tir, b: &Tir| Ok(stream_uses(a, binding)? + stream_uses(b, binding)?);
    match &t.kind {
        Kind::Var(name) => Ok(match binding {
            StreamBinding::Param(p) => (p == name) as usize,
            StreamBinding::Local(_) => 0,
        }),
        Kind::Local(id) => Ok(match binding {
            StreamBinding::Local(l) => (l == id) as usize,
            StreamBinding::Param(_) => 0,
        }),
        Kind::Str(_) | Kind::Int(_) | Kind::Input | Kind::Inputs | Kind::Lines => Ok(0),
        Kind::VecLit(items) => items
            .iter()
            .try_fold(0, |n, i| Ok(n + stream_uses(i, binding)?)),
        Kind::RecordLit { fields } => fields
            .iter()
            .try_fold(0, |n, (_, v)| Ok(n + stream_uses(v, binding)?)),
        Kind::EnumLit { payload, .. } => payload
            .as_deref()
            .map_or(Ok(0), |p| stream_uses(p, binding)),
        Kind::Call { arg, .. } | Kind::Builtin { arg, .. } => stream_uses(arg, binding),
        Kind::Concat(l, r) => both(l, r),
        Kind::Arith { lhs, rhs, .. } | Kind::Compare { lhs, rhs, .. } => both(lhs, rhs),
        Kind::Bind { value, body, .. } => both(value, body),
        Kind::Map { source, body, .. } => {
            if stream_uses(body, binding)? > 0 {
                return Err(LinearViolation::InMapper);
            }
            stream_uses(source, binding)
        }
        Kind::Select { source, pred, .. } => {
            if stream_uses(pred, binding)? > 0 {
                return Err(LinearViolation::InMapper);
            }
            stream_uses(source, binding)
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => stream_uses(base, binding),
        Kind::Index { base, index, .. } => both(base, index),
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            let t = stream_uses(then, binding)?;
            let o = stream_uses(otherwise, binding)?;
            if t != o {
                return Err(LinearViolation::Branches {
                    first: t,
                    second: o,
                });
            }
            Ok(stream_uses(cond, binding)? + t)
        }
        Kind::Match {
            subject,
            arms,
            partial,
        } => {
            // A path through the chain evaluates every guard up to its arm, then that arm's
            // body; a partial chain has one more path, the fall-through, which evaluates every
            // guard and no body. All of them must agree, exactly as a conditional's branches
            // must.
            let mut guards_so_far = 0;
            let mut counts: Vec<usize> = Vec::new();
            for a in arms {
                if let Some(g) = &a.guard {
                    guards_so_far += stream_uses(g, binding)?;
                }
                counts.push(guards_so_far + stream_uses(&a.body, binding)?);
            }
            if *partial {
                counts.push(guards_so_far);
            }
            if let Some(w) = counts.windows(2).find(|w| w[0] != w[1]) {
                return Err(LinearViolation::Branches {
                    first: w[0],
                    second: w[1],
                });
            }
            Ok(stream_uses(subject, binding)? + counts.first().copied().unwrap_or(0))
        }
    }
}

/// The per-binding half of stream linearity: `binding`, already known to be stream-typed, must
/// be consumed exactly once by `body`. Zero uses is an error too -- linear, not affine: a
/// dropped stream is the Python silent-empty-generator mistake, and exactly-once can relax to
/// at-most-once later without breaking a program, while the reverse tightening could not.
fn check_linear(
    body: &Tir,
    binding: &StreamBinding,
    subject: &str,
    span: Span,
) -> Result<(), Error> {
    match stream_uses(body, binding) {
        Err(LinearViolation::Branches { first, second }) => Err(Error::new(
            span,
            format!(
                "{subject} must be consumed exactly once on every path, but one branch \
                 consumes it {first} times and another {second}"
            ),
        )),
        Err(LinearViolation::InMapper) => Err(Error::new(
            span,
            format!(
                "{subject} must be consumed exactly once, but here it is consumed inside a \
                 mapper body, which runs once per element"
            ),
        )),
        Ok(1) => Ok(()),
        Ok(0) => Err(Error::new(
            span,
            format!("{subject} must be consumed exactly once; it is never consumed"),
        )),
        Ok(n) => Err(Error::new(
            span,
            format!("{subject} must be consumed exactly once, not {n} times"),
        )),
    }
}

/// Every function the program's body can actually reach, directly or through calls a reached
/// function itself makes. `pub fn`s from the prelude are always merged into `file.defs` before
/// this runs, so a `pub` one the program never calls needs pruning here to keep it out of a
/// backend's output and out of `tags::node_types` -- and an unused function the program wrote
/// itself is pruned by the same pass, for the same reason.
fn prune_unreachable(funcs: Vec<tir::Func>, body: &Tir) -> Vec<tir::Func> {
    let mut reached: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    calls_in(body, &mut worklist);
    while let Some(name) = worklist.pop() {
        if reached.insert(name.clone())
            && let Some(f) = funcs.iter().find(|f| f.name == name)
        {
            calls_in(&f.body, &mut worklist);
        }
    }
    funcs
        .into_iter()
        .filter(|f| reached.contains(&f.name))
        .collect()
}

/// Every function name a `Kind::Call` inside `t` names, collected recursively through every
/// other kind of node.
fn calls_in(t: &Tir, out: &mut Vec<String>) {
    if let Kind::Call { func, arg } = &t.kind {
        out.push(func.clone());
        calls_in(arg, out);
        return;
    }
    match &t.kind {
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Var(_)
        | Kind::Local(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines => {}
        Kind::VecLit(items) => items.iter().for_each(|i| calls_in(i, out)),
        Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| calls_in(v, out)),
        Kind::EnumLit { payload, .. } => {
            if let Some(p) = payload {
                calls_in(p, out);
            }
        }
        Kind::Call { .. } => unreachable!("handled above"),
        Kind::Concat(l, r) => {
            calls_in(l, out);
            calls_in(r, out);
        }
        Kind::Arith { lhs, rhs, .. } | Kind::Compare { lhs, rhs, .. } => {
            calls_in(lhs, out);
            calls_in(rhs, out);
        }
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            calls_in(cond, out);
            calls_in(then, out);
            calls_in(otherwise, out);
        }
        Kind::Bind { value, body, .. }
        | Kind::Map {
            source: value,
            body,
            ..
        } => {
            calls_in(value, out);
            calls_in(body, out);
        }
        Kind::Select { source, pred, .. } => {
            calls_in(source, out);
            calls_in(pred, out);
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => calls_in(base, out),
        Kind::Builtin { arg, .. } => calls_in(arg, out),
        Kind::Index { base, index, .. } => {
            calls_in(base, out);
            calls_in(index, out);
        }
        Kind::Match { subject, arms, .. } => {
            calls_in(subject, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    calls_in(g, out);
                }
                calls_in(&a.body, out);
            }
        }
    }
}

/// Every function name the language itself provides, and therefore reserves. `str` and `range`
/// live in `builtin()`'s fixed table; `jsonlines`, `extent`, `concat`, `tail`, and `collect` are
/// polymorphic and checked from `synth`'s own arms; `select` and `map` rebind `.`. All nine are
/// reserved the same way, and the docs harness (tests/docs.rs) reads this list to insist each
/// one has a reference page.
pub const BUILTIN_NAMES: [&str; 9] = [
    "collect",
    "concat",
    "extent",
    "jsonlines",
    "map",
    "range",
    "select",
    "str",
    "tail",
];

/// Functions the language provides. Unary like every other function, so they need no special
/// call syntax and are looked up before user definitions.
fn builtin(name: &str) -> Option<(tir::Builtin, Sig)> {
    let vec_of = |t: Type| Type::Vec(Box::new(t));
    Some(match name {
        "str" => (
            tir::Builtin::IntToStr,
            Sig {
                param: Type::Int,
                ret: Type::Str,
            },
        ),
        "range" => (
            tir::Builtin::Range,
            Sig {
                param: Type::Int,
                ret: vec_of(Type::Int),
            },
        ),
        _ => return None,
    })
}

/// Capitalised names are types and lowercase names are values.
///
/// Every name in the language already followed this, unenforced. Making it a rule is what lets a
/// constructor share a spelling with a call without sharing a namespace: `User {..}` builds and
/// `area {..}` calls, decided by the first letter rather than by looking either one up.
///
/// Field names are deliberately not covered. They come from data, and JSON objects are entitled
/// to a key spelled `Name`.
fn value_name(name: &str, span: Span, what: &str) -> Result<(), Error> {
    if name.chars().next().is_some_and(char::is_uppercase) {
        return Err(Error::new(
            span,
            format!("a {what} starts with a lowercase letter, and `{name}` reads as a type"),
        ));
    }
    Ok(())
}

/// What a type name stands for. An alias is an abbreviation, so this maps to the written form
/// and `resolve` expands it; nothing downstream ever learns a name was involved.
type Aliases<'a> = HashMap<String, &'a TypeExpr>;

/// The named types a written annotation can refer to. Aliases expand away; an enum resolves to
/// the identity its declaration created.
struct TypeEnv<'a> {
    aliases: Aliases<'a>,
    enums: HashMap<String, &'a EnumDecl>,
}

fn enum_map(enums: &[EnumDecl]) -> Result<HashMap<String, &EnumDecl>, Error> {
    let mut map: HashMap<String, &EnumDecl> = HashMap::new();
    for e in enums {
        if !e.name.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                e.span,
                format!(
                    "a type name starts with a capital letter, and `{}` reads as a value",
                    e.name
                ),
            ));
        }
        if ty::is_builtin_type_name(&e.name) {
            return Err(Error::new(
                e.span,
                format!("`{}` is a built-in type and cannot be redefined", e.name),
            ));
        }
        for (i, v) in e.variants.iter().enumerate() {
            if e.variants[..i].iter().any(|earlier| earlier.name == v.name) {
                return Err(Error::new(
                    v.span,
                    format!("variant `{}` is declared twice in `{}`", v.name, e.name),
                ));
            }
        }
        if map.insert(e.name.clone(), e).is_some() {
            return Err(Error::new(
                e.span,
                format!("type `{}` is defined twice", e.name),
            ));
        }
    }
    Ok(map)
}

/// Resolve one enum declaration to its type. `seen` carries the names being expanded, exactly
/// as for aliases, so an enum whose payload mentions itself is refused rather than expanded
/// forever -- there is no indirection for a recursive payload to hide behind yet.
fn resolve_enum(decl: &EnumDecl, env: &TypeEnv, seen: &mut Vec<String>) -> Result<Type, Error> {
    seen.push(decl.name.clone());
    let mut variants = Vec::new();
    for v in &decl.variants {
        let payload = match &v.payload {
            Some(ty) => {
                let resolved = resolve(ty, env, seen)?;
                // The record and Vec spellings refuse a stream inside themselves, but the
                // parens spelling can put one directly in payload position, so the ban has to
                // be stated here too: an enum value is storable, and a stream is not.
                if resolved.contains_stream() {
                    return Err(Error::new(
                        ty.span(),
                        format!(
                            "the payload of `{}` cannot hold a stream, which has nothing to store",
                            v.name
                        ),
                    ));
                }
                Some(resolved)
            }
            None => None,
        };
        variants.push((v.name.clone(), payload));
    }
    seen.pop();
    Ok(Type::Enum {
        name: decl.name.clone(),
        variants,
    })
}

fn alias_map(aliases: &[Alias]) -> Result<Aliases<'_>, Error> {
    let mut map: Aliases = HashMap::new();
    for a in aliases {
        if !a.name.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                a.span,
                format!(
                    "a type name starts with a capital letter, and `{}` reads as a value",
                    a.name
                ),
            ));
        }
        if ty::is_builtin_type_name(&a.name) {
            return Err(Error::new(
                a.span,
                format!("`{}` is a built-in type and cannot be redefined", a.name),
            ));
        }
        if map.insert(a.name.clone(), &a.ty).is_some() {
            return Err(Error::new(
                a.span,
                format!("type `{}` is defined twice", a.name),
            ));
        }
    }
    Ok(map)
}

fn signatures(defs: &[Def], env: &TypeEnv) -> Result<HashMap<String, Sig>, Error> {
    let mut sigs = HashMap::new();
    for def in defs {
        value_name(&def.name, def.span, "function name")?;
        value_name(&def.param.name, def.param.span, "parameter name")?;
        if BUILTIN_NAMES.contains(&def.name.as_str()) {
            return Err(Error::new(
                def.span,
                format!("`{}` is a builtin and cannot be redefined", def.name),
            ));
        }
        if sigs.contains_key(&def.name) {
            return Err(Error::new(
                def.span,
                format!("`{}` is defined twice", def.name),
            ));
        }
        let sig = Sig {
            param: resolve(&def.param.ty, env, &mut Vec::new())?,
            ret: resolve(&def.ret, env, &mut Vec::new())?,
        };
        // A stream is born only at a source, so a function cannot conjure one: a stream result
        // flows in through a stream parameter, and the pipeline stays one chain fusion can
        // read. Refusing is the reversible direction.
        if matches!(sig.ret, Type::Stream(_)) && !matches!(sig.param, Type::Stream(_)) {
            return Err(Error::new(
                def.span,
                format!(
                    "`{}` returns {} without taking a stream; a stream is born only at a source",
                    def.name, sig.ret
                ),
            ));
        }
        sigs.insert(def.name.clone(), sig);
    }
    Ok(sigs)
}

/// `seen` is the chain of names currently being expanded -- aliases and enums share it -- so a
/// type written in terms of itself is caught rather than expanded forever.
fn resolve(ty: &TypeExpr, env: &TypeEnv, seen: &mut Vec<String>) -> Result<Type, Error> {
    match ty {
        TypeExpr::Named { name, span } => {
            if let Some(built_in) = Type::from_name(name) {
                return Ok(built_in);
            }
            if let Some(at) = seen.iter().position(|s| s == name) {
                // The names expanded since this one last appeared are the cycle, and naming them
                // is the difference between knowing there is one and finding it.
                let through: Vec<String> =
                    seen[at + 1..].iter().map(|s| format!("`{s}`")).collect();
                let path = if through.is_empty() {
                    String::new()
                } else {
                    format!(", through {}", through.join(" and "))
                };
                return Err(Error::new(
                    *span,
                    format!("type `{name}` is written in terms of itself{path}"),
                ));
            }
            if let Some(written) = env.aliases.get(name) {
                seen.push(name.clone());
                let expanded = resolve(written, env, seen)?;
                seen.pop();
                return Ok(expanded);
            }
            if let Some(decl) = env.enums.get(name) {
                return resolve_enum(decl, env, seen);
            }
            Err(Error::new(*span, format!("unknown type `{name}`")))
        }
        // The containment bans hold in the grammar itself, not just at value construction
        // sites: a stream is not a value, so no annotation may describe one as stored.
        TypeExpr::Vec { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Vec cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            Ok(Type::Vec(Box::new(inner)))
        }
        TypeExpr::Stream { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Stream cannot hold another stream; there is nothing it could yield"
                        .to_string(),
                ));
            }
            Ok(Type::Stream(Box::new(inner)))
        }
        TypeExpr::Opt { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "an Opt cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            Ok(Type::Opt(Box::new(inner)))
        }
        TypeExpr::Record { fields, span } => {
            let mut out = Vec::new();
            for (name, ty) in fields {
                if out.iter().any(|(n, _): &(String, Type)| n == name) {
                    return Err(Error::new(
                        *span,
                        format!("field `{name}` is declared twice"),
                    ));
                }
                let field = resolve(ty, env, seen)?;
                if field.contains_stream() {
                    return Err(Error::new(
                        ty.span(),
                        format!("`{name}` cannot hold a stream, which has nothing to store"),
                    ));
                }
                out.push((name.clone(), field));
            }
            Ok(Type::Record(out))
        }
    }
}

/// A chain split by `rebase`: the rebased body, and the stream source it was split from.
struct Rebased {
    body: Tir,
    source: Tir,
}

/// Split an access chain whose outermost dimension is a stream from its source, rebasing the
/// chain on `param` as a map body: every node's type loses its Stream wrapper, and an Index's
/// stored depth drops by one, since the layer it counted is now the loop.
///
/// Only Field, Unwrap, and Index appear in a chain, and the source is never one of them: a
/// stream-typed access chain is normalized to a Map right here, so one can never be the base of
/// another.
fn rebase(t: Tir, param: LocalId) -> Rebased {
    fn peel(ty: Type) -> Type {
        match ty {
            Type::Stream(t) => *t,
            other => unreachable!(
                "every node inside the stream dimension is stream-typed, found {other}"
            ),
        }
    }
    fn is_chain(t: &Tir) -> bool {
        matches!(
            t.kind,
            Kind::Field { .. } | Kind::Unwrap { .. } | Kind::Index { .. }
        )
    }
    /// The base of one link in the chain: another link to recurse into, or the stream source
    /// itself, in which case `param` stands in for the per-element value the map body reads.
    fn split_base(base: Tir, param: LocalId) -> Rebased {
        if is_chain(&base) {
            rebase(base, param)
        } else {
            let elem = peel(base.ty.clone());
            Rebased {
                body: Tir::new(elem, Kind::Local(param)),
                source: base,
            }
        }
    }
    match t.kind {
        Kind::Field { base, name } => {
            let Rebased { body: base, source } = split_base(*base, param);
            Rebased {
                body: Tir::new(
                    peel(t.ty),
                    Kind::Field {
                        base: Box::new(base),
                        name,
                    },
                ),
                source,
            }
        }
        Kind::Unwrap { base } => {
            let Rebased { body: base, source } = split_base(*base, param);
            Rebased {
                body: Tir::new(
                    peel(t.ty),
                    Kind::Unwrap {
                        base: Box::new(base),
                    },
                ),
                source,
            }
        }
        Kind::Index {
            base,
            index,
            depth,
            elem_is_record,
        } => {
            let Rebased { body: base, source } = split_base(*base, param);
            let kind = Kind::Index {
                base: Box::new(base),
                index,
                depth: depth - 1,
                elem_is_record,
            };
            Rebased {
                body: Tir::new(peel(t.ty), kind),
                source,
            }
        }
        other => unreachable!(
            "only an access chain is rebased, found {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

/// The element `select` and `map` rebind `.` to: a Vec's, or a Stream's. Deliberately not
/// `Type::elem`, which stays Vec-only so the reducers (`extent`, `jsonlines` today) keep
/// refusing a stream.
fn mapper_elem(subject: &Type) -> Option<Type> {
    match subject {
        Type::Vec(t) | Type::Stream(t) => Some((**t).clone()),
        _ => None,
    }
}

/// The context a mapper body (`select`'s predicate, `map`'s body) checks in: `.` rebound to the
/// element a fresh `param` will hold at runtime, with a source read refused since the body runs
/// once per element rather than once.
fn mapper_ctx<'a>(ctx: &'a Ctx, elem: Type, param: LocalId) -> Ctx<'a> {
    let mut inner = ctx.with(Some((elem, param)));
    inner.in_mapper = true;
    inner
}

/// The one enum a bare variant name refers to, or the error naming every candidate: guessing
/// between two claimants would silently pick a type the program never wrote down.
fn sole_owner<'a>(
    ctx: &'a Ctx,
    variant: &str,
    owners: &[String],
    span: Span,
) -> Result<&'a Type, Error> {
    if owners.len() > 1 {
        let named: Vec<String> = owners.iter().map(|e| format!("`{e}`")).collect();
        let qualified: Vec<String> = owners.iter().map(|e| format!("`{e}.{variant}`")).collect();
        return Err(Error::new(
            span,
            format!(
                "`{variant}` is a variant of {}; qualify it as {}",
                named.join(" and "),
                qualified.join(" or ")
            ),
        ));
    }
    Ok(&ctx.enums[&owners[0]])
}

/// Build one variant of `enum_ty`, checking that the payload written matches the payload
/// declared: a unit variant takes none, a payload variant requires one.
fn construct(
    ctx: &Ctx,
    enum_ty: &Type,
    variant: &str,
    variant_span: Span,
    payload: Option<&Expr>,
) -> Result<Tir, Error> {
    let Type::Enum { name, variants } = enum_ty else {
        unreachable!("construct is only called with an enum type")
    };
    let Some((_, declared)) = variants.iter().find(|(n, _)| n == variant) else {
        return Err(Error::new(
            variant_span,
            format!("`{name}` has no variant `{variant}`"),
        ));
    };
    let payload = match (declared, payload) {
        (None, None) => None,
        (Some(want), Some(expr)) => Some(Box::new(expect(ctx, expr, want)?)),
        (None, Some(expr)) => {
            return Err(Error::new(
                expr.span(),
                format!("`{variant}` is a unit variant of `{name}` and takes no payload"),
            ));
        }
        (Some(want), None) => {
            // The hint mirrors the declaration's two spellings: braces for a record payload,
            // parens for any other type.
            let spelled = match want {
                Type::Record(_) => format!("{variant}{{...}}"),
                _ => format!("{variant}(...)"),
            };
            return Err(Error::new(
                variant_span,
                format!(
                    "`{variant}` of `{name}` carries a payload of {want}, so it is written `{spelled}`"
                ),
            ));
        }
    };
    Ok(Tir::new(
        enum_ty.clone(),
        Kind::EnumLit {
            variant: variant.to_string(),
            payload,
        },
    ))
}

/// A source spelled inside a `fn` body. The rule exists because the mapper-body check alone had
/// a cross-function hole: a function reading a source, called from a mapper's body, re-read
/// stdin once per element with nothing at the call site to refuse. Refusing sources in every
/// `fn` closes that everywhere; the alternative, tracking source consumption per function
/// transitively, was rejected as an invisible effect on every signature.
fn source_in_fn(span: Span, source: &str, func: &str) -> Error {
    Error::new(
        span,
        format!(
            "`{source}` cannot be read inside `fn {func}`; a source is legal only in the \
             program's own body, so take the stream through a `Stream` parameter"
        ),
    )
}

/// The one exit a stream has: `Stream<T> -> Vec<T>`. `want_elem` is `Some` in the checking
/// direction, needed because `collect(inputs)` has no type to synthesise until the wanted
/// Vec<T> pushes a Stream<T> expectation down onto `inputs`; it is `None` in the synthesising
/// direction, where the argument's own type says what T is.
fn collect(ctx: &Ctx, arg: &Expr, want_elem: Option<&Type>) -> Result<Tir, Error> {
    let (elem, arg) = match want_elem {
        Some(elem) => (
            elem.clone(),
            expect(ctx, arg, &Type::Stream(Box::new(elem.clone())))?,
        ),
        None => {
            let arg_span = arg.span();
            let arg = synth(ctx, arg)?;
            let Type::Stream(elem) = &arg.ty else {
                return Err(Error::new(
                    arg_span,
                    format!("`collect` needs a stream, found {}", arg.ty),
                ));
            };
            (elem.as_ref().clone(), arg)
        }
    };
    Ok(Tir::new(
        Type::Vec(Box::new(elem)),
        Kind::Builtin {
            which: tir::Builtin::Collect,
            arg: Box::new(arg),
        },
    ))
}

/// A `map` call over the subject. `want_elem` is the element type the position expects of the
/// result, when it expects one whose cardinality matches the subject's; the body is checked
/// against it, which is what lets a mapper body hold `[]`, a variant-naming string, or the
/// other checked-only forms. With `None` the body synthesises, as it always has.
fn map_call(ctx: &Ctx, arg: &Expr, span: Span, want_elem: Option<&Type>) -> Result<Tir, Error> {
    let Some((subject, id)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "`map` needs a subject, so it must follow `|`",
        ));
    };
    let Some(elem) = mapper_elem(&subject) else {
        return Err(Error::new(
            span,
            format!("`map` needs a Vec or a stream, found {subject}"),
        ));
    };
    let param = ctx.fresh();
    let inner = mapper_ctx(ctx, elem, param);
    let body = match want_elem {
        Some(want) => expect(&inner, arg, want)?,
        None => synth(&inner, arg)?,
    };
    // The elements of the result are stored, and a stream is not storable: this is the same
    // containment ban a Vec literal enforces, met here before the Vec or Stream of them could
    // exist.
    if body.ty.contains_stream() {
        return Err(Error::new(
            arg.span(),
            "a `map` body cannot be a stream, which has nothing to store".to_string(),
        ));
    }
    let out = match &subject {
        Type::Stream(_) => Type::Stream(Box::new(body.ty.clone())),
        _ => Type::Vec(Box::new(body.ty.clone())),
    };
    let source = Tir::new(subject, Kind::Local(id));
    Ok(Tir::new(
        out,
        Kind::Map {
            source: Box::new(source),
            param,
            body: Box::new(body),
        },
    ))
}

/// `|` binds `.` in the right side to the value of the left. It is composition, not a map:
/// the operators that distribute over a Vec do so themselves. The pipe's value is the right
/// side's, so an expectation flows into the right side whole -- which is how one reaches the
/// subject-fed forms (`map`, `select`, a match chain), since they only exist after `|`.
fn pipe(ctx: &Ctx, lhs: &Expr, rhs: &Expr, want: Option<&Type>) -> Result<Expected, Error> {
    let value = synth(ctx, lhs)?;
    let local = ctx.fresh();
    let inner = ctx.with(Some((value.ty.clone(), local)));
    let body = match want {
        Some(want) => expect_inner(&inner, rhs, want)?,
        None => Expected::Synthesised(synth(&inner, rhs)?),
    };
    let (checked, body) = match body {
        Expected::Checked(tir) => (true, tir),
        Expected::Synthesised(tir) => (false, tir),
    };
    // A stream on the right of `|` must have flowed in from the left: a source read beside an
    // unrelated piped value is not one chain, and one chain from source to sink is the shape
    // the whole effect layer keeps.
    if matches!(body.ty, Type::Stream(_)) && !matches!(value.ty, Type::Stream(_)) {
        return Err(Error::new(
            rhs.span(),
            "this stream does not flow in from the left of `|`; write the pipeline as \
             one chain from its source"
                .to_string(),
        ));
    }
    // The same linearity a stream-typed parameter gets: `|` is the one construct that can
    // silently drop its left side, so a stream piped in must be consumed here.
    if matches!(value.ty, Type::Stream(_)) {
        check_linear(
            &body,
            &StreamBinding::Local(local),
            "the stream piped in here",
            rhs.span(),
        )?;
    }
    let tir = Tir::new(
        body.ty.clone(),
        Kind::Bind {
            local,
            value: Box::new(value),
            body: Box::new(body),
        },
    );
    Ok(if checked {
        Expected::Checked(tir)
    } else {
        Expected::Synthesised(tir)
    })
}

/// The hybrid totality the match-arms decision fixed. A chain with variant patterns is
/// closed-world: the subject is a declared enum, and the arms are proved to cover every
/// variant (or end in a default); guards do not count toward that coverage. A pure guard
/// chain is open-world and may be honestly partial, yielding `Opt` -- except when the arm
/// bodies are themselves `Opt`-typed, refused below. `.` narrows per arm: to the payload in
/// a payload arm, to nothing in a unit arm, and it stays the subject in a guard or default
/// arm.
///
/// `body_want` is what every arm's body must be -- for a partial chain, the expectation with
/// its Opt layer already peeled by the caller. With `None` the first arm's body decides, as
/// it always has.
/// The dead-arm rule, both ways a chain can already be finished: a default above matches
/// everything, and arms covering every variant leave nothing for a later arm to see. (The
/// second is also what lets every backend take a total chain's last arm without a test.)
fn check_reachable(
    subject_ty: &Type,
    covered: &[String],
    default_seen: bool,
    arm_span: Span,
) -> Result<(), Error> {
    if default_seen {
        return Err(Error::new(
            arm_span,
            "this arm can never match; the `any()` arm above it already matches everything"
                .to_string(),
        ));
    }
    if let Type::Enum { name, variants } = subject_ty
        && !covered.is_empty()
        && variants.iter().all(|(n, _)| covered.contains(n))
    {
        return Err(Error::new(
            arm_span,
            format!(
                "this arm can never match; the arms above it already cover every variant of `{name}`"
            ),
        ));
    }
    Ok(())
}

/// The closed-world proof: every variant of the subject's enum is either covered by a pattern
/// arm or the chain ends in a default, which the caller has already ruled out.
fn check_coverage(subject_ty: &Type, covered: &[String], span: Span) -> Result<(), Error> {
    let Type::Enum {
        name: enum_name,
        variants,
    } = subject_ty
    else {
        unreachable!("a pattern arm was checked against an enum subject")
    };
    let missing: Vec<String> = variants
        .iter()
        .filter(|(n, _)| !covered.contains(n))
        .map(|(n, _)| format!("`{n}`"))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(Error::new(
        span,
        format!(
            "a match over `{enum_name}` must cover every variant or end in a default; missing {}",
            missing.join(" and ")
        ),
    ))
}

/// One variant pattern against the subject: the variant must exist on the subject's enum, a
/// unit variant takes no field pattern, and a payload variant rebinds `.` to a fresh payload
/// local, with any destructured fields proved against the payload's record type. Returns the
/// payload local (`None` for a unit variant) and the context the arm's body checks in.
fn variant_arm<'a>(
    ctx: &'a Ctx,
    subject_ty: &Type,
    vname: &str,
    vspan: Span,
    fields: Option<&FieldsPattern>,
) -> Result<(Option<LocalId>, Ctx<'a>), Error> {
    let Type::Enum {
        name: enum_name,
        variants,
    } = subject_ty
    else {
        return Err(Error::new(
            vspan,
            format!("a match needs an enum subject, found {subject_ty}"),
        ));
    };
    let Some((_, payload_ty)) = variants.iter().find(|(n, _)| n == vname) else {
        return Err(Error::new(
            vspan,
            format!("`{enum_name}` has no variant `{vname}`"),
        ));
    };
    match payload_ty {
        None => {
            if let Some(f) = fields {
                return Err(Error::new(
                    f.span,
                    format!(
                        "`{vname}` is a unit variant of `{enum_name}` and has no payload to destructure"
                    ),
                ));
            }
            Ok((None, ctx.with(None)))
        }
        Some(pty) => {
            let pid = ctx.fresh();
            let mut arm_ctx = ctx.with(Some((pty.clone(), pid)));
            if let Some(f) = fields {
                let Type::Record(pfields) = pty else {
                    return Err(Error::new(
                        f.span,
                        format!(
                            "the payload of `{vname}` is {pty}, not a record, so there are no fields to destructure; the arm's `.` is the payload"
                        ),
                    ));
                };
                for (i, (fname, fspan)) in f.names.iter().enumerate() {
                    if f.names[..i].iter().any(|(seen, _)| seen == fname) {
                        return Err(Error::new(
                            *fspan,
                            format!("`{fname}` is bound twice in this pattern"),
                        ));
                    }
                    if !pfields.iter().any(|(n, _)| n == fname) {
                        return Err(Error::new(*fspan, format!("no field `{fname}` on {pty}")));
                    }
                    arm_ctx.arm_fields.push((fname.clone(), pty.clone(), pid));
                }
                // Leaving fields out of a match against a closed type is a forgotten field
                // until `..` says it was meant.
                if !f.rest {
                    let missing: Vec<String> = pfields
                        .iter()
                        .filter(|(n, _)| !f.names.iter().any(|(m, _)| m == n))
                        .map(|(n, _)| format!("`{n}`"))
                        .collect();
                    if !missing.is_empty() {
                        return Err(Error::new(
                            f.span,
                            format!(
                                "a `{vname}` pattern must name every payload field or end in `..`; missing {}",
                                missing.join(" and ")
                            ),
                        ));
                    }
                }
            }
            Ok((Some(pid), arm_ctx))
        }
    }
}

fn match_chain(
    ctx: &Ctx,
    arms: &[MatchArm],
    span: Span,
    body_want: Option<&Type>,
) -> Result<Tir, Error> {
    let Some((subject_ty, sid)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "a match needs a subject, so it must follow `|`".to_string(),
        ));
    };
    let mut covered: Vec<String> = Vec::new();
    let mut default_seen = false;
    let mut has_pattern_arm = false;
    let mut result: Option<Type> = body_want.cloned();
    let mut out = Vec::new();
    for arm in arms {
        check_reachable(&subject_ty, &covered, default_seen, arm.span)?;
        let (variant, guard, payload, arm_ctx) = match &arm.pattern {
            Pattern::Default { .. } => {
                default_seen = true;
                // A default matched the whole subject, so `.` stays the enum value.
                (None, None, None, ctx.with(ctx.subject.clone()))
            }
            // The guard is a Bool over the unrebound subject, so it is checked in the
            // enclosing context, and the body keeps `.` as the subject too.
            Pattern::Guard(g) => {
                let cond = expect(ctx, g, &Type::Bool)?;
                (None, Some(cond), None, ctx.with(ctx.subject.clone()))
            }
            Pattern::Variant {
                name: vname,
                span: vspan,
                fields,
            } => {
                has_pattern_arm = true;
                covered.push(vname.clone());
                let (payload, arm_ctx) =
                    variant_arm(ctx, &subject_ty, vname, *vspan, fields.as_ref())?;
                (Some(vname.clone()), None, payload, arm_ctx)
            }
        };
        let body = match &result {
            None => {
                let body = synth(&arm_ctx, &arm.body)?;
                // The same rule a conditional has: which arm runs is decided at
                // runtime, and a pipeline's shape must not be.
                if body.ty.contains_stream() {
                    return Err(Error::new(
                        arm.body.span(),
                        "a match cannot yield a stream; pass each arm to `collect` first"
                            .to_string(),
                    ));
                }
                result = Some(body.ty.clone());
                body
            }
            Some(t) => expect(&arm_ctx, &arm.body, t)?,
        };
        out.push(tir::MatchArm {
            variant,
            guard,
            payload,
            body,
        });
    }
    // Closed-world half: a chain with variant patterns keeps exhaustiveness, and only
    // patterns count toward it -- a guard is a runtime Bool the checker cannot see
    // through.
    if has_pattern_arm && !default_seen {
        check_coverage(&subject_ty, &covered, span)?;
    }
    let result = result.expect("the parser produces at least one arm");
    // Open-world half: a pure guard chain with no default may decline every arm, so
    // its result is `Opt` -- unless the bodies are already `Opt`-typed, where one
    // `null` would mean both "no arm matched" and "matched, and found nothing" (our
    // `Opt` is untagged), the exact conflation the field-access rule forbids.
    let partial = !has_pattern_arm && !default_seen;
    let result = if partial {
        if matches!(result, Type::Opt(_)) {
            return Err(Error::new(
                span,
                format!(
                    "this guard chain is partial, and its arms are already {result}: \
                         a declined chain and a matched-but-absent value would both print \
                         `null`; add a default arm"
                ),
            ));
        }
        Type::Opt(Box::new(result))
    } else {
        result
    };
    let subject = Tir::new(subject_ty.clone(), Kind::Local(sid));
    Ok(Tir::new(
        result,
        Kind::Match {
            subject: Box::new(subject),
            arms: out,
            partial,
        },
    ))
}

fn synth(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    match expr {
        Expr::Str { text, .. } => Ok(Tir::new(Type::Str, Kind::Str(text.clone()))),
        Expr::Lines { span } => {
            if let Some(func) = ctx.in_fn {
                return Err(source_in_fn(*span, "lines", func));
            }
            if ctx.in_mapper {
                return Err(Error::new(
                    *span,
                    "`lines` cannot be read inside a mapper body, which runs once per element"
                        .to_string(),
                ));
            }
            if ctx.lines_used.get() {
                return Err(Error::new(
                    *span,
                    "`lines` has already been read; there is only one stdin".to_string(),
                ));
            }
            ctx.lines_used.set(true);
            Ok(Tir::new(Type::Stream(Box::new(Type::Str)), Kind::Lines))
        }
        Expr::Int { value, span } => {
            // The literal is the one place a value could enter without meeting the 32-bit rule,
            // and four backends agreed on the wrong answer only because each held it in its own
            // wider representation until an operator wrapped it. Go refuses to compile such a
            // constant at all, which is what made the hole visible.
            if *value > i32::MAX as i64 {
                return Err(Error::new(
                    *span,
                    format!("integer `{value}` does not fit in Int, which is 32 bits"),
                ));
            }
            Ok(Tir::new(Type::Int, Kind::Int(*value)))
        }

        Expr::Subject { span } => match &ctx.subject {
            Some((ty, id)) => Ok(Tir::new(ty.clone(), Kind::Local(*id))),
            None => Err(Error::new(*span, "`.` is not bound here")),
        },

        // Innermost binding first: a pattern binding shadows a parameter, and a parameter
        // shadows a variant, the way any binding shadows a constant.
        Expr::Var { name, span } => {
            if let Some((_, payload_ty, pid)) =
                ctx.arm_fields.iter().rev().find(|(n, _, _)| n == name)
            {
                let fty = payload_ty
                    .field(name)
                    .expect("the field existed when the pattern was checked")
                    .clone();
                let base = Tir::new(payload_ty.clone(), Kind::Local(*pid));
                return Ok(Tir::new(
                    fty,
                    Kind::Field {
                        base: Box::new(base),
                        name: name.clone(),
                    },
                ));
            }
            if let Some((_, t)) = ctx.scope.iter().find(|(n, _)| n == name) {
                return Ok(Tir::new(t.clone(), Kind::Var(name.clone())));
            }
            if let Some(owners) = ctx.variant_owners.get(name) {
                let enum_ty = sole_owner(ctx, name, owners, *span)?.clone();
                return construct(ctx, &enum_ty, name, *span, None);
            }
            // A function is not a value, but "`f` is not defined" for a defined function is a
            // lie. This is where `f -1` lands: `-` cannot start a bare argument, so the parse
            // is subtraction and `f` arrives here alone.
            if ctx.sigs.contains_key(name) || BUILTIN_NAMES.contains(&name.as_str()) {
                return Err(Error::new(
                    *span,
                    format!("`{name}` is a function, not a value; write `{name}(...)` to call it"),
                ));
            }
            Err(Error::new(*span, format!("`{name}` is not defined")))
        }

        Expr::RecordLit { fields, .. } => {
            let mut built: Vec<(String, Tir)> = Vec::new();
            for (name, name_span, value) in fields {
                if built.iter().any(|(seen, _)| seen == name) {
                    return Err(Error::new(
                        *name_span,
                        format!("field `{name}` is given twice"),
                    ));
                }
                let field = synth(ctx, value)?;
                // A stream can never enter a record: a record's fields can be printed, copied,
                // and read back out by name, none of which make sense for a single-use stream,
                // and no path exists for a stream-typed field to leave one again once inside.
                if field.ty.contains_stream() {
                    return Err(Error::new(
                        value.span(),
                        format!("`{name}` cannot hold a stream, which has nothing to store"),
                    ));
                }
                built.push((name.clone(), field));
            }
            // Declaration order, kept as written: the type's field list and this literal's
            // fields stay index-for-index aligned, which is what the columnar backends key on.
            let ty = Type::Record(
                built
                    .iter()
                    .map(|(n, t)| (n.clone(), t.ty.clone()))
                    .collect(),
            );
            Ok(Tir::new(ty, Kind::RecordLit { fields: built }))
        }

        Expr::VecLit { items, span } => {
            let Some(first) = items.first() else {
                // Nothing says what an empty literal contains, and there is no expected type to
                // supply it. Guessing here is what the annotation rule exists to avoid.
                return Err(Error::new(*span, "cannot tell what `[]` contains"));
            };
            let head = synth(ctx, first)?;
            let elem = head.ty.clone();
            // Same reasoning as a record field: nothing can get a stream back out of a Vec
            // once it is in one, and a Vec of them makes no sense when there is only ever one
            // real stdin to begin with.
            if elem.contains_stream() {
                return Err(Error::new(
                    first.span(),
                    "a Vec cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            let mut out = vec![head];
            for item in &items[1..] {
                out.push(expect(ctx, item, &elem)?);
            }
            Ok(Tir::new(Type::Vec(Box::new(elem)), Kind::VecLit(out)))
        }

        Expr::Pipe { lhs, rhs, .. } => match pipe(ctx, lhs, rhs, None)? {
            Expected::Checked(tir) | Expected::Synthesised(tir) => Ok(tir),
        },

        Expr::Field { .. } | Expr::Index { .. } | Expr::Unwrap { .. } => {
            let Access { tir, stream, .. } = access(ctx, expr)?;
            // Projection over a stream is a mapper, and it is normalized to one here: the chain
            // is rebased onto a fresh per-element param, so neither the backends nor fusion
            // ever see a Field, Index, or Unwrap whose base is stream-typed.
            if stream {
                let param = ctx.fresh();
                let Rebased { body, source } = rebase(tir, param);
                return Ok(Tir::new(
                    Type::Stream(Box::new(body.ty.clone())),
                    Kind::Map {
                        source: Box::new(source),
                        param,
                        body: Box::new(body),
                    },
                ));
            }
            Ok(tir)
        }

        // `v[]` with nothing after it is the identity: "keep every entry" is what a Vec (or
        // stream) already is, with no field/index/unwrap left to distribute across it.
        Expr::Project { .. } => {
            let access = access(ctx, expr)?;
            Ok(access.tir)
        }

        // `input` names one value, so its first checked use fixes the type for the whole
        // program -- and a later use in a position that expects nothing can borrow what the
        // first use fixed. With no use typed yet, nothing says what it contains, and guessing
        // is what the annotation rule avoids.
        Expr::Input { span } => match ctx.input.borrow().as_ref() {
            Some(ty) => Ok(Tir::new(ty.clone(), Kind::Input)),
            None => Err(Error::new(*span, "cannot tell what `input` contains")),
        },
        Expr::Inputs { span } => match ctx.in_fn {
            Some(func) => Err(source_in_fn(*span, "inputs", func)),
            None => Err(Error::new(*span, "cannot tell what `inputs` contains")),
        },

        Expr::Call {
            func,
            func_span,
            arg,
            span,
        } => {
            // `select` and `map` are not special syntax, only special names: they are ordinary
            // calls whose argument is checked with `.` rebound to the subject's element type
            // instead of evaluated in the enclosing scope, which no ordinary function needs and
            // is why they cannot be defined as one (see `signatures`).
            // Cardinality-polymorphic: the same subject-context mechanism types them over a
            // Vec and over a Stream, with the element drawn from either's parameter. Stream in,
            // stream out.
            if func == "select" {
                let Some((subject, id)) = ctx.subject.clone() else {
                    return Err(Error::new(
                        *span,
                        "`select` needs a subject, so it must follow `|`",
                    ));
                };
                let Some(elem) = mapper_elem(&subject) else {
                    return Err(Error::new(
                        *span,
                        format!("`select` needs a Vec or a stream, found {subject}"),
                    ));
                };
                let param = ctx.fresh();
                let inner = mapper_ctx(ctx, elem, param);
                let pred = expect(&inner, arg, &Type::Bool)?;
                let source = Tir::new(subject.clone(), Kind::Local(id));
                return Ok(Tir::new(
                    subject,
                    Kind::Select {
                        source: Box::new(source),
                        param,
                        pred: Box::new(pred),
                    },
                ));
            }
            // The one way to produce a new element value. `select` removes elements and a field
            // access reads a field; neither can turn a Vec<Int> into a Vec<Str>.
            if func == "map" {
                return map_call(ctx, arg, *span, None);
            }
            // A sink, not a function: `check` handles the one legal position (the program's
            // outermost expression) before `synth` ever runs, so reaching it here means it is
            // nested inside something that would need its result -- and it has none. The old
            // `Str` typing was a placeholder asserting the opposite of what the fused loop
            // does (a type claiming the whole output exists as one value).
            if func == "jsonlines" {
                return Err(Error::new(
                    *span,
                    "`jsonlines` is a sink, legal only as the program's outermost expression"
                        .to_string(),
                ));
            }
            // `extent`, `tail`, and `concat` are polymorphic over the element type, the same
            // reason `jsonlines` is checked here rather than through `builtin()`'s fixed table.
            if func == "extent" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                if arg.ty.elem().is_none() {
                    return Err(Error::new(
                        arg_span,
                        format!("`extent` needs a Vec, found {}", arg.ty),
                    ));
                }
                return Ok(Tir::new(
                    Type::Int,
                    Kind::Builtin {
                        which: tir::Builtin::Extent,
                        arg: Box::new(arg),
                    },
                ));
            }
            // `None` on an empty Vec, the same way `Index` turns reaching past what's there
            // into `Opt` rather than a runtime failure.
            if func == "tail" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                let Some(elem) = arg.ty.elem().cloned() else {
                    return Err(Error::new(
                        arg_span,
                        format!("`tail` needs a Vec, found {}", arg.ty),
                    ));
                };
                return Ok(Tir::new(
                    Type::Opt(Box::new(Type::Vec(Box::new(elem)))),
                    Kind::Builtin {
                        which: tir::Builtin::Tail,
                        arg: Box::new(arg),
                    },
                ));
            }
            // Flattens a Vec<Vec<T>> into a Vec<T>, the way jq's `add` flattens a list of
            // arrays. A named function rather than `+` on Vec, so it does not decide Q2.
            if func == "concat" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                let inner = arg.ty.elem().cloned();
                let Some(elem) = inner.as_ref().and_then(Type::elem).cloned() else {
                    return Err(Error::new(
                        arg_span,
                        format!("`concat` needs a Vec of Vecs, found {}", arg.ty),
                    ));
                };
                return Ok(Tir::new(
                    Type::Vec(Box::new(elem)),
                    Kind::Builtin {
                        which: tir::Builtin::Concat,
                        arg: Box::new(arg),
                    },
                ));
            }
            // The one exit a stream has: `Stream<T> -> Vec<T>`, polymorphic over the element
            // type like `extent` and the others above, so the argument is synthesised first.
            if func == "collect" {
                return collect(ctx, arg, None);
            }
            if let Some((which, sig)) = builtin(func) {
                let arg = expect(ctx, arg, &sig.param)?;
                return Ok(Tir::new(
                    sig.ret,
                    Kind::Builtin {
                        which,
                        arg: Box::new(arg),
                    },
                ));
            }
            let Some(sig) = ctx.sigs.get(func) else {
                // A payload constructor is ordinary application (the Q34 path), so `circle{r: 1}`
                // lands here; it resolves as a variant only after the function namespace declines.
                if let Some(owners) = ctx.variant_owners.get(func) {
                    let enum_ty = sole_owner(ctx, func, owners, *func_span)?.clone();
                    return construct(ctx, &enum_ty, func, *func_span, Some(arg));
                }
                return Err(Error::new(
                    *func_span,
                    format!("`{func}` is not a function"),
                ));
            };
            let arg = expect(ctx, arg, &sig.param)?;
            Ok(Tir::new(
                sig.ret.clone(),
                Kind::Call {
                    func: func.clone(),
                    arg: Box::new(arg),
                },
            ))
        }

        Expr::Match { arms, span } => match_chain(ctx, arms, *span, None),

        Expr::Variant {
            enum_name,
            enum_span,
            variant,
            variant_span,
            payload,
            ..
        } => {
            let Some(enum_ty) = ctx.enums.get(enum_name) else {
                return Err(Error::new(
                    *enum_span,
                    format!("`{enum_name}` is not an enum"),
                ));
            };
            construct(
                ctx,
                &enum_ty.clone(),
                variant,
                *variant_span,
                payload.as_deref(),
            )
        }

        Expr::Neg { base, span } => {
            // A minus directly on a literal is part of the literal, so the most negative Int can
            // be written even though its magnitude is one past the most positive. This is the
            // rule Rust uses, and it is why `-` was not folded into the lexer: `a -1` has to
            // stay `a - 1`.
            if let Expr::Int { value, span: lit } = base.as_ref() {
                if *value > -(i32::MIN as i64) {
                    return Err(Error::new(
                        *lit,
                        format!("integer `-{value}` does not fit in Int, which is 32 bits"),
                    ));
                }
                return Ok(Tir::new(Type::Int, Kind::Int(-value)));
            }
            let inner = expect(ctx, base, &Type::Int)?;
            let zero = Tir::new(Type::Int, Kind::Int(0));
            let _ = span;
            Ok(Tir::new(
                Type::Int,
                Kind::Arith {
                    op: BinOp::Sub,
                    lhs: Box::new(zero),
                    rhs: Box::new(inner),
                },
            ))
        }

        // The first construct that consumes a type rather than carrying one: the condition has
        // to be exactly one Bool, and both branches have to agree.
        Expr::Cond {
            then,
            cond,
            otherwise,
            ..
        } => {
            let cond = expect(ctx, cond, &Type::Bool)?;
            let then = synth(ctx, then)?;
            // A pipeline's shape must be knowable at compile time for fusion to emit its loop,
            // and a branch chosen at runtime is exactly what that excludes. Refusing is the
            // reversible direction; lifting it later breaks nothing.
            if then.ty.contains_stream() {
                return Err(Error::new(
                    expr.span(),
                    "a conditional cannot yield a stream; pass each branch to `collect` first"
                        .to_string(),
                ));
            }
            let otherwise = expect(ctx, otherwise, &then.ty)?;
            Ok(Tir::new(
                then.ty.clone(),
                Kind::Cond {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                },
            ))
        }

        Expr::Binary { op, lhs, rhs, .. } => binary(ctx, *op, lhs, rhs),
    }
}

/// Walk an access chain left to right, carrying what we are currently looking at and how many
/// dimensions we are inside.
///
/// Every dimension needs a spec. `[]` enters one, so it strips a layer off what we are looking at
/// and adds one to the depth; a field access reads a field of it and leaves the depth alone.
/// The expression's type is what we are looking at, wrapped back up that many times.
///
/// This is why `db.users.name` is an error and `db.users[].name` is not: the first never said
/// what happens to the dimension it reached through.
///
/// A Stream is one more dimension `[]` can enter -- projection is a mapper -- and since the
/// grammar keeps a stream strictly outermost, the walk only has to remember one bit: whether
/// the first-stripped layer was a Stream (`stream_outer`), so wrapping back up restores a
/// Stream there and a Vec everywhere below.
struct Access {
    tir: Tir,
    /// What the chain currently evaluates to, one dimension down: a field access reads this, a
    /// further `[]` strips a layer off it.
    elem: Type,
    /// How many dimensions the chain is inside.
    depth: usize,
    /// Whether the first-stripped layer was a Stream rather than a Vec.
    stream: bool,
}

impl Access {
    fn new(tir: Tir, elem: Type, depth: usize, stream: bool) -> Access {
        Access {
            tir,
            elem,
            depth,
            stream,
        }
    }
}

fn access(ctx: &Ctx, expr: &Expr) -> Result<Access, Error> {
    /// `elem`, wrapped back up under every dimension the chain is inside.
    fn wrap(mut ty: Type, depth: usize, stream_outer: bool) -> Type {
        for i in 0..depth {
            ty = if stream_outer && i == depth - 1 {
                Type::Stream(Box::new(ty))
            } else {
                Type::Vec(Box::new(ty))
            };
        }
        ty
    }

    match expr {
        Expr::Project { base, span } => {
            let b = access(ctx, base)?;
            if let Type::Stream(inner) = &b.elem {
                return Ok(Access::new(b.tir, (**inner).clone(), b.depth + 1, true));
            }
            let Some(inner) = b.elem.elem().cloned() else {
                return Err(Error::new(
                    *span,
                    format!("`[]` needs a dimension, found {}", b.elem),
                ));
            };
            Ok(Access::new(b.tir, inner, b.depth + 1, b.stream))
        }

        // The absence stops being carried and starts being asserted.
        Expr::Unwrap { base, span } => {
            let b = access(ctx, base)?;
            let Type::Opt(inner) = b.elem else {
                return Err(Error::new(
                    *span,
                    format!("`!` needs an Opt, found {}", b.elem),
                ));
            };
            let ty = wrap((*inner).clone(), b.depth, b.stream);
            let tir = Tir::new(
                ty,
                Kind::Unwrap {
                    base: Box::new(b.tir),
                },
            );
            Ok(Access::new(tir, *inner, b.depth, b.stream))
        }

        // Collapsing a dimension. The entry may not be there, so what comes out is `Opt`.
        Expr::Index { base, index, span } => {
            let b = access(ctx, base)?;
            let Some(inner) = b.elem.elem().cloned() else {
                return Err(Error::new(
                    *span,
                    format!("`[i]` needs a dimension, found {}", b.elem),
                ));
            };
            let index_tir = expect(ctx, index, &Type::Int)?;
            let elem_is_record = matches!(inner, Type::Record(_));
            let out = Type::Opt(Box::new(inner));
            let ty = wrap(out.clone(), b.depth, b.stream);
            let kind = Kind::Index {
                base: Box::new(b.tir),
                index: Box::new(index_tir),
                depth: b.depth,
                elem_is_record,
            };
            Ok(Access::new(Tir::new(ty, kind), out, b.depth, b.stream))
        }

        Expr::Field { base, name, span } => {
            let b = access(ctx, base)?;
            if b.elem.elem().is_some() || matches!(b.elem, Type::Stream(_)) {
                return Err(Error::new(
                    *span,
                    format!(
                        "`.{name}` needs a record, found {}: give the dimension a spec with `[]`",
                        b.elem
                    ),
                ));
            }
            let Some(field) = b.elem.field(name).cloned() else {
                return Err(Error::new(
                    *span,
                    format!("no field `{name}` on {}", b.elem),
                ));
            };
            let ty = wrap(field.clone(), b.depth, b.stream);
            let kind = Kind::Field {
                base: Box::new(b.tir),
                name: name.clone(),
            };
            Ok(Access::new(Tir::new(ty, kind), field, b.depth, b.stream))
        }

        other => {
            let tir = synth(ctx, other)?;
            let ty = tir.ty.clone();
            Ok(Access::new(tir, ty, 0, false))
        }
    }
}

fn binary(ctx: &Ctx, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<Tir, Error> {
    let left = synth(ctx, lhs)?;

    // Q2 is open, so an operator over a Vec is rejected rather than being silently given
    // broadcast or zip semantics. Under C1 that restriction is ordinary typing: there is no
    // separate cardinality to check, because a Vec is just a type.
    if left.ty.elem().is_some() {
        return Err(Error::new(
            lhs.span(),
            format!("`{op}` does not apply to {}", left.ty),
        ));
    }

    if op.is_comparison() {
        let right = match expect(ctx, rhs, &left.ty) {
            Ok(right) => right,
            Err(err) => {
                // Postfix `!` next to `!=` merges into one token: `x!=y` lexes as `!=`, not
                // `x! == y`. Once the plain comparison has failed, Opt<T> against its own
                // element is specific enough to name the likely cause instead of just the
                // type mismatch.
                if op == BinOp::Ne
                    && let Type::Opt(inner) = &left.ty
                    && expect(ctx, rhs, inner).is_ok()
                {
                    return Err(Error::new(
                        rhs.span(),
                        format!(
                            "expected {}, found {inner}: `!` and `=` read as one token \
                             here (`!=`); did you mean `! ==`?",
                            left.ty
                        ),
                    ));
                }
                return Err(err);
            }
        };
        return Ok(Tir::new(
            Type::Bool,
            Kind::Compare {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
            },
        ));
    }

    if op.is_arithmetic() {
        if left.ty != Type::Int {
            return Err(Error::new(
                lhs.span(),
                format!("expected Int, found {}", left.ty),
            ));
        }
        let right = expect(ctx, rhs, &Type::Int)?;
        return Ok(Tir::new(
            Type::Int,
            Kind::Arith {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
            },
        ));
    }

    // `+` is the one operator whose meaning depends on its operands: addition on Int and
    // concatenation on Str. Both sides must agree, since nothing is coerced.
    match left.ty {
        Type::Int => {
            let right = expect(ctx, rhs, &Type::Int)?;
            Ok(Tir::new(
                Type::Int,
                Kind::Arith {
                    op: BinOp::Add,
                    lhs: Box::new(left),
                    rhs: Box::new(right),
                },
            ))
        }
        Type::Str => {
            let right = expect(ctx, rhs, &Type::Str)?;
            Ok(Tir::new(
                Type::Str,
                Kind::Concat(Box::new(left), Box::new(right)),
            ))
        }
        other => Err(Error::new(
            lhs.span(),
            format!("`+` needs Int or Str, found {other}"),
        )),
    }
}

/// How an expression met an expectation. `Checked` consumed it: the expected type flowed into
/// the form, and the result is `want`-typed by construction. `Synthesised` means the form
/// answers for itself, so the expectation resolved nothing and the caller still owns the
/// comparison -- which is what lets `check` blame a function's signature, by name, when a
/// body's synthesised type misses its annotation.
enum Expected {
    Checked(Tir),
    Synthesised(Tir),
}

/// The checking direction: an expected type goes in, and the expression is verified against it
/// rather than asked what it is. Most forms answer both questions, but not all do.
fn expect(ctx: &Ctx, expr: &Expr, want: &Type) -> Result<Tir, Error> {
    match expect_inner(ctx, expr, want)? {
        Expected::Checked(tir) => Ok(tir),
        Expected::Synthesised(found) => {
            if &found.ty != want {
                return Err(Error::new(
                    expr.span(),
                    format!(
                        "expected {want}, found {}{}",
                        found.ty,
                        reordered_fields_hint(want, &found.ty)
                    ),
                ));
            }
            Ok(found)
        }
    }
}

/// `expect`, minus the final comparison. Each arm here is a form whose type can come from its
/// position rather than its contents; expectation only ever resolves what synthesis would have
/// refused -- a form that can synthesise falls through and is compared, never coerced.
/// `input` against the type its position wants: the first use fills the program-wide slot,
/// and every later use must agree with it. A signature can spell Stream now, so this position
/// can ask for one; `input` is a whole value already in hand, which is exactly what a stream
/// is not.
fn input_read(ctx: &Ctx, span: Span, want: &Type) -> Result<Tir, Error> {
    if want.contains_stream() {
        return Err(Error::new(
            span,
            format!("`input` is one value read from stdin, but {want} is wanted here"),
        ));
    }
    let mut slot = ctx.input.borrow_mut();
    match slot.as_ref() {
        None => *slot = Some(want.clone()),
        Some(prev) if prev != want => {
            return Err(Error::new(
                span,
                format!(
                    "`input` is used as {prev} here and as {want} elsewhere{}",
                    reordered_fields_hint(prev, want)
                ),
            ));
        }
        Some(_) => {}
    }
    Ok(Tir::new(want.clone(), Kind::Input))
}

/// `inputs` against the type its position wants, which must be a Stream. The filled slot
/// doubles as the single-use flag: a second `inputs` would be a second stream claiming the
/// same real stdin, the same mistake a second `lines` is.
fn inputs_read(ctx: &Ctx, span: Span, want: &Type) -> Result<Tir, Error> {
    if let Some(func) = ctx.in_fn {
        return Err(source_in_fn(span, "inputs", func));
    }
    if ctx.in_mapper {
        return Err(Error::new(
            span,
            "`inputs` cannot be read inside a mapper body, which runs once per element".to_string(),
        ));
    }
    let Type::Stream(elem) = want else {
        return Err(Error::new(
            span,
            format!(
                "`inputs` is a stream, but {want} is wanted here; eager use is spelled \
                 `collect(inputs)`"
            ),
        ));
    };
    let mut slot = ctx.inputs.borrow_mut();
    if slot.is_some() {
        return Err(Error::new(
            span,
            "`inputs` has already been read; there is only one stdin".to_string(),
        ));
    }
    *slot = Some((**elem).clone());
    Ok(Tir::new(want.clone(), Kind::Inputs))
}

fn expect_inner(ctx: &Ctx, expr: &Expr, want: &Type) -> Result<Expected, Error> {
    // The forms whose type comes from their position rather than their contents.
    if let Expr::Input { span } = expr {
        return input_read(ctx, *span, want).map(Expected::Checked);
    }

    if let Expr::Inputs { span } = expr {
        return inputs_read(ctx, *span, want).map(Expected::Checked);
    }

    // `collect(inputs)` in a Vec-wanted position: the honest eager spelling the decision
    // records. `collect` is otherwise synthesised argument-first, but `inputs` has no type of
    // its own until checked, so the wanted Vec<T> is pushed back through as Stream<T> here.
    if let Expr::Call { func, arg, .. } = expr
        && func == "collect"
        && let Some(elem) = want.elem()
    {
        return collect(ctx, arg, Some(elem)).map(Expected::Checked);
    }

    // A string literal where an enum is wanted is the variant it names, the same spelling the
    // wire format already gives a unit variant. `construct` owns the errors: a payload variant
    // gets the how-to-write-it hint, and an unknown name is named against the enum.
    if let Expr::Str { text, span } = expr
        && matches!(want, Type::Enum { .. })
    {
        return construct(ctx, want, text, *span, None).map(Expected::Checked);
    }

    // The pipe's value is the right side's, so the expectation flows into the right side --
    // the only road into the subject-fed forms, which exist only after `|`.
    if let Expr::Pipe { lhs, rhs, .. } = expr {
        return pipe(ctx, lhs, rhs, Some(want));
    }

    // A `map` whose position expects a Vec (over a Vec subject) or a Stream (over a Stream
    // subject) pushes the expected element type into its body. Any other want, or no subject
    // at all, falls through to synthesis, which owns those errors.
    if let Expr::Call {
        func, arg, span, ..
    } = expr
        && func == "map"
    {
        let want_elem = match (ctx.subject.as_ref().map(|(ty, _)| ty), want) {
            (Some(Type::Vec(_)), Type::Vec(elem)) => Some(elem.as_ref()),
            (Some(Type::Stream(_)), Type::Stream(elem)) => Some(elem.as_ref()),
            _ => None,
        };
        if let Some(elem) = want_elem {
            return map_call(ctx, arg, *span, Some(elem)).map(Expected::Checked);
        }
    }

    // Both branches of a conditional receive the expectation: which one runs is a runtime
    // fact, so each must meet the position on its own. A want containing a stream falls
    // through to synthesis instead, which owns the runtime-chosen-pipeline refusal.
    if let Expr::Cond {
        then,
        cond,
        otherwise,
        ..
    } = expr
        && !want.contains_stream()
    {
        let cond = expect(ctx, cond, &Type::Bool)?;
        let then = expect(ctx, then, want)?;
        let otherwise = expect(ctx, otherwise, want)?;
        return Ok(Expected::Checked(Tir::new(
            want.clone(),
            Kind::Cond {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
        )));
    }

    // Every arm of a total match chain receives the expectation the same way. A partial
    // chain -- all guards, no default -- keeps synthesising instead: its result is Opt of
    // what the arms yield, and pushing a peeled expectation into the arms would turn the
    // arms-are-already-Opt refusal, which explains the design, into a bare mismatch.
    if let Expr::Match { arms, span } = expr
        && !want.contains_stream()
        && arms.iter().any(|a| !matches!(a.pattern, Pattern::Guard(_)))
    {
        return match_chain(ctx, arms, *span, Some(want)).map(Expected::Checked);
    }

    // A record literal checked against a record type pushes each field's expected type into
    // its value -- but only when the written fields are the declared fields, in order: field
    // order is part of a record type, so a shuffled or mis-fielded literal falls back to
    // synthesis, where the existing errors (the reordered-fields hint included) say what is
    // wrong.
    if let Expr::RecordLit { fields, .. } = expr
        && let Type::Record(want_fields) = want
        && fields.len() == want_fields.len()
        && fields
            .iter()
            .zip(want_fields)
            .all(|((name, _, _), (wanted, _))| name == wanted)
    {
        let mut built = Vec::new();
        for ((name, _, value), (_, field_ty)) in fields.iter().zip(want_fields) {
            let field = expect(ctx, value, field_ty).map_err(|e| {
                stream_refusal(ctx, value, || {
                    format!("`{name}` cannot hold a stream, which has nothing to store")
                })
                .unwrap_or(e)
            })?;
            built.push((name.clone(), field));
        }
        return Ok(Expected::Checked(Tir::new(
            want.clone(),
            Kind::RecordLit { fields: built },
        )));
    }

    // A Vec literal where a Vec is wanted takes its element type from the position, which is
    // what lets `[]` -- refused outright under synthesis -- resolve wherever something already
    // says what it holds.
    if let Expr::VecLit { items, .. } = expr
        && let Type::Vec(elem) = want
    {
        let mut out = Vec::new();
        for item in items {
            let item = expect(ctx, item, elem).map_err(|e| {
                stream_refusal(ctx, item, || {
                    "a Vec cannot hold a stream, which has nothing to store".to_string()
                })
                .unwrap_or(e)
            })?;
            out.push(item);
        }
        return Ok(Expected::Checked(Tir::new(want.clone(), Kind::VecLit(out))));
    }

    synth(ctx, expr).map(Expected::Synthesised)
}

/// The synth arms refuse a stream inside a record or Vec with a message that teaches why
/// (nothing can store or re-read one); the checked fast paths above would otherwise demote
/// that refusal to a bare "expected T, found Stream<T>" mismatch. When a pushed field or
/// element fails, ask synthesis whether a stream was the reason and keep the lesson.
fn stream_refusal(ctx: &Ctx, value: &Expr, msg: impl FnOnce() -> String) -> Option<Error> {
    let found = synth(ctx, value).ok()?;
    found
        .ty
        .contains_stream()
        .then(|| Error::new(value.span(), msg()))
}

/// The hint for the one mismatch that looks self-contradictory: both sides spell the same
/// fields, so "expected X, found X-with-the-fields-shuffled" needs the reader told that order
/// is the difference. Fires only when the field sets (names and types both) genuinely match.
fn reordered_fields_hint(a: &Type, b: &Type) -> &'static str {
    let (Type::Record(x), Type::Record(y)) = (a, b) else {
        return "";
    };
    if x.len() == y.len() && x.iter().all(|f| y.contains(f)) {
        "; the fields agree, but field order is part of a record type"
    } else {
        ""
    }
}
