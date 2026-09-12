use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::ast::{
    Alias, BinOp, Def, EnumDecl, Expr, FieldsPattern, File, ImplDecl, MatchArm, Origin, Param,
    ParamShape, Pattern, Span, TraitDecl,
};
use crate::error::Error;
use crate::tir::{self, Kind, LocalId, Tir};
use crate::ty::{self, Sig, Type};

mod linearity;
mod types;

use linearity::{
    StreamBinding, check_linear, field_used, local_used, param_used, prune_unreachable,
};
use types::{
    TypeEnv, alias_map, check_sig_invariants, check_type_params, constructor_of, enum_map,
    is_constructor_of, matcher_of, resolve, resolve_enum, resolve_with_params, signatures,
};

struct Ctx<'a> {
    sigs: &'a HashMap<String, Sig>,
    /// Every declared enum, resolved. Keyed by name, which is the identity.
    enums: &'a HashMap<String, Type>,
    /// Which enums declare each variant name, in declaration order. A bare variant use resolves
    /// through this while exactly one enum claims the name; two claimants are a loud error
    /// naming both, with `Shape.circle` as the qualified way out.
    variant_owners: &'a HashMap<String, Vec<String>>,
    /// Named bindings, innermost last. A function parameter carries `None` for its local (the
    /// backends render a param by name); a `let` binding carries the `LocalId` it was bound to,
    /// so reading the name is reading that local. `let` can stack many of these, and a later one
    /// shadows an earlier, which is what the reverse search in `synth` is for.
    scope: Vec<(String, Type, Option<LocalId>)>,
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
    /// The delimiter `dsv` was read with, filled in the first time it is used. `csv`/`tsv`
    /// arrive here with the delimiter already fixed by the parser. A second read of any kind
    /// is refused the same way a second `lines` is.
    dsv: &'a RefCell<Option<String>>,
    /// Whether checking is inside a mapper's body (`map`'s, or `select`'s predicate), which
    /// runs once per element: a source read there would drain stdin on the first element and
    /// hand every later one nothing, so `lines` and `inputs` are refused in that position.
    in_mapper: bool,
    /// The name of the `fn` whose body is being checked; `None` in the program's own body. A
    /// source is legal only in the program body (see `source_in_fn`), so `lines` and `inputs`
    /// are refused whenever this is set.
    in_fn: Option<&'a str>,
    /// Every user function, keyed by name, with the file it was defined in and whether it is
    /// `pub`. A call to a non-`pub` one from a different file than `file` is refused, which is
    /// the visibility rule (gh:166).
    visibility: &'a HashMap<String, (Origin, bool)>,
    /// Which file the code being checked was written in. The program's body and definitions are
    /// `Program`; the prelude's definitions (checked at build time) are `Prelude`.
    file: Origin,
    next_local: &'a Cell<LocalId>,
    /// Every resolved impl method, `collect_impls`'s output: what a colon call
    /// (`Expr::ColonCall`, `x:foo(y)`) consults to dispatch by the receiver's concrete type.
    /// Kept out of `sigs`/`visibility` entirely -- see `collect_impls`'s doc comment.
    impls: &'a [ImplEntry],
    /// Every generic impl (`impl<T> Trait for Vec<T>`), still templated: `collect_impls`'s
    /// other output. A colon call whose method matches none of `impls` tries `unify`ing each of
    /// these templates' receiver against the call's concrete receiver type instead (`find_or_monomorphize`).
    generic_templates: &'a [GenericImplTemplate],
    /// Every generic impl method actually monomorphized so far, in dispatch order: the memo
    /// table `find_or_monomorphize` checks before unifying a template again, and consults before
    /// `impls` misses so a second dispatch to the same concrete type reuses the already-checked
    /// `Def` rather than rechecking its body. Grows during checking, unlike `impls`, which is
    /// fixed once `collect_impls` returns -- hence the `RefCell`, shared by every `Ctx` derived
    /// from the same top-level call.
    generic_synth: &'a RefCell<Vec<ImplEntry>>,
    /// The `tir::Func` `find_or_monomorphize` synthesizes each time it monomorphizes a new
    /// `(method, concrete type)` pair, in the same order as `generic_synth`. Merged into the
    /// program's function list once checking finishes.
    generic_funcs: &'a RefCell<Vec<tir::Func>>,
}

impl Ctx<'_> {
    fn with(&self, subject: Option<(Type, LocalId)>) -> Ctx<'_> {
        self.rebuild(self.scope.clone(), subject)
    }

    fn rebuild(
        &self,
        scope: Vec<(String, Type, Option<LocalId>)>,
        subject: Option<(Type, LocalId)>,
    ) -> Ctx<'_> {
        Ctx {
            sigs: self.sigs,
            enums: self.enums,
            variant_owners: self.variant_owners,
            scope,
            arm_fields: self.arm_fields.clone(),
            subject,
            input: self.input,
            inputs: self.inputs,
            lines_used: self.lines_used,
            dsv: self.dsv,
            in_mapper: self.in_mapper,
            in_fn: self.in_fn,
            visibility: self.visibility,
            file: self.file,
            next_local: self.next_local,
            impls: self.impls,
            generic_templates: self.generic_templates,
            generic_synth: self.generic_synth,
            generic_funcs: self.generic_funcs,
        }
    }

    fn fresh(&self) -> LocalId {
        let id = self.next_local.get();
        self.next_local.set(id + 1);
        id
    }
}

/// The setup `check` and `check_module` share: build the type environment, resolve every declared
/// enum, index which enum claims each variant name, collect the provisional signatures, and map
/// each function to its file and visibility. Everything a `Ctx` needs except the per-file cells,
/// which the caller owns because a module has no program body to run the input-exclusivity
/// checks over. The module caller passes `&[]` for aliases, so the eager alias resolution below
/// is a no-op for it.
fn resolve_defs<'a>(
    aliases: &'a [Alias],
    enum_decls: &'a [EnumDecl],
    defs: &'a [Def],
    file: Origin,
) -> Result<
    (
        TypeEnv<'a>,
        HashMap<String, Type>,
        HashMap<String, Vec<String>>,
        HashMap<String, Sig>,
        HashMap<String, (Origin, bool)>,
    ),
    Error,
> {
    let env = TypeEnv {
        aliases: alias_map(aliases)?,
        enums: enum_map(enum_decls)?,
    };
    // Resolved eagerly so a broken declaration is an error even when nothing uses it, and so a
    // cycle is found here rather than wherever it happened to be reached from.
    for a in aliases {
        resolve(&a.ty, &env, &mut vec![(a.name.clone(), Vec::new())])?;
    }
    let mut enums: HashMap<String, Type> = HashMap::new();
    for e in enum_decls {
        enums.insert(
            e.name.clone(),
            resolve_enum(e, &env, &mut Vec::new(), None)?,
        );
    }
    let mut variant_owners: HashMap<String, Vec<String>> = HashMap::new();
    for e in enum_decls {
        for v in &e.variants {
            variant_owners
                .entry(v.name.clone())
                .or_default()
                .push(e.name.clone());
            // The lowercase spelling is the constructor, the value built right away (gh:156):
            // a bare lowercase variant name resolves to the same owner so it reaches `construct`,
            // which maps it to the declared variant. A declared name already lowercase (the
            // prelude's `Opt`/`Result`) has the same constructor spelling, so pushing again would
            // make one owner look like two.
            let constructor = constructor_of(&v.name);
            if constructor != v.name {
                variant_owners
                    .entry(constructor)
                    .or_default()
                    .push(e.name.clone());
            }
        }
    }
    let sigs = signatures(defs, &env)?;
    let visibility: HashMap<String, (Origin, bool)> = defs
        .iter()
        .map(|d| (d.name.clone(), (d.origin, d.is_pub)))
        .collect();
    Ok((env, enums, variant_owners, sigs, visibility))
}

/// The per-file mutable checker state, borrowed so a module with no program body can still build
/// a `Ctx`. Bundled so both callers construct it once and get every `Ctx` from one method rather
/// than repeating the seventeen-field literal.
struct Cells<'a> {
    input: &'a RefCell<Option<Type>>,
    inputs: &'a RefCell<Option<Type>>,
    lines_used: &'a Cell<bool>,
    dsv: &'a RefCell<Option<String>>,
    next_local: &'a Cell<LocalId>,
    /// Every generic impl, and the two growth points a generic impl's dispatch writes into,
    /// shared by every `Ctx` this `Cells` builds -- see the fields of the same name on `Ctx`.
    /// Grouped here (rather than passed to `ctx()` like `impls`) purely to keep that method's
    /// own argument count down; `generic_templates` does not vary between a caller's two `ctx()`
    /// calls any more than `generic_synth`/`generic_funcs` do.
    generic_templates: &'a [GenericImplTemplate],
    generic_synth: &'a RefCell<Vec<ImplEntry>>,
    generic_funcs: &'a RefCell<Vec<tir::Func>>,
}

impl Cells<'_> {
    /// A fresh top-level context over `sigs` and the resolved declarations: empty scope and
    /// subject, not inside a mapper or a function. `file` is the only per-caller difference.
    fn ctx<'a>(
        &'a self,
        sigs: &'a HashMap<String, Sig>,
        enums: &'a HashMap<String, Type>,
        variant_owners: &'a HashMap<String, Vec<String>>,
        visibility: &'a HashMap<String, (Origin, bool)>,
        impls: &'a [ImplEntry],
        file: Origin,
    ) -> Ctx<'a> {
        Ctx {
            sigs,
            enums,
            variant_owners,
            scope: Vec::new(),
            arm_fields: Vec::new(),
            subject: None,
            input: self.input,
            inputs: self.inputs,
            lines_used: self.lines_used,
            dsv: self.dsv,
            in_mapper: false,
            in_fn: None,
            visibility,
            file,
            next_local: self.next_local,
            impls,
            generic_templates: self.generic_templates,
            generic_synth: self.generic_synth,
            generic_funcs: self.generic_funcs,
        }
    }
}

pub fn check(file: File) -> Result<tir::Program, Error> {
    let File {
        aliases,
        enums: enum_decls,
        traits,
        impls: impl_decls,
        defs,
        body: program_body,
    } = file;
    let (env, enums, variant_owners, mut sigs, visibility) =
        resolve_defs(&aliases, &enum_decls, &defs, Origin::Program)?;
    for e in &enum_decls {
        if env.aliases.contains_key(&e.name) {
            return Err(Error::new(
                e.span,
                format!("type `{}` is defined twice", e.name),
            ));
        }
    }
    // `prelude::inject` already prepended the prelude's own traits/impls to these, tagged
    // `Origin::Prelude`, alongside its defs -- one combined collection pass, so a program's
    // colon call can dispatch to either a prelude-declared impl or its own, and the
    // plain-function-vs-trait-method and impl-vs-impl checks below see the whole picture rather
    // than half of it twice.
    let (impl_defs, impl_table, generic_templates) =
        collect_impls(&traits, impl_decls, &env, &sigs)?;
    let input = RefCell::new(None);
    let inputs = RefCell::new(None);
    let lines_used = Cell::new(false);
    let dsv = RefCell::new(None);
    let next_local = Cell::new(0);
    let generic_synth: RefCell<Vec<ImplEntry>> = RefCell::new(Vec::new());
    let generic_funcs: RefCell<Vec<tir::Func>> = RefCell::new(Vec::new());
    let cells = Cells {
        input: &input,
        inputs: &inputs,
        lines_used: &lines_used,
        dsv: &dsv,
        next_local: &next_local,
        generic_templates: &generic_templates,
        generic_synth: &generic_synth,
        generic_funcs: &generic_funcs,
    };
    // The first context carries `signatures`' provisional hoisted signatures (ret = the matched
    // enum), enough to check their bodies; the inference pass below replaces each provisional
    // return with the body's actual type and rebuilds the context before anything is checked.
    let ctx = cells.ctx(
        &sigs,
        &enums,
        &variant_owners,
        &visibility,
        &impl_table,
        Origin::Program,
    );
    // Return-type inference for hoisted definitions (`fn name = expr`, gh:152): a hoisted
    // function's signature is not written, so no body -- its own or another's -- may be checked
    // against the provisional return `signatures` seeded. Checking each hoisted body once here
    // fixes the real return, and rebuilding `sigs` first is what lets every later call, a
    // recursive one included, resolve it.
    for (name, sig) in infer_hoisted(&ctx, defs.iter())? {
        sigs.insert(name, sig);
    }
    let ctx = cells.ctx(
        &sigs,
        &enums,
        &variant_owners,
        &visibility,
        &impl_table,
        Origin::Program,
    );

    // `prelude::inject` prepended prelude.toy's own defs to `defs`, which is what lets `sigs`
    // above resolve calls into them and is what catches a program that redefines one -- but
    // their bodies were already checked once, at build time (`build.rs`, via `prelude::checked`),
    // and prelude.toy cannot have changed since. Rechecking them here would only repeat that
    // work on every single compile. The prelude's impls are not part of that precomputed set
    // (`build.rs` strips them before generating it, see its `main`) -- they flow through
    // `collect_impls` above instead, the same one pass the program's own impls do, so
    // `check_defs` below checks every impl method's body fresh regardless of origin.
    let mut funcs = crate::prelude::checked();
    let precompiled_names: HashSet<String> = funcs.iter().map(|f| f.name.clone()).collect();
    let own_defs = defs.iter().filter(|d| !precompiled_names.contains(&d.name));
    funcs.extend(check_defs(own_defs, &ctx)?);
    funcs.extend(check_defs(impl_defs.iter(), &ctx)?);

    let body = check_program_body(&ctx, &program_body)?;
    // Every generic impl method a dispatch inside any of the above actually monomorphized: a
    // program-body dispatch pushes into the same `RefCell` an impl method's own body might have,
    // since `find_or_monomorphize` runs during whichever body reaches the dispatch site first.
    funcs.extend(generic_funcs.into_inner());
    let stdin =
        check_result_and_stdin(&body, program_body.span(), input, inputs, &lines_used, dsv)?;
    Ok(tir::Program {
        funcs: prune_unreachable(funcs, &body),
        body,
        input: stdin.input,
        inputs: stdin.inputs,
        uses_lines: lines_used.get(),
        dsv: stdin.dsv,
        enums,
    })
}

/// What `check_result_and_stdin` resolves `input`/`inputs`/`dsv` to, for `tir::Program` to carry
/// -- named so the function's own return type is not a bare three-`Option` tuple nothing
/// distinguishes at the call site.
struct StdinReaders {
    input: Option<Type>,
    inputs: Option<Type>,
    dsv: Option<String>,
}

/// `check`'s validation once the program's result and every stdin-reading form are known: the
/// program's own result cannot be a stream (nothing to print; `collect` first) or a Char (no
/// wire form), and `input`/`lines`/`inputs`/`dsv` are four different ways of reading the one
/// real stdin, mutually exclusive because the backends force it -- Python's `input` reads stdin
/// to EOF before parsing, and jq needs a different invocation flag (`-R -n` vs `-n`) for raw
/// lines than for parsed JSON, so no one process can run with more than one of them requested.
fn check_result_and_stdin(
    body: &Tir,
    body_span: Span,
    input: RefCell<Option<Type>>,
    inputs: RefCell<Option<Type>>,
    lines_used: &Cell<bool>,
    dsv: RefCell<Option<String>>,
) -> Result<StdinReaders, Error> {
    // A stream cannot be printed, having nothing to show: it is not a value, and collect() is
    // what turns it into one. A function body catches this for free, since its return
    // annotation can never spell Stream and so can never match a body that contains one; the
    // program's own result has no annotation to check against, so it needs asking directly.
    if body.ty.contains_stream() {
        return Err(Error::new(
            body_span,
            "the program's result contains a stream, which has nothing to print; pass it to \
             `collect` first"
                .to_string(),
        ));
    }
    // A Char has no wire form (the reverse of `input`'s refusal above), so a program cannot
    // hand one to the printer either -- it has to build a Str first.
    if body.ty.contains_char() {
        return Err(Error::new(
            body_span,
            "the program's result contains a Char, which has no wire form to print".to_string(),
        ));
    }
    let input = input.into_inner();
    let inputs = inputs.into_inner();
    if input.is_some() && lines_used.get() {
        return Err(Error::new(
            body_span,
            "a program cannot use both `input` and `lines`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    if input.is_some() && inputs.is_some() {
        return Err(Error::new(
            body_span,
            "a program cannot use both `input` and `inputs`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    if lines_used.get() && inputs.is_some() {
        return Err(Error::new(
            body_span,
            "a program cannot use both `lines` and `inputs`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    // `dsv` reads the same raw lines `stdin` does, so it joins the same exclusivity: one real
    // stdin, read one way.
    let dsv = dsv.into_inner();
    for (other, name) in [
        (input.is_some(), "`parse(stdin)`"),
        (inputs.is_some(), "`stdin | map(parse(.))`"),
        (lines_used.get(), "`stdin`"),
    ] {
        if dsv.is_some() && other {
            return Err(Error::new(
                body_span,
                format!(
                    "a program cannot use both `dsv` and {name}; they read the same real stdin \
                     two different ways"
                ),
            ));
        }
    }
    Ok(StdinReaders { input, inputs, dsv })
}

/// What `check_defs` made of a definition's parameter: the TIR param name the backends bind
/// the function's argument to, and, for a destructured one, the locals each named field was
/// lowered to. The param name is a real user name for a plain parameter and a hidden one the body
/// never sees for a destructured one.
struct LoweredParam {
    name: String,
    record: Option<LocalId>,
    field_locals: Vec<(String, Span, LocalId, Type)>,
}

/// A checked def's `Sig`: a plain function's lives in `ctx.sigs` as always, an impl method's in
/// `ctx.impls` instead (`collect_impls` keeps the two apart entirely -- see its doc comment), so
/// `check_defs` asks whichever one actually has `name` rather than assuming its own name never
/// falls in the other. The two are disjoint by construction (`collect_impls`'s cross-check
/// refuses a name shared by both), so exactly one of them ever answers.
fn sig_of<'a>(ctx: &'a Ctx, name: &str) -> &'a Sig {
    if let Some(sig) = ctx.sigs.get(name) {
        return sig;
    }
    &ctx.impls
        .iter()
        .find(|e| e.def_name == name)
        .expect("a checked def's name resolves in sigs or the impl table")
        .sig
}

/// Signatures are collected before any body is checked, so a definition may call one that
/// appears later in `defs`. This is also what recursion needs. No reachability pruning happens
/// here: that needs the whole program's body, which `check_module` never has and `check` only
/// gets once this returns, so both call this and prune afterward, over the combined result.
/// `ctx`'s own per-def fields (`scope`, `subject`, `in_fn`, ...) are ignored -- each def gets its
/// own, built from its signature exactly as `check` always has.
fn check_defs<'a>(
    defs: impl IntoIterator<Item = &'a crate::ast::Def>,
    ctx: &Ctx<'a>,
) -> Result<Vec<tir::Func>, Error> {
    let mut funcs = Vec::new();
    for def in defs {
        // A hoisted definition (`fn name = expr`, gh:152) is checked by its own path: the
        // signature is inferred rather than written, so none of the annotated-param machinery
        // below applies. `infer_hoisted` already checked it once to fix the return type; this
        // second check produces the `Func` the backends emit.
        if def.hoisted {
            funcs.push(check_hoisted_def(ctx, def)?);
            continue;
        }
        let sig = sig_of(ctx, &def.name);
        funcs.push(check_one_def(
            ctx,
            &def.name,
            def.param.as_ref(),
            sig,
            &def.body,
        )?);
    }
    Ok(funcs)
}

/// `Ctx::scope`'s own element type, named so `lower_param` can spell it as a return type without
/// tripping clippy's `type_complexity` (the tuple itself is nothing new -- every `scope` in this
/// file already carries it inline).
type Scope = Vec<(String, Type, Option<LocalId>)>;

/// The `(scope, lowered)` pair `check_one_def` needs before it can check a body: a plain-named
/// parameter binds directly; a destructuring pattern (`{a, b}: T`) binds each named field to a
/// fresh local off a hidden whole-record parameter the body never sees, which is what lets
/// `let`-shadowing behave the same for either shape (kantord/toylang#144).
fn lower_param(
    ctx: &Ctx,
    name: &str,
    param: Option<&Param>,
    param_ty: &Option<Type>,
) -> Result<(Scope, LoweredParam), Error> {
    match (param, param_ty) {
        (Some(param), Some(param_ty)) => match &param.shape {
            ParamShape::Name(pname, _) => Ok((
                vec![(pname.clone(), param_ty.clone(), None)],
                LoweredParam {
                    name: pname.clone(),
                    record: None,
                    field_locals: Vec::new(),
                },
            )),
            ParamShape::Fields(fields) => {
                let Type::Record(pfields) = param_ty else {
                    return Err(Error::new(
                        fields.span,
                        format!("a destructuring parameter needs a record type, found {param_ty}"),
                    ));
                };
                let record = ctx.fresh();
                let mut scope = Vec::new();
                let mut field_locals = Vec::new();
                for (i, (fname, fspan)) in fields.names.iter().enumerate() {
                    if fields.names[..i].iter().any(|(seen, _)| seen == fname) {
                        return Err(Error::new(
                            *fspan,
                            format!("`{fname}` is bound twice in this pattern"),
                        ));
                    }
                    let Some((_, fty)) = pfields.iter().find(|(n, _)| n == fname) else {
                        return Err(Error::new(
                            *fspan,
                            format!("no field `{fname}` on {param_ty}"),
                        ));
                    };
                    let fid = ctx.fresh();
                    scope.push((fname.clone(), fty.clone(), Some(fid)));
                    field_locals.push((fname.clone(), *fspan, fid, fty.clone()));
                }
                // Leaving fields out of a destructuring parameter is a forgotten field until
                // `..` says it was meant, the same rule a match arm's pattern follows.
                if !fields.rest {
                    let missing: Vec<String> = pfields
                        .iter()
                        .filter(|(n, _)| !fields.names.iter().any(|(m, _)| m == n))
                        .map(|(n, _)| format!("`{n}`"))
                        .collect();
                    if !missing.is_empty() {
                        return Err(Error::new(
                            fields.span,
                            format!(
                                "a destructuring parameter must name every field of its type \
                                 or end in `..`; missing {}",
                                missing.join(" and ")
                            ),
                        ));
                    }
                }
                let pname = format!("__{name}_param");
                Ok((
                    scope,
                    LoweredParam {
                        name: pname,
                        record: Some(record),
                        field_locals,
                    },
                ))
            }
        },
        (None, None) => Ok((
            Vec::new(),
            LoweredParam {
                name: String::new(),
                record: None,
                field_locals: Vec::new(),
            },
        )),
        _ => unreachable!("a signature's param mirrors its definition's"),
    }
}

/// A destructured parameter's record arrives as a hidden param and is bound to a fresh local,
/// then each named field is projected off it -- the same `Bind` shape `let` uses, so every
/// backend already knows how to run one. Field bindings wrap innermost-first so the record's
/// own bind comes first.
fn bind_destructured_fields(
    record: LocalId,
    param_ty: Type,
    lowered: &LoweredParam,
    body: Tir,
) -> Tir {
    let mut body = body;
    for (fname, _, fid, fty) in lowered.field_locals.iter().rev() {
        let base = Tir::new(param_ty.clone(), Kind::Local(record));
        body = Tir::new(
            body.ty.clone(),
            Kind::Bind {
                local: *fid,
                value: Box::new(Tir::new(
                    fty.clone(),
                    Kind::Field {
                        base: Box::new(base),
                        name: fname.clone(),
                    },
                )),
                body: Box::new(body),
            },
        );
    }
    Tir::new(
        body.ty.clone(),
        Kind::Bind {
            local: record,
            value: Box::new(Tir::new(param_ty, Kind::Var(lowered.name.clone()))),
            body: Box::new(body),
        },
    )
}

/// One non-hoisted definition's body, checked against `sig` and lowered to a `tir::Func`: the
/// machinery a plain top-level `fn` and a generic impl's freshly monomorphized method share
/// alike, the only difference being where `sig` came from (`sig_of`'s lookup for the former,
/// `unify`+`substitute` for the latter, in `find_or_monomorphize`). `name` is what the body's
/// own error messages name and what the resulting `Func` is called; for an impl method this is
/// already the mangled `"{method}::{Type}"` form.
fn check_one_def(
    ctx: &Ctx,
    name: &str,
    param: Option<&Param>,
    sig: &Sig,
    body_expr: &Expr,
) -> Result<tir::Func, Error> {
    let (scope, lowered) = lower_param(ctx, name, param, &sig.param)?;
    let def_ctx = Ctx {
        sigs: ctx.sigs,
        enums: ctx.enums,
        variant_owners: ctx.variant_owners,
        scope,
        arm_fields: Vec::new(),
        subject: None,
        input: ctx.input,
        inputs: ctx.inputs,
        lines_used: ctx.lines_used,
        dsv: ctx.dsv,
        in_mapper: false,
        in_fn: Some(name),
        visibility: ctx.visibility,
        file: ctx.file,
        next_local: ctx.next_local,
        impls: ctx.impls,
        generic_templates: ctx.generic_templates,
        generic_synth: ctx.generic_synth,
        generic_funcs: ctx.generic_funcs,
    };
    // The declared return type flows into the body, so a form whose type comes from its
    // position (`[]`, `input`, a variant-naming string) resolves against the annotation. A
    // body that synthesises instead is compared below, where the error can name the
    // function rather than just the two types.
    //
    // A function declared `-> Sink` is the other sink position, so its body must be a sink
    // call -- the tail-pipeline `|>` form or a direct call to a sink; nothing else is a
    // sink, and a body that is not one fails here with the declared-vs-found mismatch below
    // naming the function.
    let mut body = if sig.ret == Type::Sink {
        if matches!(body_expr, Expr::TailPipe { .. }) {
            tail_pipe(&def_ctx, body_expr)?
        } else if let Some(tir) = sink_call(&def_ctx, body_expr)? {
            tir
        } else {
            return Err(Error::new(
                body_expr.span(),
                format!(
                    "`{name}` declares it returns Sink, so its body must be a sink call \
                     such as `jsonlines(...)` or `x |> jsonlines`"
                ),
            ));
        }
    } else {
        match expect_inner(&def_ctx, body_expr, &sig.ret)? {
            Expected::Checked(body) => body,
            Expected::Synthesised(body) => conform(&def_ctx, body, &sig.ret),
        }
    };
    if let Some(record) = lowered.record {
        let param_ty = sig
            .param
            .clone()
            .expect("a destructured param's type is Some");
        body = bind_destructured_fields(record, param_ty, &lowered, body);
    }
    if let Some(param) = param {
        check_param(&body, param, &lowered, &sig.param, name, body_expr.span())?;
    }
    if body.ty != sig.ret {
        return Err(Error::new(
            body_expr.span(),
            format!(
                "`{name}` declares it returns {}, but its body is {}",
                sig.ret, body.ty
            ),
        ));
    }
    Ok(tir::Func {
        name: name.to_string(),
        param: if param.is_some() {
            Some(lowered.name.clone())
        } else {
            None
        },
        param_ty: sig.param.clone(),
        body,
    })
}

/// One resolved impl method: `collect_impls`'s dispatch table, and what a colon call
/// (`x:foo(y)`) looks up by `(method, receiver)`. `Type` has no `Hash` (`src/ty.rs`), so the
/// table this lives in is a `Vec` scanned with `==`, not a `HashMap` -- an implementation
/// detail, not a design constraint. `Clone` is for `find_or_monomorphize`, which hands one back
/// by value whether it came from the static table or was just synthesized into the dynamic one.
#[derive(Clone)]
struct ImplEntry {
    /// The method name as written (`"add"`), what a colon call names and what the
    /// plain-function-vs-trait-method cross-check compares against `sigs`' keys.
    method: String,
    /// The concrete type `Self` substituted to: what a colon call's receiver type is matched
    /// against.
    receiver: Type,
    /// The synthesized internal `Func` name, `"{method}::{TypeName}"`: unreachable by any
    /// user-written function name (`::` cannot appear inside a lexed `Ident`), and what
    /// `check_defs` checks the body under and a dispatching `Kind::Call` names.
    def_name: String,
    sig: Sig,
    origin: Origin,
    is_pub: bool,
    /// The impl method's own span, for a collision or cross-check error to point at.
    span: Span,
}

/// One trait method of a generic impl (`impl<T> Trait for Vec<T>`), templated: `param_ty`/
/// `ret_ty` are resolved from the impl's written signature exactly as `collect_impls` resolves a
/// concrete impl's, except with `T` (and any other declared parameter) bound to `Type::Param`
/// rather than a concrete type. The body is checked only once a dispatch site actually
/// instantiates this at a concrete type (`find_or_monomorphize`) -- until then it is untouched
/// source, borrowed from here rather than cloned.
struct GenericImplMethod {
    name: String,
    param: Option<Param>,
    param_ty: Option<Type>,
    ret_ty: Type,
    body: Expr,
    span: Span,
    origin: Origin,
}

/// One `impl<T, ...> Trait for Type<T, ...>` block, kept templated rather than expanded into a
/// `Def` per method the way a concrete impl is: `collect_impls` resolves `receiver` and each
/// method's signature once, with every declared parameter bound to `Type::Param`, and stops
/// there. `find_or_monomorphize` is what actually turns one of `methods` into a checked `Def`,
/// by unifying `receiver` against a dispatch site's concrete receiver type.
struct GenericImplTemplate {
    receiver: Type,
    methods: Vec<GenericImplMethod>,
}

/// Whether `t` mentions the type parameter `name` anywhere in its structure -- `collect_impls`
/// refuses a generic impl whose declared parameter never appears in its own target type
/// (`impl<T> Trait for Circle`, say), since `unify` would then never bind it and a later
/// `substitute` on a method that does mention it would have nothing to substitute.
fn type_mentions_param(t: &Type, name: &str) -> bool {
    match t {
        Type::Param(p) => p == name,
        Type::Vec(inner) | Type::Stream(inner) => type_mentions_param(inner, name),
        Type::Record(fields) => fields.iter().any(|(_, t)| type_mentions_param(t, name)),
        Type::Enum { args, .. } => args.iter().any(|a| type_mentions_param(a, name)),
        Type::Str
        | Type::Int
        | Type::Int64
        | Type::Float
        | Type::Bool
        | Type::Char
        | Type::Sink => false,
    }
}

/// An impl block's methods, checked against their trait's signatures (`Self` substituted by
/// the impl's target type) and synthesized into ordinary functions the same path a hand-written
/// one takes -- named `"{method}::{TypeName}"` rather than by source, so two impls of different
/// types can share a method name without colliding, which is the actual fix over the old
/// `collect_impls`: that one wrote straight into a flat `sigs: HashMap<String, Sig>` keyed by
/// the bare method name (`src/check/mod.rs` before this rewrite), so a second `impl ... for`
/// with a same-named method as a first collided outright regardless of the two types being
/// different. `sigs` is read-only here (the combined plain-function set, prelude and program
/// alike, from whichever caller already resolved it) -- impl methods are never written into it
/// or into a `visibility` map; `ImplEntry` carries its own `origin`/`is_pub`, keyed by
/// `(method, receiver)` the same as the dispatch table itself, so one type's impl can never
/// clobber another same-named method's visibility the way a flat map would.
///
/// Validates, in order: every trait name is declared once (a duplicate first-match-wins the old
/// code silently allowed); every impl names a declared trait and a resolvable type, with no two
/// impls of the *same* trait for the *same* type; each method matches its trait's signature
/// exactly and every trait method has a body; no two impls give the *same* type a method of the
/// *same* name, regardless of trait (the actual bug fix, see above); no name is shared between
/// the plain-function namespace and any impl method (the cross-check the old flat-map write used
/// to get "for free" and this rewrite must restate explicitly); and, since the `::` -> `__`
/// backend escaping (`tir::escape_name`) is not provably injective on its own -- underscores in a
/// method or type name can make two distinct `(method, Type)` pairs escape identically -- no two
/// synthesized names collide once escaped, checked both against each other and against every
/// plain function name (escaping a name with no `::` is a no-op, so this is the same comparison
/// as asking whether a user's own function is literally named `add__Circle`).
/// One impl block against the trait it names: every method it gives a body matches that trait
/// method's signature exactly (`Self` substituted by the impl's own target), and every trait
/// method got a body -- neither direction optional. Pulled out of `collect_impls`'s main loop
/// since it is a complete, self-contained question ("does this impl actually implement this
/// trait?") independent of the dispatch table `collect_impls` builds from the answer.
///
/// `params` is the generic-impl parameter bindings in scope (`Type::Param` per declared name),
/// empty for a concrete impl -- both the impl's own signature and the trait's are resolved with
/// it in scope, since the impl's target type can carry a parameter into the trait's `Self`
/// positions too.
fn check_impl_matches_trait(
    imp: &ImplDecl,
    trait_decl: &TraitDecl,
    env: &TypeEnv,
    params: &HashMap<&str, &Type>,
) -> Result<(), Error> {
    for m in &imp.methods {
        let Some(tm) = trait_decl.methods.iter().find(|tm| tm.name == m.name) else {
            return Err(Error::new(
                m.span,
                format!("`{}` is not a method of trait `{}`", m.name, imp.trait_name),
            ));
        };
        let impl_param = m
            .param
            .as_ref()
            .map(|p| {
                resolve_with_params(&p.ty.substitute_self(&imp.ty), env, &mut Vec::new(), params)
            })
            .transpose()?;
        let impl_ret = resolve_with_params(
            &m.ret.substitute_self(&imp.ty),
            env,
            &mut Vec::new(),
            params,
        )?;
        let trait_param = tm
            .param
            .as_ref()
            .map(|p| {
                resolve_with_params(&p.ty.substitute_self(&imp.ty), env, &mut Vec::new(), params)
            })
            .transpose()?;
        let trait_ret = resolve_with_params(
            &tm.ret.substitute_self(&imp.ty),
            env,
            &mut Vec::new(),
            params,
        )?;
        if impl_param != trait_param || impl_ret != trait_ret {
            let show = |p: &Option<Type>| p.as_ref().map_or("()".to_string(), |t| t.to_string());
            return Err(Error::new(
                m.span,
                format!(
                    "impl method `{}`'s signature does not match trait `{}`'s; found {} -> {}, \
                     expected {} -> {}",
                    m.name,
                    imp.trait_name,
                    show(&impl_param),
                    impl_ret,
                    show(&trait_param),
                    trait_ret,
                ),
            ));
        }
    }
    for tm in &trait_decl.methods {
        if !imp.methods.iter().any(|m| m.name == tm.name) {
            return Err(Error::new(
                imp.span,
                format!(
                    "impl of trait `{}` is missing method `{}`",
                    imp.trait_name, tm.name
                ),
            ));
        }
    }
    Ok(())
}

/// The concrete (non-generic) half of `collect_impls`'s per-impl handling: resolves `imp.ty` to
/// one exact type, synthesizes a `Def` per method (`Self` substituted, name mangled to
/// `"{method}::{Type}"`), and pushes both into the caller's accumulators. `seen_impls`/`defs`/
/// `meta` are `collect_impls`'s own running state, threaded through rather than returned, since
/// the duplicate-impl and same-type-same-method checks need to see every earlier impl already
/// processed, generic or not.
fn collect_concrete_impl(
    imp: ImplDecl,
    trait_decl: &TraitDecl,
    env: &TypeEnv,
    seen_impls: &mut Vec<(String, Type)>,
    defs: &mut Vec<Def>,
    meta: &mut Vec<(String, Type)>,
) -> Result<(), Error> {
    let self_ty = resolve(&imp.ty, env, &mut Vec::new())?;
    if seen_impls
        .iter()
        .any(|(tn, ty)| tn == &imp.trait_name && *ty == self_ty)
    {
        return Err(Error::new(
            imp.span,
            format!("`{}` is already implemented for {self_ty}", imp.trait_name),
        ));
    }
    seen_impls.push((imp.trait_name.clone(), self_ty.clone()));
    check_impl_matches_trait(&imp, trait_decl, env, &HashMap::new())?;
    for m in imp.methods {
        // Collision only when two impls target the *same concrete type* with the *same*
        // method name, regardless of trait: different types sharing a method name is fine,
        // which is the fix over the old flat-`sigs` write.
        if defs
            .iter()
            .any(|d| d.name == format!("{}::{}", m.name, self_ty.ident()))
        {
            return Err(Error::new(
                m.span,
                format!("`{}` is already implemented for {self_ty}", m.name),
            ));
        }
        let param = m.param.map(|p| Param {
            shape: p.shape,
            ty: p.ty.substitute_self(&imp.ty),
            span: p.span,
        });
        defs.push(Def {
            name: format!("{}::{}", m.name, self_ty.ident()),
            param,
            ret: Some(m.ret.substitute_self(&imp.ty)),
            body: m.body,
            span: m.span,
            is_pub: true,
            origin: imp.origin,
            hoisted: false,
        });
        meta.push((m.name, self_ty.clone()));
    }
    Ok(())
}

/// The generic half of `collect_impls`'s per-impl handling: `Self` resolves against `imp.ty`
/// exactly as a concrete impl's does, except every declared parameter binds to `Type::Param`
/// rather than a concrete argument -- the same registry-template shape `resolve_enum`'s own
/// `None`-args branch builds for a generic enum. Nothing here checks a method's body: that
/// happens only once a concrete dispatch instantiates it (`find_or_monomorphize`), since only
/// then is every parameter bound to something real to check against.
fn collect_generic_impl(
    imp: ImplDecl,
    trait_decl: &TraitDecl,
    env: &TypeEnv,
    seen_impls: &mut Vec<(String, Type)>,
) -> Result<GenericImplTemplate, Error> {
    check_type_params(&imp.params, &format!("impl {}", imp.trait_name))?;
    let bound: Vec<Type> = imp
        .params
        .iter()
        .map(|(p, _)| Type::Param(p.clone()))
        .collect();
    let params_map: HashMap<&str, &Type> = imp
        .params
        .iter()
        .map(|(p, _)| p.as_str())
        .zip(bound.iter())
        .collect();
    let self_ty = resolve_with_params(&imp.ty, env, &mut Vec::new(), &params_map)?;
    for (p, span) in &imp.params {
        if !type_mentions_param(&self_ty, p) {
            return Err(Error::new(
                *span,
                format!(
                    "type parameter `{p}` does not appear in `{self_ty}`, so a dispatch \
                     could never bind it"
                ),
            ));
        }
    }
    if seen_impls
        .iter()
        .any(|(tn, ty)| tn == &imp.trait_name && *ty == self_ty)
    {
        return Err(Error::new(
            imp.span,
            format!("`{}` is already implemented for {self_ty}", imp.trait_name),
        ));
    }
    seen_impls.push((imp.trait_name.clone(), self_ty.clone()));
    check_impl_matches_trait(&imp, trait_decl, env, &params_map)?;
    let origin = imp.origin;
    let mut methods = Vec::new();
    for m in imp.methods {
        let param_ty = m
            .param
            .as_ref()
            .map(|p| {
                resolve_with_params(
                    &p.ty.substitute_self(&imp.ty),
                    env,
                    &mut Vec::new(),
                    &params_map,
                )
            })
            .transpose()?;
        let ret_ty = resolve_with_params(
            &m.ret.substitute_self(&imp.ty),
            env,
            &mut Vec::new(),
            &params_map,
        )?;
        methods.push(GenericImplMethod {
            name: m.name,
            param: m.param,
            param_ty,
            ret_ty,
            body: m.body,
            span: m.span,
            origin,
        });
    }
    Ok(GenericImplTemplate {
        receiver: self_ty,
        methods,
    })
}

/// `collect_impls`'s three outputs, named so its signature does not trip clippy's
/// `type_complexity`: the concrete impls' synthesized `Def`s (still to be checked, same as any
/// plain function's), the dispatch table built from them, and every generic impl, still
/// templated.
type CollectedImpls = (Vec<Def>, Vec<ImplEntry>, Vec<GenericImplTemplate>);

fn collect_impls(
    traits: &[TraitDecl],
    impls: Vec<ImplDecl>,
    env: &TypeEnv,
    sigs: &HashMap<String, Sig>,
) -> Result<CollectedImpls, Error> {
    for (i, t) in traits.iter().enumerate() {
        if traits[..i].iter().any(|earlier| earlier.name == t.name) {
            return Err(Error::new(
                t.span,
                format!("trait `{}` is defined twice", t.name),
            ));
        }
    }
    let mut defs: Vec<Def> = Vec::new();
    // One entry per `defs` entry, in the same order: the method name (unmangled) and receiver
    // type each synthesized `Def` came from, since a `Def`'s own mangled name cannot be split
    // back into the two without the same non-injective ambiguity `tir::escape_name`'s own
    // collision check exists to catch (`ty.ident()` is not provably reversible either).
    let mut meta: Vec<(String, Type)> = Vec::new();
    // (trait name, target type) pairs already given an impl, for the duplicate-impl check. A
    // generic impl's (still templated) type lands here too, compared the same way -- structural
    // `Type` equality, which is exact on `Type::Param` names, so two generic impls of one trait
    // for `Vec<T>` collide regardless of what either calls its parameter, but a generic and a
    // concrete impl of the same trait that could in fact overlap once instantiated (`impl Fold
    // for Vec<Int>` beside `impl<T> Fold for Vec<T>`) are not caught here -- only at whichever
    // one a dispatch actually reaches first (`find_or_monomorphize`'s own collision check).
    let mut seen_impls: Vec<(String, Type)> = Vec::new();
    let mut generics: Vec<GenericImplTemplate> = Vec::new();
    for imp in impls {
        let Some(trait_decl) = traits.iter().find(|t| t.name == imp.trait_name) else {
            return Err(Error::new(
                imp.span,
                format!("trait `{}` is not declared", imp.trait_name),
            ));
        };
        if imp.params.is_empty() {
            collect_concrete_impl(imp, trait_decl, env, &mut seen_impls, &mut defs, &mut meta)?;
        } else {
            generics.push(collect_generic_impl(imp, trait_decl, env, &mut seen_impls)?);
        }
    }
    let impl_sigs = signatures(&defs, env)?;
    let table: Vec<ImplEntry> = defs
        .iter()
        .zip(meta)
        .map(|(d, (method, receiver))| ImplEntry {
            method,
            receiver,
            def_name: d.name.clone(),
            sig: impl_sigs[&d.name].clone(),
            origin: d.origin,
            is_pub: d.is_pub,
            span: d.span,
        })
        .collect();

    check_name_collisions(&defs, &table, &generics, sigs)?;
    Ok((defs, table, generics))
}

/// The three name-collision checks `collect_impls` needs once its dispatch table is built: the
/// plain-function-vs-trait-method cross-check (the old flat-map write got this "for free" as an
/// accidental collision; a `Vec`-scan table needs it stated outright), and the two escaped-name
/// checks -- impl-vs-plain-function and impl-vs-impl -- since the `::` -> `__` backend escaping
/// (`tir::escape_name`) is not provably injective on its own (underscores in a method or type
/// name can make two distinct `(method, Type)` pairs escape identically).
fn check_name_collisions(
    defs: &[Def],
    table: &[ImplEntry],
    generics: &[GenericImplTemplate],
    sigs: &HashMap<String, Sig>,
) -> Result<(), Error> {
    for entry in table {
        if sigs.contains_key(&entry.method) {
            return Err(Error::new(
                entry.span,
                format!(
                    "`{}` is already a plain function; a trait method cannot share its name",
                    entry.method
                ),
            ));
        }
    }
    // A generic impl's method name is checked the very same way, and eagerly: unlike the
    // same-concrete-type-and-method collision below, this does not depend on which type a
    // dispatch ever instantiates, so there is no reason to defer it to `find_or_monomorphize`.
    for template in generics {
        for m in &template.methods {
            if sigs.contains_key(&m.name) {
                return Err(Error::new(
                    m.span,
                    format!(
                        "`{}` is already a plain function; a trait method cannot share its name",
                        m.name
                    ),
                ));
            }
        }
    }
    // Escaping a name with no `::` is a no-op, so this is exactly asking whether the escaped
    // form already names a plain function.
    for d in defs {
        let escaped = crate::tir::escape_name(&d.name);
        if sigs.contains_key(&escaped) {
            return Err(Error::new(
                d.span,
                format!(
                    "impl method `{}` compiles to the identifier `{escaped}`, which collides \
                     with the plain function `{escaped}`",
                    d.name
                ),
            ));
        }
    }
    for (i, a) in defs.iter().enumerate() {
        let esc_a = crate::tir::escape_name(&a.name);
        for b in &defs[i + 1..] {
            if crate::tir::escape_name(&b.name) == esc_a {
                return Err(Error::new(
                    b.span,
                    format!(
                        "`{}` and `{}` both compile to the identifier `{esc_a}`, which no \
                         backend can tell apart",
                        a.name, b.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Checks a module's own `pub` declarations in isolation -- `prelude.toy`'s, at build time
/// (`build.rs`), before there is any file for them to be merged into and nothing yet calling any
/// of it. Every declaration is kept: reachability is the calling file's question, decided once
/// program and prelude are merged (`check` above, via `prune_unreachable`).
pub fn check_module(module: crate::ast::Module) -> Result<(Vec<tir::Func>, ty::Enums), Error> {
    let crate::ast::Module {
        defs,
        aliases,
        enums,
        traits,
        impls,
    } = module;
    let (env, enum_tys, variant_owners, mut sigs, visibility) =
        resolve_defs(&aliases, &enums, &defs, Origin::Prelude)?;
    let (impl_defs, impl_table, generic_templates) = collect_impls(&traits, impls, &env, &sigs)?;
    let input = RefCell::new(None);
    let inputs = RefCell::new(None);
    let lines_used = Cell::new(false);
    let dsv = RefCell::new(None);
    let next_local = Cell::new(0);
    let generic_synth: RefCell<Vec<ImplEntry>> = RefCell::new(Vec::new());
    let generic_funcs: RefCell<Vec<tir::Func>> = RefCell::new(Vec::new());
    let cells = Cells {
        input: &input,
        inputs: &inputs,
        lines_used: &lines_used,
        dsv: &dsv,
        next_local: &next_local,
        generic_templates: &generic_templates,
        generic_synth: &generic_synth,
        generic_funcs: &generic_funcs,
    };
    let ctx = cells.ctx(
        &sigs,
        &enum_tys,
        &variant_owners,
        &visibility,
        &impl_table,
        Origin::Prelude,
    );
    for (name, sig) in infer_hoisted(&ctx, defs.iter())? {
        sigs.insert(name, sig);
    }
    let ctx = cells.ctx(
        &sigs,
        &enum_tys,
        &variant_owners,
        &visibility,
        &impl_table,
        Origin::Prelude,
    );
    let mut funcs = check_defs(defs.iter().chain(impl_defs.iter()), &ctx)?;
    // A prelude helper that itself colon-dispatches on a generic impl monomorphizes here, the
    // same as a program body would -- see `check`'s own `generic_funcs.into_inner()`.
    funcs.extend(generic_funcs.into_inner());
    Ok((funcs, enum_tys))
}

/// A signature may spell Stream now, which un-does the trick the Lines design leaned on (a
/// return annotation could never match a body holding a stream), so what that trick guaranteed
/// for free is checked for real here. Called only when `def` takes a parameter: a nullary
/// function has none of this to check.
fn check_param(
    body: &Tir,
    param: &Param,
    lowered: &LoweredParam,
    param_ty: &Option<Type>,
    func_name: &str,
    body_span: Span,
) -> Result<(), Error> {
    match &param.shape {
        ParamShape::Name(name, _) => {
            if matches!(param_ty, Some(Type::Stream(_))) {
                check_linear(
                    body,
                    &StreamBinding::Param(name),
                    &format!("`{}`", name),
                    body_span,
                )?;
            }
            if !param_used(body, name) {
                return Err(Error::new(
                    param.span,
                    format!(
                        "parameter `{}` is never used; delete it from `{}`'s definition and its call sites",
                        name, func_name
                    ),
                ));
            }
        }
        // A destructured parameter's record type can never be a Stream (streams cannot live in
        // records), so nothing linear to check; each named field is instead held to the same
        // dead-code rule the plain parameter's name is, echoing what a match arm does to its
        // pattern's fields.
        ParamShape::Fields(fields) => {
            for (fname, fspan) in &fields.names {
                let (_, _, fid, _) = lowered
                    .field_locals
                    .iter()
                    .find(|(n, _, _, _)| n == fname)
                    .expect("the checker lowered every named field");
                if !local_used(body, *fid) {
                    let hint = if fields.rest {
                        "remove it from the pattern".to_string()
                    } else {
                        "remove it from the pattern and close it with `..`".to_string()
                    };
                    return Err(Error::new(
                        *fspan,
                        format!("`{fname}` is bound here but never used in the body; {hint}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The program's own body. A sink-shaped body -- the tail-pipeline `lhs |> callee` or a direct
/// sink call (`jsonlines(...)` or a call to a function declared `-> Sink`) -- is the one legal
/// place a `Sink` may be born besides a `Sink`-returning function's body, so it is recognized
/// ahead of `synth`, which refuses a sink everywhere else (the general position rule; see
/// `tail_pipe` and `sink_call`). Any other body is synthesized, and a `Sink` result that slips
/// past recognition can only mean a shape `synth` should have refused.
fn check_program_body(ctx: &Ctx, body: &Expr) -> Result<Tir, Error> {
    if matches!(body, Expr::TailPipe { .. }) {
        return tail_pipe(ctx, body);
    }
    if let Some(tir) = sink_call(ctx, body)? {
        return Ok(tir);
    }
    synth(ctx, body)
}

/// Whether `body` is a direct sink call -- `jsonlines(...)` or a call to a function declared
/// `-> Sink` -- and, if so, the `Sink`-typed Tir it checks to. The `|>` tail-pipeline is the
/// other way a sink is written (see `tail_pipe`); both are legal in the program's body and a
/// `Sink`-returning function's body, and everywhere else `synth` runs the general position
/// rule and a `Sink` that slips past here is a shape `synth` should have refused.
fn sink_call(ctx: &Ctx, body: &Expr) -> Result<Option<Tir>, Error> {
    let Expr::Call {
        func,
        func_span,
        arg,
        span,
    } = body
    else {
        return Ok(None);
    };
    if func == "jsonlines" {
        return Ok(Some(jsonlines_call(ctx, arg, *span)?));
    }
    if ctx.sigs.get(func).is_some_and(|sig| sig.ret == Type::Sink) {
        return Ok(Some(call(ctx, func, *func_span, arg, *span)?));
    }
    Ok(None)
}

/// `lhs |> callee`, the tail-pipeline marker: `callee` -- a sink -- applied to `lhs`. This is
/// one of the two legal places a `Sink` may be born (the program's body and a `Sink`-returning
/// function's body; the other is a direct sink call, see `sink_call`); everywhere else `synth`
/// runs the general position rule and a `Sink` that slips past here is a shape `synth` should
/// have refused. Only a `Sink`-typed callee is legal, which is the general rule the old
/// hand-checked `jsonlines` position (kantord/toylang#151, #154) folded into: `jsonlines` is
/// the one sink builtin, and every other callee must be a function whose declared return type
/// is `Sink`.
fn tail_pipe(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    let Expr::TailPipe {
        lhs,
        callee,
        callee_span,
        ..
    } = expr
    else {
        unreachable!("tail_pipe is only called on a `|>` node");
    };
    let value = synth(ctx, lhs)?;
    if callee == "jsonlines" {
        return jsonlines_arg(value, lhs.span());
    }
    let Some(sig) = ctx.sigs.get(callee.as_str()) else {
        return Err(Error::new(
            *callee_span,
            format!(
                "the callee of `|>` must be a sink, such as `jsonlines` or a function that \
                 returns Sink; `{callee}` is not one"
            ),
        ));
    };
    if sig.ret != Type::Sink {
        return Err(Error::new(
            *callee_span,
            format!(
                "the callee of `|>` must be a sink, but `{callee}` returns {}",
                sig.ret
            ),
        ));
    }
    let Some(param_ty) = &sig.param else {
        return Err(Error::new(
            *callee_span,
            format!("`{callee}` takes no argument, so there is nothing for `|>` to pass it"),
        ));
    };
    if value.ty != *param_ty {
        return Err(Error::new(
            lhs.span(),
            format!("`{callee}` needs {param_ty}, found {}", value.ty),
        ));
    }
    let value = conform(ctx, value, param_ty);
    Ok(Tir::new(
        Type::Sink,
        Kind::Call {
            func: callee.clone(),
            arg: Some(Box::new(value)),
        },
    ))
}

/// Every function name the language itself provides, and therefore reserves. `str`, `range`,
/// `chars`, and `i64` live in `builtin()`'s fixed table; `jsonlines`, `length`, `flatten`,
/// `tail`, `collect`, `fields`, `sort`, `reverse`, `sum`, `max`, `parse`, `pipe_through`, `first`, `any`, and
/// `all` are polymorphic and checked from `synth`/`expect_inner`'s own arms; `select` and `map`
/// rebind `.`,and `sort_by`/`max_by` do the same with an orderable projection. All
/// twenty-three are reserved the same way, and the docs harness (tests/docs.rs) reads this list
/// to insist each one has a reference page.
pub const BUILTIN_NAMES: [&str; 23] = [
    "all",
    "any",
    "chars",
    "collect",
    "first",
    "length",
    "fields",
    "flatten",
    "i64",
    "jsonlines",
    "map",
    "max",
    "max_by",
    "parse",
    "pipe_through",
    "range",
    "reverse",
    "select",
    "sort",
    "sort_by",
    "str",
    "sum",
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
                param: Some(Type::Int),
                ret: Type::Str,
            },
        ),
        "range" => (
            tir::Builtin::Range,
            Sig {
                param: Some(Type::Int),
                ret: Type::Stream(Box::new(Type::Int)),
            },
        ),
        "chars" => (
            tir::Builtin::Chars,
            Sig {
                param: Some(Type::Str),
                ret: vec_of(Type::Char),
            },
        ),
        // The one bridge between the integer types (kantord/toylang#83). Its argument is an
        // Int, so `i64(9000000000)` is refused like any other too-big Int literal: a wide
        // value enters as a literal only where an Int64 is already expected.
        "i64" => (
            tir::Builtin::IntToI64,
            Sig {
                param: Some(Type::Int),
                ret: Type::Int64,
            },
        ),
        _ => return None,
    })
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
/// `Type::elem`, which stays Vec-only so the reducers (`length`, `jsonlines` today) keep
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

/// Every builtin and special-cased call form (`select`, `map`, `length`, ...) is unary; only a
/// user-defined function can be nullary. This unwraps a call's optional argument for those
/// forms, turning a bare `name()` at one of these names into the same "not a function" shape as
/// any other arity mismatch would need, rather than a panic.
fn need_arg<'a>(arg: &'a Option<Box<Expr>>, func: &str, span: Span) -> Result<&'a Expr, Error> {
    arg.as_deref()
        .ok_or_else(|| Error::new(span, format!("`{func}` needs an argument")))
}

/// `expr` is a call to `name` carrying an argument, for `expect_inner`'s `collect`/`map`
/// special cases: a bare `name()` (no argument) is not one of these, and falls through to
/// `synth`, which owns the arity error for a name that is not actually nullary.
fn call_named<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Call { func, arg, .. } = expr else {
        return None;
    };
    if func != name {
        return None;
    }
    arg.as_deref()
}

/// `name`'s variant list at `args`. A thin wrapper over the shared re-derivation
/// (`ty::variants_of`), which the backends reach for too: the checker's own `Type::Enum` in
/// hand carries a placeholder wherever a self-reference sits behind a `Vec`
/// (`types::resolve_named`, kantord/toylang#76), so every consumer that actually needs the
/// variants comes through here instead, one layer at a time, exactly as deep as the program
/// itself ever navigates into the value.
fn enum_variants(ctx: &Ctx, name: &str, args: &[Type]) -> Vec<(String, Option<Type>)> {
    ty::variants_of(ctx.enums, name, args)
}

/// The prelude's `Opt<T>` at a concrete `T`: what every Opt-producing site -- the collapsing
/// index, `tail`, a partial guard chain -- wraps its result in. Instantiated from the
/// registry template rather than spelled out, so the checker never re-states the prelude's
/// declaration.
fn opt_of(ctx: &Ctx, inner: Type) -> Type {
    let template = ctx.enums.get("Opt").expect("the prelude declares Opt");
    let Type::Enum { args, .. } = template else {
        unreachable!("the registry holds enum types")
    };
    let Type::Param(p) = &args[0] else {
        unreachable!("Opt's template argument is its own parameter")
    };
    ty::substitute(template, &HashMap::from([(p.clone(), inner)]))
}

/// Match a template type (parameters possibly inside) against a concrete one, binding each
/// parameter to what it faces. Records match fields by name, not position, the same way type
/// equality does (kantord/toylang#60); enums match on name and arguments, since the variants
/// follow from those.
fn unify(template: &Type, concrete: &Type, map: &mut HashMap<String, Type>) -> bool {
    match (template, concrete) {
        (Type::Param(p), c) => match map.get(p) {
            Some(bound) => bound == c,
            None => {
                map.insert(p.clone(), c.clone());
                true
            }
        },
        (Type::Vec(a), Type::Vec(b)) | (Type::Stream(a), Type::Stream(b)) => unify(a, b, map),
        (Type::Record(a), Type::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(n, t)| {
                    b.iter()
                        .find(|(bn, _)| bn == n)
                        .is_some_and(|(_, bt)| unify(t, bt, map))
                })
        }
        (
            Type::Enum {
                name: n1, args: a1, ..
            },
            Type::Enum {
                name: n2, args: a2, ..
            },
        ) => n1 == n2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| unify(x, y, map)),
        _ => template == concrete,
    }
}

/// Build one variant of `enum_ty`, checking that the payload written matches the payload
/// declared: a unit variant takes none, a payload variant requires one.
///
/// `enum_ty` is a registry template for a generic enum reached by name, or a concrete type
/// when the position already knows one (an expectation, or a written annotation). A template
/// instantiates from the expectation when `expected` names the same enum; otherwise the
/// payload's synthesised type binds the parameters, and a variant that leaves any parameter
/// open -- every unit variant of a generic enum does -- is refused, the `[]` answer.
fn construct(
    ctx: &Ctx,
    enum_ty: &Type,
    variant: &str,
    variant_span: Span,
    payload: Option<&Expr>,
    expected: Option<&Type>,
    from_string: bool,
) -> Result<Tir, Error> {
    let Type::Enum { name, args, .. } = enum_ty else {
        unreachable!("construct is only called with an enum type")
    };
    let is_template = args.iter().any(|a| matches!(a, Type::Param(_)));
    let instantiated;
    let enum_ty = if !is_template {
        enum_ty
    } else if let Some(want @ Type::Enum { name: wanted, .. }) = expected
        && wanted == name
    {
        want
    } else {
        instantiated = infer_instantiation(ctx, enum_ty, variant, variant_span, payload)?;
        return Ok(instantiated);
    };
    let Type::Enum { name, args, .. } = enum_ty else {
        unreachable!("construct is only called with an enum type")
    };
    let variants = enum_variants(ctx, name, args);
    // A capitalized name is the matcher, legal only in a pattern; a value is built with the
    // lowercase constructor, which resolves to the declared (capitalized) variant. A string
    // literal naming a variant is the wire's spelling, so it is exempt -- see the caller.
    if !from_string && variant.chars().next().is_some_and(char::is_uppercase) {
        return Err(Error::new(
            variant_span,
            format!(
                "a constructor starts with a lowercase letter; `{variant}` is the matcher and \
                 matches only in a pattern, so write `{}` to build the value",
                constructor_of(variant)
            ),
        ));
    }
    let Some((variant_name, declared)) = variants
        .iter()
        .find(|(n, _)| n == variant || is_constructor_of(n, variant))
    else {
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
            // The wire carries the matcher's name, the declared variant (`Circle`), so a backend
            // reader and printer can agree on it without lowering; the lowercase `variant` is the
            // constructor, the source spelling that builds the value (gh:156).
            variant: variant_name.to_string(),
            payload,
        },
    ))
}

/// A generic enum's constructor met with no expectation: the payload is synthesised and its
/// type binds the parameters. The template's own display (`Pair<T>`) does the talking in the
/// errors, since it shows the reader exactly which names are still open.
fn infer_instantiation(
    ctx: &Ctx,
    template: &Type,
    variant: &str,
    variant_span: Span,
    payload: Option<&Expr>,
) -> Result<Tir, Error> {
    let Type::Enum { name, args, .. } = template else {
        unreachable!("construct is only called with an enum type")
    };
    let variants = enum_variants(ctx, name, args);
    if variant.chars().next().is_some_and(char::is_uppercase) {
        return Err(Error::new(
            variant_span,
            format!(
                "a constructor starts with a lowercase letter; `{variant}` is the matcher and \
                 matches only in a pattern, so write `{}` to build the value",
                constructor_of(variant)
            ),
        ));
    }
    let Some((variant_name, declared)) = variants
        .iter()
        .find(|(n, _)| n == variant || is_constructor_of(n, variant))
    else {
        return Err(Error::new(
            variant_span,
            format!("`{name}` has no variant `{variant}`"),
        ));
    };
    let (declared, expr) = match (declared, payload) {
        (Some(declared), Some(expr)) => (declared, expr),
        (None, Some(expr)) => {
            return Err(Error::new(
                expr.span(),
                format!("`{variant}` is a unit variant of `{name}` and takes no payload"),
            ));
        }
        (Some(want), None) => {
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
        (None, None) => {
            return Err(Error::new(
                variant_span,
                format!(
                    "cannot tell what `{template}` a bare `{variant}` builds; only a position \
                     that expects one can say"
                ),
            ));
        }
    };
    let built = synth(ctx, expr)?;
    let mut bindings = HashMap::new();
    if !unify(declared, &built.ty, &mut bindings) {
        return Err(Error::new(
            expr.span(),
            format!("expected {declared}, found {}", built.ty),
        ));
    }
    if let Type::Enum { args, .. } = template
        && let Some(open) = args.iter().find_map(|a| match a {
            Type::Param(p) if !bindings.contains_key(p) => Some(p),
            _ => None,
        })
    {
        return Err(Error::new(
            variant_span,
            format!(
                "`{variant}`'s payload does not say what `{template}`'s `{open}` is; only a \
                 position that expects one can"
            ),
        ));
    }
    Ok(Tir::new(
        ty::substitute(template, &bindings),
        Kind::EnumLit {
            variant: variant_name.to_string(),
            payload: Some(Box::new(built)),
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

/// `let <name> = <expr>` bindings stacked one per line, then the value expression that ends
/// the block (kantord/toylang#150): no `in` keyword to pair, just the sequence and the result.
/// The one place a `let` block appears is a function body, so this is reached only through
/// `check_defs`'s `expect_inner`, and the body's type is whatever the block's value expression is.
///
/// Later bindings see earlier ones: each value is checked in the scope built so far, and the name
/// is bound after, so `let a = a` reads the older `a`, the way any shadowing works. Every
/// binding gets a fresh `LocalId` and lowers to the same `Kind::Bind` a pipe does, so all seven
/// backends already know how to run one.
fn let_bind(
    ctx: &Ctx,
    bindings: &[(String, Expr)],
    body: &Expr,
    want: Option<&Type>,
) -> Result<Expected, Error> {
    let locals: Vec<LocalId> = bindings.iter().map(|_| ctx.fresh()).collect();
    let mut values: Vec<Tir> = Vec::new();
    let mut scope = ctx.scope.clone();
    for ((name, value), local) in bindings.iter().zip(&locals) {
        let inner = ctx.rebuild(scope.clone(), ctx.subject.clone());
        let value = synth(&inner, value)?;
        scope.push((name.clone(), value.ty.clone(), Some(*local)));
        values.push(value);
    }
    let body_ctx = ctx.rebuild(scope, ctx.subject.clone());
    let body = match want {
        Some(want) => expect_inner(&body_ctx, body, want)?,
        None => Expected::Synthesised(synth(&body_ctx, body)?),
    };
    let (checked, mut tir) = match body {
        Expected::Checked(tir) => (true, tir),
        Expected::Synthesised(tir) => (false, tir),
    };
    for (local, value) in locals.into_iter().zip(values).into_iter().rev() {
        let body_ty = tir.ty.clone();
        tir = Tir::new(
            body_ty,
            Kind::Bind {
                local,
                value: Box::new(value),
                body: Box::new(tir),
            },
        );
    }
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
    ctx: &Ctx,
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
    if let Type::Enum { name, args, .. } = subject_ty
        && !covered.is_empty()
        && enum_variants(ctx, name, args)
            .iter()
            .all(|(n, _)| covered.contains(n))
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
fn check_coverage(
    ctx: &Ctx,
    subject_ty: &Type,
    covered: &[String],
    span: Span,
) -> Result<(), Error> {
    let Type::Enum {
        name: enum_name,
        args,
        ..
    } = subject_ty
    else {
        unreachable!("a pattern arm was checked against an enum subject")
    };
    let missing: Vec<String> = enum_variants(ctx, enum_name, args)
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
    // Deliberately parameterized, not built: how `some`/`none` arms compose -- their
    // totality, and whether they flow through first-class matchers -- is what the pending
    // matcher-totality round decides (plans/opt-as-enum.md, "Open points, owned elsewhere").
    // Until it runs, `!` and the producers' own combinators are Opt's consumption surface.
    if subject_ty.as_opt().is_some() {
        return Err(Error::new(
            vspan,
            format!(
                "{subject_ty} cannot be matched by variant yet; how its arms compose is \
                 still being decided, so insist with `!` or keep the value whole"
            ),
        ));
    }
    let Type::Enum {
        name: enum_name,
        args,
        ..
    } = subject_ty
    else {
        return Err(Error::new(
            vspan,
            format!("a match needs an enum subject, found {subject_ty}"),
        ));
    };
    let variants = enum_variants(ctx, enum_name, args);
    let Some((_, payload_ty)) = variants.iter().find(|(n, _)| n == vname) else {
        // A lowercase name is the constructor, which builds a value; a pattern names the matcher.
        if variants.iter().any(|(n, _)| is_constructor_of(n, vname)) {
            return Err(Error::new(
                vspan,
                format!(
                    "`{vname}` is the constructor, not a matcher; a pattern names the matcher, \
                     so write `{}`",
                    matcher_of(vname)
                ),
            ));
        }
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
        check_reachable(ctx, &subject_ty, &covered, default_seen, arm.span)?;
        let (variant, guard, payload, arm_ctx, fields) = match &arm.pattern {
            Pattern::Default { .. } => {
                default_seen = true;
                // A default matched the whole subject, so `.` stays the enum value.
                (None, None, None, ctx.with(ctx.subject.clone()), None)
            }
            // The guard is a Bool over the unrebound subject, so it is checked in the
            // enclosing context, and the body keeps `.` as the subject too.
            Pattern::Guard(g) => {
                let cond = expect(ctx, g, &Type::Bool)?;
                (None, Some(cond), None, ctx.with(ctx.subject.clone()), None)
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
                (Some(vname.clone()), None, payload, arm_ctx, fields.as_ref())
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
        // A bound field the arm's body never reads is the same dead-code smell as an
        // unused parameter; the fix is the same one the grammar already offers, naming
        // only what is read and closing the rest with `..`.
        if let (Some(fields), Some(pid)) = (fields, payload) {
            for (fname, fspan) in &fields.names {
                if !field_used(&body, pid, fname) {
                    let hint = if fields.rest {
                        "remove it from the pattern".to_string()
                    } else {
                        "remove it from the pattern and close it with `..`".to_string()
                    };
                    return Err(Error::new(
                        *fspan,
                        format!("`{fname}` is bound here but never used in the arm's body; {hint}"),
                    ));
                }
            }
        }
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
        check_coverage(ctx, &subject_ty, &covered, span)?;
    }
    let result = result.expect("the parser produces at least one arm");
    // Open-world half: a pure guard chain with no default may decline every arm, so its
    // result is `Opt` -- of whatever the arms yield, `Opt`-typed arms included. A declined
    // chain is `none` and a matched-but-absent arm is `some(none)`, two different values now
    // that absence is tagged (kantord/toylang#62); both still print `null`, which is
    // serialization's documented lossiness, not a conflation to refuse.
    let partial = !has_pattern_arm && !default_seen;
    let result = if partial { opt_of(ctx, result) } else { result };
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

/// `Msg(Ping -> "ping" or Quit -> "quit")` (gh:152): a `Match` over the subject `.`, with the
/// named enum asserted as the subject's type. The arms are the same `or`-chain a `Match`
/// carries; only the head differs. Resolution is delegated to `match_chain` once the subject is
/// confirmed to be the enum the name spells, which is what makes the whole call form something
/// the checker can reason about rather than a type name that happened to be called.
fn match_call(
    ctx: &Ctx,
    enum_name: &str,
    enum_span: Span,
    arms: &[MatchArm],
    span: Span,
) -> Result<Tir, Error> {
    if !ctx.enums.contains_key(enum_name) {
        return Err(Error::new(enum_span, format!("unknown type `{enum_name}`")));
    }
    let Some((subject_ty, _)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "a match needs a subject, so it must follow `|`".to_string(),
        ));
    };
    let Type::Enum { name, .. } = &subject_ty else {
        return Err(Error::new(
            enum_span,
            format!("`{enum_name}` names an enum, but the subject is {subject_ty}"),
        ));
    };
    if name != enum_name {
        return Err(Error::new(
            enum_span,
            format!("`{enum_name}` is not the subject's type, which is {subject_ty}"),
        ));
    }
    match_chain(ctx, arms, span, None)
}

/// The body of `fn name = Msg(Ping -> ... or ...)` (gh:152): the implicit `.` parameter is the
/// named enum, the body is a match call over it, and the function's signature -- parameter type
/// and return both -- is whatever that resolves to. The parameter is bound as the subject and
/// arrives through a hidden name the body never sees (it reads the subject), so the call form's
/// `.` is real input flowing in, not a constant.
fn check_hoisted_def(ctx: &Ctx, def: &Def) -> Result<tir::Func, Error> {
    let Expr::MatchCall {
        enum_name,
        enum_span,
        arms,
        span,
    } = &def.body
    else {
        return Err(Error::new(
            def.span,
            format!(
                "`fn {} = ...` needs a match-call body that names the enum it matches, \
                 such as `Msg(Ping -> \"ping\" or Quit -> \"quit\")`",
                def.name
            ),
        ));
    };
    let Some(enum_ty) = ctx.enums.get(enum_name.as_str()) else {
        return Err(Error::new(
            *enum_span,
            format!("unknown type `{enum_name}`"),
        ));
    };
    let sid = ctx.fresh();
    let def_ctx = Ctx {
        sigs: ctx.sigs,
        enums: ctx.enums,
        variant_owners: ctx.variant_owners,
        scope: Vec::new(),
        arm_fields: Vec::new(),
        subject: Some((enum_ty.clone(), sid)),
        input: ctx.input,
        inputs: ctx.inputs,
        lines_used: ctx.lines_used,
        dsv: ctx.dsv,
        in_mapper: false,
        in_fn: Some(&def.name),
        visibility: ctx.visibility,
        file: ctx.file,
        next_local: ctx.next_local,
        impls: ctx.impls,
        generic_templates: ctx.generic_templates,
        generic_synth: ctx.generic_synth,
        generic_funcs: ctx.generic_funcs,
    };
    let body = match_call(&def_ctx, enum_name, *enum_span, arms, *span)?;
    let param_name = format!("__{}_param", def.name);
    let body = Tir::new(
        body.ty.clone(),
        Kind::Bind {
            local: sid,
            value: Box::new(Tir::new(enum_ty.clone(), Kind::Var(param_name.clone()))),
            body: Box::new(body),
        },
    );
    Ok(tir::Func {
        name: def.name.clone(),
        param: Some(param_name),
        param_ty: Some(enum_ty.clone()),
        body,
    })
}

/// Refine the provisional signatures of hoisted definitions (`fn name = expr`, gh:152) from
/// their bodies. `signatures` seeded each with the enum it matches as a stand-in return; no
/// call may rely on that, so this pass runs before any body is checked and replaces every one
/// with the body's actual type. `ctx` is a provisional context carrying those seed signatures.
fn infer_hoisted<'a>(
    ctx: &Ctx<'a>,
    defs: impl IntoIterator<Item = &'a Def>,
) -> Result<HashMap<String, Sig>, Error> {
    let mut refined = HashMap::new();
    for def in defs {
        if !def.hoisted {
            continue;
        }
        let func = check_hoisted_def(ctx, def)?;
        let sig = Sig {
            param: func.param_ty.clone(),
            ret: func.body.ty.clone(),
        };
        // The stream/sink invariants apply to the inferred signature too: `signatures`
        // skipped them for hoisted defs (the return was still the provisional enum), and
        // nothing has re-checked them since the real return was inferred (gh:152).
        check_sig_invariants(&def.name, def.span, &sig)?;
        refined.insert(def.name.clone(), sig);
    }
    Ok(refined)
}

fn synth(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    let tir = synth_inner(ctx, expr)?;
    // A sink is not a value, so it can exist only where `tail_pipe` or `sink_call` recognized a
    // sink position: the program's own body or a Sink-returning function's. Reaching `synth`
    // with a sink means it is nested where its output would be observed, which is the general
    // position rule.
    if tir.ty.contains_sink() {
        return Err(Error::new(
            expr.span(),
            "a sink is not a value, so it is legal only as the program's outermost expression \
             or a Sink-returning function's body"
                .to_string(),
        ));
    }
    Ok(tir)
}

fn synth_inner(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    match expr {
        Expr::Str { text, .. } => Ok(Tir::new(Type::Str, Kind::Str(text.clone()))),
        Expr::Stdin { span } => {
            if let Some(func) = ctx.in_fn {
                return Err(source_in_fn(*span, "stdin", func));
            }
            if ctx.in_mapper {
                return Err(Error::new(
                    *span,
                    "`stdin` cannot be read inside a mapper body, which runs once per element"
                        .to_string(),
                ));
            }
            if ctx.lines_used.get() {
                return Err(Error::new(
                    *span,
                    "`stdin` has already been read; there is only one stdin".to_string(),
                ));
            }
            ctx.lines_used.set(true);
            Ok(Tir::new(Type::Stream(Box::new(Type::Str)), Kind::Lines))
        }
        // `dsv` reads the same raw lines `lines` does and splits each on the delimiter, so it
        // shares every rule: refused in function and mapper bodies, read at most once, and its
        // type is fixed (`Vec<Vec<Str>>`) rather than borrowed from a position. `csv`/`tsv`
        // are this same node with the delimiter already fixed by the parser.
        Expr::Dsv { delim, span } => {
            if let Some(func) = ctx.in_fn {
                return Err(Error::new(
                    *span,
                    format!(
                        "`dsv` cannot be read inside `fn {func}`; a source is legal only in \
                         the program's own body, so take the value through a parameter"
                    ),
                ));
            }
            if ctx.in_mapper {
                return Err(Error::new(
                    *span,
                    "`dsv` cannot be read inside a mapper body, which runs once per element"
                        .to_string(),
                ));
            }
            // Every backend's split on an empty separator is its own undefined behaviour (Rust
            // panics, Python raises, jq matches nothing), so the empty delimiter is refused
            // rather than left to disagree.
            if delim.is_empty() {
                return Err(Error::new(
                    *span,
                    "`dsv`'s delimiter cannot be empty".to_string(),
                ));
            }
            if ctx.dsv.borrow().is_some() {
                return Err(Error::new(
                    *span,
                    "`dsv` has already been read; there is only one stdin".to_string(),
                ));
            }
            *ctx.dsv.borrow_mut() = Some(delim.clone());
            let field = Type::Vec(Box::new(Type::Str));
            Ok(Tir::new(
                Type::Vec(Box::new(field)),
                Kind::Dsv {
                    delim: delim.clone(),
                },
            ))
        }
        Expr::Int { value, span } => {
            // The literal is the one place a value could enter without meeting the 32-bit rule,
            // and four backends agreed on the wrong answer only because each held it in its own
            // wider representation until an operator wrapped it. Go refuses to compile such a
            // constant at all, which is what made the hole visible. A wider literal is not
            // guessed to be Int64 -- `expect_inner` resolves one only where an Int64 is
            // already expected, the same rule `[]` follows.
            if *value > i32::MAX as i64 {
                return Err(Error::new(
                    *span,
                    format!(
                        "integer `{value}` does not fit in Int, which is 32 bits; only a \
                         position that expects Int64 can hold it"
                    ),
                ));
            }
            Ok(Tir::new(Type::Int, Kind::Int(*value)))
        }

        // A float literal is exactly the binary64 it spells (ADR 0007): there is no width to
        // resolve and no range to check, so a position has nothing to contribute, unlike Int.
        Expr::Float { value, span } => {
            let _ = span;
            Ok(Tir::new(Type::Float, Kind::Float(*value)))
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
            if let Some((_, t, local)) = ctx.scope.iter().rev().find(|(n, _, _)| n == name) {
                return match local {
                    // A `let`-bound name reads the local it was bound to, not a `Var`: the
                    // backends' `Bind` only ever binds locals, so a bare name reference would dangle.
                    Some(id) => Ok(Tir::new(t.clone(), Kind::Local(*id))),
                    None => Ok(Tir::new(t.clone(), Kind::Var(name.clone()))),
                };
            }
            if let Some(owners) = ctx.variant_owners.get(name) {
                let enum_ty = sole_owner(ctx, name, owners, *span)?.clone();
                return construct(ctx, &enum_ty, name, *span, None, None, false);
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
            // A trait method is never reachable by its bare name (gh:174: colon-only), so the
            // plain "not defined" message would be misleading about what is actually wrong.
            if ctx.impls.iter().any(|e| e.method == *name) {
                return Err(Error::new(
                    *span,
                    format!(
                        "`{name}` is a trait method, not a value; write `receiver:{name}(...)` \
                         to call it"
                    ),
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

        // A `|>` node only ever appears as a program's or a Sink-returning function's body,
        // which `check_program_body` and `check_defs` handle before `synth` is reached. A `Sink`
        // result here is refused by `synth`'s wrapper with the general position rule, so this
        // arm exists for totality and is only reachable through a shape the parser cannot make.
        Expr::TailPipe { .. } => tail_pipe(ctx, expr),

        Expr::Field { .. } | Expr::Index { .. } | Expr::Slice { .. } | Expr::Unwrap { .. } => {
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

        // `x:foo(y)`: UFCS sugar for a plain function, or trait-method dispatch, decided by
        // which namespace `foo` names -- see `colon_call`.
        Expr::ColonCall {
            receiver,
            method,
            method_span,
            arg,
            span,
        } => colon_call(ctx, receiver, method, *method_span, arg, *span),

        // `v[]` with nothing after it is the identity: "keep every entry" is what a Vec (or
        // stream) already is, with no field/index/unwrap left to distribute across it.
        Expr::Project { .. } => {
            let access = access(ctx, expr)?;
            Ok(access.tir)
        }

        // `parse(stdin)` reads one value, so its first checked use fixes the type for the whole
        // program -- and a later use in a position that expects nothing can borrow what the
        // first use fixed. With no use typed yet, nothing says what it contains, and guessing
        // is what the annotation rule avoids. `parse(s)` on an ordinary string cannot
        // synthesise either: its result type comes only from the position it is checked in.
        Expr::Call {
            func,
            func_span,
            arg,
            span,
        } => call(ctx, func, *func_span, arg, *span),

        Expr::Match { arms, span } => match_chain(ctx, arms, *span, None),

        // A type name used as a match call (gh:152): sugar for a `Match` over `.`, resolved by
        // `match_call` against the enum the name spells, then down to the same `match_chain`
        // the ordinary `Match` runs.
        Expr::MatchCall {
            enum_name,
            enum_span,
            arms,
            span,
        } => match_call(ctx, enum_name, *enum_span, arms, *span),

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
                None,
                false,
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
                        format!(
                            "integer `-{value}` does not fit in Int, which is 32 bits; only a \
                             position that expects Int64 can hold it"
                        ),
                    ));
                }
                return Ok(Tir::new(Type::Int, Kind::Int(-value)));
            }
            if let Expr::Float { value, .. } = base.as_ref() {
                return Ok(Tir::new(Type::Float, Kind::Float(-value)));
            }
            // Synthesise first, so `-x` works at whichever integer width `x` already has; a
            // form that can only be checked (`-input` fixing input to Int) falls back to the
            // Int expectation this arm always applied.
            let inner = match synth(ctx, base) {
                Ok(inner) if matches!(inner.ty, Type::Int | Type::Int64 | Type::Float) => inner,
                Ok(inner) => {
                    return Err(Error::new(
                        base.span(),
                        format!("expected Int, found {}", inner.ty),
                    ));
                }
                Err(_) => expect(ctx, base, &Type::Int)?,
            };
            let width = inner.ty.clone();
            // A Float's zero has to be `Kind::Float(0.0)`, not `Kind::Int(0)` typed as Float:
            // every backend but JS keeps Int and Float in different representations (an i64 and
            // an LLVM double are not interchangeable bits the way JS's untyped numbers let this
            // slide), so a mismatched Kind here compiles on JS by accident and is a real bug on
            // any backend that tells the two apart (kantord/toylang#149, found building the
            // Native backend's Float support).
            let zero_kind = if width == Type::Float {
                Kind::Float(0.0)
            } else {
                Kind::Int(0)
            };
            let zero = Tir::new(width.clone(), zero_kind);
            let _ = span;
            Ok(Tir::new(
                width,
                Kind::Arith {
                    op: BinOp::Sub,
                    lhs: Box::new(zero),
                    rhs: Box::new(inner),
                },
            ))
        }

        Expr::Binary { op, lhs, rhs, .. } => binary(ctx, *op, lhs, rhs),

        // The connectives are the one place a subexpression may go unevaluated: `and` runs its
        // right side only when the left is true, `or` only when it is false. Nothing here has
        // to say so -- every backend emits its own short-circuiting operator -- but it is why
        // `x != 0 and 10 / x > 1` is a program and not a division by zero.
        Expr::Logic { op, lhs, rhs, .. } => {
            let lhs = expect(ctx, lhs, &Type::Bool)?;
            let rhs = expect(ctx, rhs, &Type::Bool)?;
            Ok(Tir::new(
                Type::Bool,
                Kind::Logic {
                    op: *op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            ))
        }

        Expr::Not { base, .. } => {
            let base = expect(ctx, base, &Type::Bool)?;
            Ok(Tir::new(Type::Bool, Kind::Not(Box::new(base))))
        }

        // A `let` block is only reachable as a function body, which is always checked against an
        // expected return type, so `expect_inner` owns it. `synth` has no want to hand the block's
        // value, so this arm is unreachable in practice but must exist for the match to be total.
        Expr::Let { bindings, body, .. } => match let_bind(ctx, bindings, body, None)? {
            Expected::Checked(_) => unreachable!("synth has no want, so nothing is Checked"),
            Expected::Synthesised(tir) => Ok(tir),
        },

        // A `@(path)` module-as-function routing arm is a new AST node with no parser wiring
        // yet, so nothing constructs it; the arm exists only for the match to be total.
        Expr::ModuleRoute { .. } => unreachable!("module routing: AST-only stub, not wired yet"),
    }
}

/// `synth`'s `Expr::Call` arm, pulled out on its own: a call is one of `select`/`map` (special
/// forms rebinding `.`), a fixed sink or polymorphic builtin, or an ordinary user function or
/// payload-constructor lookup, each with its own argument shape to check.
fn call(
    ctx: &Ctx,
    func: &str,
    func_span: Span,
    arg: &Option<Box<Expr>>,
    span: Span,
) -> Result<Tir, Error> {
    // `parse` is resolved by its position, so `synth` owns only the two shapes that need
    // no expectation: `parse(stdin)` borrowing the type an earlier checked use fixed, and
    // every other parse refusing outright (its result type comes only from a position).
    if func == "parse" {
        let arg = need_arg(arg, func, span)?;
        if matches!(arg, Expr::Stdin { .. }) {
            return match ctx.input.borrow().as_ref() {
                Some(ty) => Ok(Tir::new(ty.clone(), Kind::Input)),
                None => Err(Error::new(
                    arg.span(),
                    "cannot tell what `parse(stdin)` contains",
                )),
            };
        }
        return Err(Error::new(
            func_span,
            "cannot tell what `parse` produces; its result type comes from the position it is \
             checked in"
                .to_string(),
        ));
    }
    // `select` and `map` are not special syntax, only special names: they are ordinary calls
    // whose argument is checked with `.` rebound to the subject's element type instead of
    // evaluated in the enclosing scope, which no ordinary function needs and is why they cannot
    // be defined as one (see `signatures`).
    if func == "select" {
        return select_call(ctx, need_arg(arg, func, span)?, span);
    }
    // The one way to produce a new element value. `select` removes elements and a field access
    // reads a field; neither can turn a Vec<Int> into a Vec<Str>.
    if func == "map" {
        return map_call(ctx, need_arg(arg, func, span)?, span, None);
    }
    // `sort_by` and `max_by` order or reduce by a scalar projection, so they are subject-fed
    // like `map`/`select`: the projection's `.` is rebound to the subject's element type. Blocking
    // (ordering and maximum both need the whole Vec), so unlike `map` they take a Vec only, never
    // a stream.
    if func == "sort_by" {
        return sort_by_call(ctx, need_arg(arg, func, span)?, span);
    }
    if func == "max_by" {
        return max_by_call(ctx, need_arg(arg, func, span)?, span);
    }
    // The one sink builtin: a regular call now, typed `Sink`, and so subject to the same
    // general position rule every sink is -- `synth`'s wrapper refuses a `Sink` result except
    // where `sink_call` or the tail-pipeline `|>` (`tail_pipe`) born it. The old hand-checked
    // `jsonlines` position in `check_program_body` retired in favor of that rule.
    if func == "jsonlines" {
        return jsonlines_call(ctx, arg, span);
    }
    // `length`, `tail`, and `flatten` are polymorphic over the element type, the same reason
    // `jsonlines` is checked here rather than through `builtin()`'s fixed table.
    if func == "length" {
        return length_call(ctx, need_arg(arg, func, span)?);
    }
    // `None` on an empty Vec, the same way `Index` turns reaching past what's there into `Opt`
    // rather than a runtime failure.
    if func == "tail" {
        return tail_call(ctx, need_arg(arg, func, span)?);
    }
    // Flattens a Vec<Vec<T>> into a Vec<T>, the way jq's `add` flattens a list of arrays.
    // Joining a fixed, known count of Vecs is `+` (kantord/toylang#97); this is for when the
    // outer Vec's length is not known at the call site.
    if func == "flatten" {
        return flatten_call(ctx, need_arg(arg, func, span)?);
    }
    // The one exit a stream has: `Stream<T> -> Vec<T>`, polymorphic over the element type like
    // `length` and the others above, so the argument is synthesised first.
    if func == "collect" {
        return collect(ctx, need_arg(arg, func, span)?, None);
    }
    // `pipe_through({cmd, args, lines}})`, `{cmd: Str, args: Vec<Str>, lines: Stream<Str>} ->
    // `Stream<PipeLine>`: stdin's lines stream into a subprocess's stdin, and its stdout/stderr
    // lines come back tagged by origin. The one place a stream is legal inside a record: it is
    // consumed by the subprocess, never stored, so it is checked here rather than through the
    // record literal path, which refuses streams.

    if func == "pipe_through" {
        return pipe_through_call(ctx, arg, span);
    }
    // Polymorphic over which record, the same reason `length` is checked here: the return type
    // (`Vec<Str>`) is fixed, but the argument's shape is not.
    if func == "fields" {
        return fields_call(ctx, need_arg(arg, func, span)?);
    }
    // Blocking, one Vec in and one Vec out with no Stream instance (kantord/toylang#86, Q20 in
    // draft.md), so both are checked here rather than through `select`'s subject-context
    // mechanism.
    if func == "sort" {
        return sort_call(ctx, need_arg(arg, func, span)?);
    }
    if func == "reverse" {
        return reverse_call(ctx, need_arg(arg, func, span)?);
    }
    // The two reductions (kantord/toylang#140): `sum` folds `+` at the element width, `max`
    // returns `Opt` because an empty Vec has no maximum. Both are checked here rather than
    // through `builtin()`'s fixed table because the return type is the element type's, which
    // no fixed signature can express.
    if func == "sum" {
        return sum_call(ctx, need_arg(arg, func, span)?);
    }
    if func == "max" {
        return max_call(ctx, need_arg(arg, func, span)?);
    }
    // The three search cuts (draft.md#query-is-search): `first` takes any element type
    // the way `tail` does, `any` and `all` each take a Vec of Bool. All three are checked here
    // rather than through `builtin()`'s fixed table: their return types are the element type's
    // (or a fixed Bool), which no fixed signature can express.
    if func == "first" {
        return first_call(ctx, need_arg(arg, func, span)?);
    }
    if func == "any" {
        return any_call(ctx, need_arg(arg, func, span)?);
    }
    if func == "all" {
        return all_call(ctx, need_arg(arg, func, span)?);
    }
    if let Some((which, sig)) = builtin(func) {
        let param_ty = sig
            .param
            .as_ref()
            .expect("every builtin in the fixed table is unary");
        let arg = expect(ctx, need_arg(arg, func, span)?, param_ty)?;
        return Ok(Tir::new(
            sig.ret,
            Kind::Builtin {
                which,
                arg: Box::new(arg),
            },
        ));
    }
    let Some(sig) = ctx.sigs.get(func) else {
        // A payload constructor is ordinary application (the Q34 path), so `circle{r: 1}` lands
        // here; it resolves as a variant only after the function namespace declines.
        if let Some(owners) = ctx.variant_owners.get(func) {
            let enum_ty = sole_owner(ctx, func, owners, func_span)?.clone();
            return construct(ctx, &enum_ty, func, func_span, arg.as_deref(), None, false);
        }
        return Err(Error::new(func_span, format!("`{func}` is not a function")));
    };
    // A non-`pub` definition is a helper for its own file: it may be called from where it was
    // defined, and nowhere else. `file` is the caller's file, so a `Prelude`-origin private
    // helper is refused from the program's file while remaining legal inside prelude.toy -- the
    // same privacy a `pub` prelude function already kept from calling a private helper, now
    // stated as a per-call-site rule rather than by dropping the helper (gh:166).
    if let Some((origin, is_pub)) = ctx.visibility.get(func)
        && !is_pub
        && *origin != ctx.file
    {
        return Err(Error::new(
            func_span,
            format!("`{func}` is not `pub`, so it can only be called from its own file"),
        ));
    }
    let arg = call_arg(ctx, func, span, &sig.param, arg)?;
    Ok(Tir::new(
        sig.ret.clone(),
        Kind::Call {
            func: func.to_string(),
            arg,
        },
    ))
}

/// A user function's argument against its (optionally nullary) signature: the four arity/type
/// combinations a call and a signature can be in relative to each other.
fn call_arg(
    ctx: &Ctx,
    func: &str,
    span: Span,
    param_ty: &Option<Type>,
    arg: &Option<Box<Expr>>,
) -> Result<Option<Box<Tir>>, Error> {
    match (param_ty, arg) {
        (Some(param_ty), Some(arg)) => Ok(Some(Box::new(expect(ctx, arg, param_ty)?))),
        (None, None) => Ok(None),
        (Some(param_ty), None) => Err(Error::new(
            span,
            format!("`{func}` needs an argument of {param_ty}, but was called with none"),
        )),
        (None, Some(arg)) => Err(Error::new(
            arg.span(),
            format!("`{func}` takes no argument"),
        )),
    }
}

/// `x:foo(y)`. Two resolutions, decided by what `foo` names -- `colon_call_ufcs` for a plain
/// function, `colon_call_dispatch` for a trait method -- and `collect_impls`'s cross-check
/// already refuses a name shared by both, so the two cannot overlap. Plain call syntax can
/// never reach a trait method (gh:174: colon-only, no fallback either direction), so dispatch
/// is that method's only spelling, and a name that is neither a plain function nor a trait
/// method is simply not defined.
fn colon_call(
    ctx: &Ctx,
    receiver: &Expr,
    method: &str,
    method_span: Span,
    arg: &Option<Box<Expr>>,
    span: Span,
) -> Result<Tir, Error> {
    if ctx.sigs.contains_key(method) {
        return colon_call_ufcs(ctx, receiver, method, method_span, arg, span);
    }
    let is_trait_method = ctx.impls.iter().any(|e| e.method == method)
        || ctx
            .generic_templates
            .iter()
            .any(|t| t.methods.iter().any(|m| m.name == method));
    if is_trait_method {
        return colon_call_dispatch(ctx, receiver, method, method_span, arg);
    }
    Err(Error::new(
        method_span,
        format!("`{method}` is not defined"),
    ))
}

/// The plain-function half of `colon_call`: `x:foo()` desugars to `foo(x)`, the receiver
/// filling `foo`'s one argument slot. `x:foo(y)` -- receiver *and* a separate argument -- is a
/// checker error regardless of `foo`'s own arity: no unary function has room for both at once.
fn colon_call_ufcs(
    ctx: &Ctx,
    receiver: &Expr,
    method: &str,
    method_span: Span,
    arg: &Option<Box<Expr>>,
    span: Span,
) -> Result<Tir, Error> {
    let sig = &ctx.sigs[method];
    if let Some(y) = arg {
        return Err(Error::new(
            y.span(),
            format!(
                "`{method}` takes one argument, already filled by the receiver before `:`; it \
                 cannot also take a separate argument"
            ),
        ));
    }
    if let Some((origin, is_pub)) = ctx.visibility.get(method)
        && !is_pub
        && *origin != ctx.file
    {
        return Err(Error::new(
            method_span,
            format!("`{method}` is not `pub`, so it can only be called from its own file"),
        ));
    }
    let Some(param_ty) = &sig.param else {
        return Err(Error::new(
            span,
            format!("`{method}` takes no argument, so it cannot be called with a receiver"),
        ));
    };
    let arg_tir = expect(ctx, receiver, param_ty)?;
    Ok(Tir::new(
        sig.ret.clone(),
        Kind::Call {
            func: method.to_string(),
            arg: Some(Box::new(arg_tir)),
        },
    ))
}

/// `colon_call_dispatch`'s lookup, in order: an exact match in the static concrete-impl table
/// (unchanged from before generic impls existed -- a non-generic impl's dispatch never even
/// reaches the code below), then an exact match among generic impls already monomorphized by an
/// earlier dispatch (`ctx.generic_synth`, the memo table), then, only if neither has it, a
/// generic template whose receiver `unify`s against `receiver_ty` -- monomorphized here, for the
/// first time, by `monomorphize`.
fn find_or_monomorphize(
    ctx: &Ctx,
    method: &str,
    receiver_ty: &Type,
    span: Span,
) -> Result<Option<ImplEntry>, Error> {
    if let Some(e) = ctx
        .impls
        .iter()
        .find(|e| e.method == method && e.receiver == *receiver_ty)
    {
        return Ok(Some(e.clone()));
    }
    if let Some(e) = ctx
        .generic_synth
        .borrow()
        .iter()
        .find(|e| e.method == method && e.receiver == *receiver_ty)
    {
        return Ok(Some(e.clone()));
    }
    for template in ctx.generic_templates {
        let Some(method_idx) = template.methods.iter().position(|m| m.name == method) else {
            continue;
        };
        let mut bindings = HashMap::new();
        if unify(&template.receiver, receiver_ty, &mut bindings) {
            return Ok(Some(monomorphize(
                ctx,
                template,
                method_idx,
                &bindings,
                receiver_ty,
                span,
            )?));
        }
    }
    Ok(None)
}

/// Monomorphizes one generic impl method at the concrete `receiver` `find_or_monomorphize` just
/// unified against `template.receiver`: substitutes `bindings` through the method's templated
/// signature to get a concrete `Sig`, runs the same collision checks `collect_impls` runs
/// eagerly for a concrete impl (deferred to here because only now is the concrete type known),
/// then checks the body for the first time via `check_one_def` -- the same path a plain
/// top-level `fn` or a concrete impl method's body takes. The result is memoized into
/// `ctx.generic_synth`/`ctx.generic_funcs` so a second dispatch to the same `(method, receiver)`
/// pair, from anywhere in the program, reuses it rather than rechecking the body.
fn monomorphize(
    ctx: &Ctx,
    template: &GenericImplTemplate,
    method_idx: usize,
    bindings: &HashMap<String, Type>,
    receiver: &Type,
    span: Span,
) -> Result<ImplEntry, Error> {
    let gm = &template.methods[method_idx];
    let def_name = format!("{}::{}", gm.name, receiver.ident());
    if let Some(existing) = ctx
        .generic_synth
        .borrow()
        .iter()
        .find(|e| e.def_name == def_name)
    {
        return Ok(existing.clone());
    }
    // The same collision `collect_impls` refuses eagerly for a concrete impl (`` `{method}` is
    // already implemented for {type}` ``), checked here instead because only a concrete
    // `receiver` -- known only once a dispatch reaches it -- can produce `def_name` at all.
    if ctx.impls.iter().any(|e| e.def_name == def_name) {
        return Err(Error::new(
            span,
            format!("`{}` is already implemented for {receiver}", gm.name),
        ));
    }
    let escaped = tir::escape_name(&def_name);
    if ctx.sigs.contains_key(&escaped) {
        return Err(Error::new(
            span,
            format!(
                "impl method `{def_name}` compiles to the identifier `{escaped}`, which \
                 collides with the plain function `{escaped}`"
            ),
        ));
    }
    let escaped_collision = ctx
        .impls
        .iter()
        .map(|e| &e.def_name)
        .chain(ctx.generic_synth.borrow().iter().map(|e| &e.def_name))
        .any(|other| tir::escape_name(other) == escaped);
    if escaped_collision {
        return Err(Error::new(
            span,
            format!(
                "`{def_name}` and another impl both compile to the identifier `{escaped}`, \
                 which no backend can tell apart"
            ),
        ));
    }

    let param_ty = gm.param_ty.as_ref().map(|t| ty::substitute(t, bindings));
    let ret_ty = ty::substitute(&gm.ret_ty, bindings);
    let sig = Sig {
        param: param_ty,
        ret: ret_ty,
    };
    check_sig_invariants(&gm.name, gm.span, &sig)?;
    let func = check_one_def(ctx, &def_name, gm.param.as_ref(), &sig, &gm.body)?;
    ctx.generic_funcs.borrow_mut().push(func);
    let entry = ImplEntry {
        method: gm.name.clone(),
        receiver: receiver.clone(),
        def_name,
        sig,
        origin: gm.origin,
        is_pub: true,
        span: gm.span,
    };
    ctx.generic_synth.borrow_mut().push(entry.clone());
    Ok(entry)
}

/// The trait-method half of `colon_call`: `(method, receiver's concrete type)` is looked up in
/// `ctx.impls`; the receiver's role is to select the impl (substituted as `Self`), not
/// necessarily to supply the underlying function's argument -- `y`, if present, is passed as
/// that argument instead, and only when `y` is absent does the receiver itself fill it (the
/// shape every colon-called nullary trait method, like `c:area()`, needs to work at all).
fn colon_call_dispatch(
    ctx: &Ctx,
    receiver: &Expr,
    method: &str,
    method_span: Span,
    arg: &Option<Box<Expr>>,
) -> Result<Tir, Error> {
    let receiver_tir = synth(ctx, receiver)?;
    let Some(entry) = find_or_monomorphize(ctx, method, &receiver_tir.ty, receiver.span())? else {
        return Err(Error::new(
            receiver.span(),
            format!("no impl provides `{method}` for {}", receiver_tir.ty),
        ));
    };
    if !entry.is_pub && entry.origin != ctx.file {
        return Err(Error::new(
            method_span,
            format!("`{method}` is not `pub`, so it can only be called from its own file"),
        ));
    }
    let arg_tir = match (&entry.sig.param, arg) {
        (Some(param_ty), Some(y)) => Some(Box::new(expect(ctx, y, param_ty)?)),
        (Some(param_ty), None) => Some(Box::new(expect(ctx, receiver, param_ty)?)),
        (None, None) => None,
        (None, Some(y)) => {
            return Err(Error::new(
                y.span(),
                format!("`{method}` takes no argument"),
            ));
        }
    };
    Ok(Tir::new(
        entry.sig.ret.clone(),
        Kind::Call {
            func: entry.def_name.clone(),
            arg: arg_tir,
        },
    ))
}

/// Cardinality-polymorphic: the same subject-context mechanism types `select` over a Vec and
/// over a Stream, with the element drawn from either's parameter. Stream in, stream out.
fn select_call(ctx: &Ctx, arg: &Expr, span: Span) -> Result<Tir, Error> {
    let Some((subject, id)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "`select` needs a subject, so it must follow `|`",
        ));
    };
    let Some(elem) = mapper_elem(&subject) else {
        return Err(Error::new(
            span,
            format!("`select` needs a Vec or a stream, found {subject}"),
        ));
    };
    let param = ctx.fresh();
    let inner = mapper_ctx(ctx, elem, param);
    let pred = expect(&inner, arg, &Type::Bool)?;
    let source = Tir::new(subject.clone(), Kind::Local(id));
    Ok(Tir::new(
        subject,
        Kind::Select {
            source: Box::new(source),
            param,
            pred: Box::new(pred),
        },
    ))
}

/// `pipe_through({cmd, args, lines}})`, `{cmd: Str, args: Vec<Str>, lines: Stream<Str>} ->
/// `Stream<PipeLine>`: stdin's lines stream into a subprocess's stdin,and its stdout/stderr
/// lines come back tagged by origin. The `lines` field is the one place a stream is legal inside a
/// record: it is consumed by the subprocess, never stored, so it is checked here rather than
/// through the record literal path, which refuses streams. `cmd` and `args` are ordinary values,
/// and the child's exit status is not an error: a filter like `grep` exits nonzero on "no
/// matches", which is a normal outcome for the shape this builtin exists to express.
fn pipe_through_call(ctx: &Ctx, arg: &Option<Box<Expr>>, span: Span) -> Result<Tir, Error> {
    let Some(arg) = arg else {
        return Err(Error::new(
            span,
            "`pipe_through` takes a record `{cmd, args, lines}`, but was called with no argument"
                .to_string(),
        ));
    };
    let arg_span = arg.span();
    let Expr::RecordLit { fields, .. } = arg.as_ref() else {
        let found = synth(ctx, arg)?;
        return Err(Error::new(
            arg_span,
            format!(
                "`pipe_through` needs a record `{{cmd: Str, args: Vec<Str>, lines: Stream<Str>}}`, \
                 found {}",
                found.ty
            ),
        ));
    };
    let (mut cmd, mut args, mut lines) = (None, None, None);
    for (name, name_span, value) in fields {
        match name.as_str() {
            "cmd" => cmd = Some(value),
            "args" => args = Some(value),
            "lines" => lines = Some(value),
            other => {
                return Err(Error::new(
                    *name_span,
                    format!(
                        "`pipe_through`'s record has no field `{other}`; it takes `cmd`, `args`,and `lines`"
                    ),
                ));
            }
        }
    }
    let (cmd, args, lines) = match (cmd, args, lines) {
        (Some(c), Some(a), Some(s)) => (c, a, s),
        _ => {
            let missing: Vec<&str> = ["cmd", "args", "lines"]
                .into_iter()
                .zip([cmd.is_some(), args.is_some(), lines.is_some()])
                .filter(|(_, present)| !present)
                .map(|(n, _)| n)
                .collect();
            return Err(Error::new(
                arg_span,
                format!(
                    "`pipe_through`'s record is missing `{}`",
                    missing.join(", `")
                ),
            ));
        }
    };
    let cmd = expect(ctx, cmd, &Type::Str)?;
    let args = expect(ctx, args, &Type::Vec(Box::new(Type::Str)))?;
    let lines = expect(ctx, lines, &Type::Stream(Box::new(Type::Str)))?;
    let pipeline = ctx
        .enums
        .get("PipeLine")
        .expect("the prelude declares PipeLine")
        .clone();
    Ok(Tir::new(
        Type::Stream(Box::new(pipeline)),
        Kind::Builtin {
            which: tir::Builtin::PipeThrough,
            arg: Box::new(Tir::new(
                Type::Record(vec![
                    ("cmd".to_string(), Type::Str),
                    ("args".to_string(), Type::Vec(Box::new(Type::Str))),
                    ("lines".to_string(), Type::Stream(Box::new(Type::Str))),
                ]),
                Kind::RecordLit {
                    fields: vec![
                        ("cmd".to_string(), cmd),
                        ("args".to_string(), args),
                        ("lines".to_string(), lines),
                    ],
                },
            )),
        },
    ))
}

/// `jsonlines(x)`, the one sink builtin, typed `Sink`. A sink is not a value, so a direct call
/// survives only where `sink_call` recognized the sink position (the program's body or a
/// `Sink`-returning function's); anywhere else `synth` reaches the general rule and refuses
/// the `Sink` result. The argument is a Vec or a Stream of elements with a wire form; Char has
/// none.
fn jsonlines_call(ctx: &Ctx, arg: &Option<Box<Expr>>, span: Span) -> Result<Tir, Error> {
    let Some(arg) = arg else {
        return Err(Error::new(
            span,
            "`jsonlines` needs a Vec or a stream, but was called with no argument".to_string(),
        ));
    };
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    jsonlines_arg(arg, arg_span)
}

/// The `jsonlines` sink's argument rules, applied to a value already synthesised: a Vec or a
/// Stream of elements with a wire form. Shared by the direct call and the `|>` tail-pipeline
/// (`x |> jsonlines`), which differ only in where the argument came from.
fn jsonlines_arg(arg: Tir, span: Span) -> Result<Tir, Error> {
    if !matches!(arg.ty, Type::Vec(_) | Type::Stream(_)) {
        return Err(Error::new(
            span,
            format!("`jsonlines` needs a Vec or a stream, found {}", arg.ty),
        ));
    }
    if arg.ty.contains_char() {
        return Err(Error::new(
            span,
            format!(
                "`jsonlines` cannot print {}; Char has no wire form to write",
                arg.ty
            ),
        ));
    }
    Ok(Tir::new(
        Type::Sink,
        Kind::Builtin {
            which: tir::Builtin::JsonLines,
            arg: Box::new(arg),
        },
    ))
}

fn length_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    if arg.ty.elem().is_none() {
        return Err(Error::new(
            arg_span,
            format!("`length` needs a Vec, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        Type::Int,
        Kind::Builtin {
            which: tir::Builtin::Length,
            arg: Box::new(arg),
        },
    ))
}

/// `fields(r)`, the declared names of `r`'s fields in the order its type carries them
/// (kantord/toylang#60: that order is metadata, not part of the type's identity, but it is
/// exactly the order this reads off). A record's field set is closed, so this needs no runtime
/// support once checked: every backend can bake the names in as a literal.
fn fields_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    if !matches!(arg.ty, Type::Record(_)) {
        return Err(Error::new(
            arg_span,
            format!("`fields` needs a record, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        Type::Vec(Box::new(Type::Str)),
        Kind::Builtin {
            which: tir::Builtin::Fields,
            arg: Box::new(arg),
        },
    ))
}

/// `None` on an empty Vec, the same way `Index` turns reaching past what's there into `Opt`
/// rather than a runtime failure.
fn tail_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem().cloned() else {
        return Err(Error::new(
            arg_span,
            format!("`tail` needs a Vec, found {}", arg.ty),
        ));
    };
    Ok(Tir::new(
        opt_of(ctx, Type::Vec(Box::new(elem))),
        Kind::Builtin {
            which: tir::Builtin::Tail,
            arg: Box::new(arg),
        },
    ))
}

/// Flattens a Vec<Vec<T>> into a Vec<T>, the way jq's `add` flattens a list of arrays.
fn flatten_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let inner = arg.ty.elem().cloned();
    let Some(elem) = inner.as_ref().and_then(Type::elem).cloned() else {
        return Err(Error::new(
            arg_span,
            format!("`flatten` needs a Vec of Vecs, found {}", arg.ty),
        ));
    };
    Ok(Tir::new(
        Type::Vec(Box::new(elem)),
        Kind::Builtin {
            which: tir::Builtin::Flatten,
            arg: Box::new(arg),
        },
    ))
}

/// The element types ordering comparisons (`<`, `<=`, `>`, `>=`) already typecheck on --
/// documented on each of their own reference pages, and what `sort` restricts itself to rather
/// than reaching past what every backend can already order natively.
fn orderable(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Int64 | Type::Str | Type::Char)
}

/// `sort(v)`, ascending by the total order `<` already gives `v`'s element type
/// (kantord/toylang#86). Restricted to `orderable` element types rather than the same
/// `expect(ctx, rhs, &left.ty)` equality `binary`'s comparison branch accepts more broadly, so
/// this never asks a backend to order a Record or an Enum.
fn sort_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem() else {
        return Err(Error::new(
            arg_span,
            format!("`sort` needs a Vec, found {}", arg.ty),
        ));
    };
    if !orderable(elem) {
        return Err(Error::new(
            arg_span,
            format!(
                "`sort` needs a Vec of Int, Int64, Str, or Char, found {}",
                arg.ty
            ),
        ));
    }
    let ty = arg.ty.clone();
    Ok(Tir::new(
        ty,
        Kind::Builtin {
            which: tir::Builtin::Sort,
            arg: Box::new(arg),
        },
    ))
}

/// `reverse(v)`, `v`'s elements in the opposite order. Unlike `sort`, no comparison is needed,
/// so every element type is accepted.
fn reverse_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    if arg.ty.elem().is_none() {
        return Err(Error::new(
            arg_span,
            format!("`reverse` needs a Vec, found {}", arg.ty),
        ));
    }
    let ty = arg.ty.clone();
    Ok(Tir::new(
        ty,
        Kind::Builtin {
            which: tir::Builtin::Reverse,
            arg: Box::new(arg),
        },
    ))
}

/// The element types a reduction is defined for: the two integer types. Neither Str nor Char
/// participates -- there is no caller for either -- so the restricted set is what a backend has
/// to spell (kantord/toylang#140, the ruling that cut min and product on the same grounds).
fn reducible(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Int64)
}

/// `sum(v)`, the reduction of `+` at the element type's width: `Vec<Int> -> Int`,
/// `Vec<Int64> -> Int64`. An empty Vec sums to 0, so unlike `max` the result is never `Opt`.
fn sum_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem() else {
        return Err(Error::new(
            arg_span,
            format!("`sum` needs a Vec of Int or Int64, found {}", arg.ty),
        ));
    };
    if !reducible(elem) {
        return Err(Error::new(
            arg_span,
            format!("`sum` needs a Vec of Int or Int64, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        elem.clone(),
        Kind::Builtin {
            which: tir::Builtin::Sum,
            arg: Box::new(arg),
        },
    ))
}

/// `max(v)`, the greatest element, `Opt<T>` because an empty Vec has no maximum -- the same
/// answer indexing gives to absence (kantord/toylang#140).
fn max_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem() else {
        return Err(Error::new(
            arg_span,
            format!("`max` needs a Vec of Int or Int64, found {}", arg.ty),
        ));
    };
    if !reducible(elem) {
        return Err(Error::new(
            arg_span,
            format!("`max` needs a Vec of Int or Int64, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        opt_of(ctx, elem.clone()),
        Kind::Builtin {
            which: tir::Builtin::Max,
            arg: Box::new(arg),
        },
    ))
}

/// `v | sort_by(.key)`, `v`'s entries ascending by the scalar the projection `body` reads off
/// each entry (gh:177), the same projection machinery `map` uses. Ties keep their original
/// order, a stable sort. Blocking like `sort`, so the subject is a Vec only -- never a stream --
/// and the projection is restricted to the same natively-ordered scalars `sort` takes, since a
/// backend orders by the key it projects.
fn sort_by_call(ctx: &Ctx, arg: &Expr, span: Span) -> Result<Tir, Error> {
    let Some((subject, id)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "`sort_by` needs a subject, so it must follow `|`",
        ));
    };
    let Some(elem) = subject.elem().cloned() else {
        return Err(Error::new(
            span,
            format!("`sort_by` needs a Vec, found {subject}"),
        ));
    };
    let param = ctx.fresh();
    let body = synth(&mapper_ctx(ctx, elem, param), arg)?;
    if !orderable(&body.ty) {
        return Err(Error::new(
            arg.span(),
            format!(
                "`sort_by`'s projection must be Int, Int64, Str, or Char, found {}",
                body.ty
            ),
        ));
    }
    let source = Tir::new(subject.clone(), Kind::Local(id));
    Ok(Tir::new(
        subject,
        Kind::SortBy {
            source: Box::new(source),
            param,
            body: Box::new(body),
        },
    ))
}

/// `v | max_by(.key)`, the entry whose projection `body` is greatest, `Opt<T>` because an empty
/// Vec has no maximum -- the same absence answer `max` gives (kantord/toylang#140). Ties keep the
/// first such entry, the way a stable maximum reads. Blocking like `max`, so the subject is a Vec
/// only, and the projection is restricted to the same natively-ordered scalars `sort` takes.
fn max_by_call(ctx: &Ctx, arg: &Expr, span: Span) -> Result<Tir, Error> {
    let Some((subject, id)) = ctx.subject.clone() else {
        return Err(Error::new(
            span,
            "`max_by` needs a subject, so it must follow `|`",
        ));
    };
    let Some(elem) = subject.elem().cloned() else {
        return Err(Error::new(
            span,
            format!("`max_by` needs a Vec, found {subject}"),
        ));
    };
    let param = ctx.fresh();
    let body = synth(&mapper_ctx(ctx, elem.clone(), param), arg)?;
    if !orderable(&body.ty) {
        return Err(Error::new(
            arg.span(),
            format!(
                "`max_by`'s projection must be Int, Int64, Str, or Char, found {}",
                body.ty
            ),
        ));
    }
    let source = Tir::new(subject.clone(), Kind::Local(id));
    Ok(Tir::new(
        opt_of(ctx, elem),
        Kind::MaxBy {
            source: Box::new(source),
            param,
            body: Box::new(body),
        },
    ))
}

/// `first(v)`, `Vec<T> -> Opt<T>`: the first entry, `None` on an empty Vec -- the cut that
/// commits to what you have and abandons the remaining alternatives. Generic over the element
/// type the way `tail` is, so it is checked here rather than through `builtin()`'s fixed table.
fn first_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem().cloned() else {
        return Err(Error::new(
            arg_span,
            format!("`first` needs a Vec, found {}", arg.ty),
        ));
    };
    Ok(Tir::new(
        opt_of(ctx, elem),
        Kind::Builtin {
            which: tir::Builtin::First,
            arg: Box::new(arg),
        },
    ))
}

/// The element type `any` and `all` are defined for: the one Bool. A Vec of anything else has
/// no truth value to reduce over, so it is refused the way `sum`'s restricted set is.
fn truthy(ty: &Type) -> bool {
    matches!(ty, Type::Bool)
}

/// `any(v)`, `Vec<Bool> -> Bool`: whether any entry is true. The existential cut: an empty Vec
/// has no true entry, so it is false.
fn any_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem() else {
        return Err(Error::new(
            arg_span,
            format!("`any` needs a Vec of Bool, found {}", arg.ty),
        ));
    };
    if !truthy(elem) {
        return Err(Error::new(
            arg_span,
            format!("`any` needs a Vec of Bool, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        Type::Bool,
        Kind::Builtin {
            which: tir::Builtin::Any,
            arg: Box::new(arg),
        },
    ))
}

/// `all(v)`, `Vec<Bool> -> Bool`: whether every entry is true. The universal cut: an empty Vec
/// has no false entry, so it is true (vacuously).
fn all_call(ctx: &Ctx, arg: &Expr) -> Result<Tir, Error> {
    let arg_span = arg.span();
    let arg = synth(ctx, arg)?;
    let Some(elem) = arg.ty.elem() else {
        return Err(Error::new(
            arg_span,
            format!("`all` needs a Vec of Bool, found {}", arg.ty),
        ));
    };
    if !truthy(elem) {
        return Err(Error::new(
            arg_span,
            format!("`all` needs a Vec of Bool, found {}", arg.ty),
        ));
    }
    Ok(Tir::new(
        Type::Bool,
        Kind::Builtin {
            which: tir::Builtin::All,
            arg: Box::new(arg),
        },
    ))
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
            let Some(inner) = b.elem.as_opt() else {
                return Err(Error::new(
                    *span,
                    format!("`!` needs an Opt, found {}", b.elem),
                ));
            };
            let ty = wrap(inner.clone(), b.depth, b.stream);
            let tir = Tir::new(
                ty,
                Kind::Unwrap {
                    base: Box::new(b.tir),
                },
            );
            Ok(Access::new(tir, inner.clone(), b.depth, b.stream))
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
            let out = opt_of(ctx, inner);
            let ty = wrap(out.clone(), b.depth, b.stream);
            let kind = Kind::Index {
                base: Box::new(b.tir),
                index: Box::new(index_tir),
                depth: b.depth,
                elem_is_record,
            };
            Ok(Access::new(Tir::new(ty, kind), out, b.depth, b.stream))
        }

        // Narrowing a dimension by position. Unlike a collapsing `[i]` the entry can never be
        // absent, so the answer is the dimension itself, not an `Opt`: out-of-range bounds
        // clamp jq-style rather than going missing (kantord/toylang#143). A `None` bound means
        // the dimension's own boundary.
        Expr::Slice {
            base,
            start,
            end,
            span,
        } => {
            let b = access(ctx, base)?;
            let Some(_) = b.elem.elem().cloned() else {
                return Err(Error::new(
                    *span,
                    format!("`[a:b]` needs a dimension, found {}", b.elem),
                ));
            };
            let start = match start {
                Some(s) => Some(Box::new(expect(ctx, s, &Type::Int)?)),
                None => None,
            };
            let end = match end {
                Some(e) => Some(Box::new(expect(ctx, e, &Type::Int)?)),
                None => None,
            };
            let kind = Kind::Slice {
                base: Box::new(b.tir),
                start,
                end,
                depth: b.depth,
            };
            let ty = wrap(b.elem.clone(), b.depth, b.stream);
            Ok(Access::new(Tir::new(ty, kind), b.elem, b.depth, b.stream))
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

/// A literal (or a minus on one, or a negation) met by an Int64 expectation: literals carry
/// no suffix, so the position is the only thing that can say which width one has
/// (kantord/toylang#83), the `[]` rule applied to numbers. A minus directly on a literal is
/// part of the literal, as for Int, so the whole written range short of
/// `-9223372036854775808` is reachable (the parser refuses `9223372036854775808` before the
/// minus could claim it, the same edge Rust's own i64 literals have). `None` for any other
/// form -- or any other `want` -- which falls through to `expect_inner`'s remaining arms.
fn int64_resolved(ctx: &Ctx, expr: &Expr, want: &Type) -> Option<Result<Tir, Error>> {
    if *want != Type::Int64 {
        return None;
    }
    match expr {
        Expr::Int { value, .. } => Some(Ok(Tir::new(Type::Int64, Kind::Int(*value)))),
        Expr::Neg { base, .. } => {
            if let Expr::Int { value, .. } = base.as_ref() {
                return Some(Ok(Tir::new(Type::Int64, Kind::Int(-value))));
            }
            let inner = match expect(ctx, base, &Type::Int64) {
                Ok(inner) => inner,
                Err(e) => return Some(Err(e)),
            };
            let zero = Tir::new(Type::Int64, Kind::Int(0));
            Some(Ok(Tir::new(
                Type::Int64,
                Kind::Arith {
                    op: BinOp::Sub,
                    lhs: Box::new(zero),
                    rhs: Box::new(inner),
                },
            )))
        }
        _ => None,
    }
}

/// An arithmetic operand checked at the width the other side fixed, with the type mismatch
/// upgraded to name the bridge when the two integer types met: "expected Int, found Int64" is
/// true but leaves the reader to discover `i64` on their own. The re-synthesis in the error
/// path mirrors `stream_refusal`'s.
fn expect_int_width(ctx: &Ctx, rhs: &Expr, width: &Type, op: BinOp) -> Result<Tir, Error> {
    expect(ctx, rhs, width).map_err(|err| {
        let Ok(found) = synth(ctx, rhs) else {
            return err;
        };
        match (width, &found.ty) {
            (Type::Int, Type::Int64) => Error::new(
                rhs.span(),
                format!("`{op}` cannot mix Int and Int64; widen the Int side with `i64(...)`"),
            ),
            (Type::Int64, Type::Int) => Error::new(
                rhs.span(),
                format!("`{op}` cannot mix Int64 and Int; widen the Int side with `i64(...)`"),
            ),
            _ => err,
        }
    })
}

fn binary(ctx: &Ctx, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<Tir, Error> {
    let left = synth(ctx, lhs)?;

    // Q2 is open, so an operator over a Vec is rejected rather than being silently given
    // broadcast or zip semantics. Under C1 that restriction is ordinary typing: there is no
    // separate cardinality to check, because a Vec is just a type. `+` is the one exception:
    // it concatenates two Vecs of the same type now (kantord/toylang#97, the add-trait
    // reading), which settles that half of Q2 without deciding what any other operator means
    // over two Vecs.
    if left.ty.elem().is_some() && op != BinOp::Add {
        return Err(Error::new(
            lhs.span(),
            format!("`{op}` does not apply to {}", left.ty),
        ));
    }

    if op.is_comparison() {
        // A Vec one level down is the same open question a bare one is. `==` on a record whose
        // field is a Vec would have to answer whether that field compares as a whole value or
        // broadcasts, which is Q2, so structural equality stops at the first Vec rather than
        // settling it silently (kantord/toylang#95). The bare case is already gone above; this
        // catches the composite that carries one.
        if left.ty.contains_vec() {
            return Err(Error::new(
                lhs.span(),
                format!("`{op}` does not apply to {}", left.ty),
            ));
        }

        // Comparison never crosses the integer widths either: the sides must already agree,
        // and the mismatch names `i64` the same way arithmetic's does.
        if matches!(left.ty, Type::Int | Type::Int64 | Type::Float) {
            let right = expect_int_width(ctx, rhs, &left.ty, op)?;
            return Ok(Tir::new(
                Type::Bool,
                Kind::Compare {
                    op,
                    lhs: Box::new(left),
                    rhs: Box::new(right),
                },
            ));
        }
        let right = match expect(ctx, rhs, &left.ty) {
            Ok(right) => right,
            Err(err) => {
                // Postfix `!` next to `!=` merges into one token: `x!=y` lexes as `!=`, not
                // `x! == y`. Once the plain comparison has failed, Opt<T> against its own
                // element is specific enough to name the likely cause instead of just the
                // type mismatch.
                if op == BinOp::Ne
                    && let Some(inner) = left.ty.as_opt().cloned()
                    && expect(ctx, rhs, &inner).is_ok()
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
        if !matches!(left.ty, Type::Int | Type::Int64 | Type::Float) {
            return Err(Error::new(
                lhs.span(),
                format!("expected Int, Int64, or Float, found {}", left.ty),
            ));
        }
        // Both sides share the left's width: nothing widens implicitly (kantord/toylang#83),
        // so an Int meeting an Int64 is an error naming the bridge rather than a silent
        // promotion -- and a bare literal on the right resolves at whichever width the left
        // already has, since `expect_inner` types a literal by its position.
        let width = left.ty.clone();
        let right = expect_int_width(ctx, rhs, &width, op)?;
        return Ok(Tir::new(
            width,
            Kind::Arith {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
            },
        ));
    }

    plus(ctx, lhs, left, rhs)
}

/// `+` is the one operator whose meaning depends on its operands: addition on Int or Int64,
/// concatenation on Str, and concatenation on Vec (kantord/toylang#97, the add-trait reading)
/// when both sides are the same Vec type. Nothing is coerced, so all three still require both
/// sides to agree.
fn plus(ctx: &Ctx, lhs: &Expr, left: Tir, rhs: &Expr) -> Result<Tir, Error> {
    match &left.ty {
        Type::Int | Type::Int64 | Type::Float => {
            let width = left.ty.clone();
            let right = expect_int_width(ctx, rhs, &width, BinOp::Add)?;
            let kind = Kind::Arith {
                op: BinOp::Add,
                lhs: Box::new(left),
                rhs: Box::new(right),
            };
            Ok(Tir::new(width, kind))
        }
        Type::Str => {
            let right = expect(ctx, rhs, &Type::Str)?;
            Ok(Tir::new(
                Type::Str,
                Kind::Concat(Box::new(left), Box::new(right)),
            ))
        }
        Type::Vec(_) => {
            let want = left.ty.clone();
            let right = expect(ctx, rhs, &want)?;
            Ok(Tir::new(
                want,
                Kind::Concat(Box::new(left), Box::new(right)),
            ))
        }
        other => Err(Error::new(
            lhs.span(),
            format!("`+` needs Int, Str, or Vec, found {other}"),
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
                    format!("expected {want}, found {}", found.ty),
                ));
            }
            Ok(reorder_record(ctx, found, want))
        }
    }
}

/// `found`, rebuilt to `want`'s field order when the two already agree (kantord/toylang#60) --
/// a no-op when they don't, leaving a real mismatch for the caller's own comparison to catch
/// and name.
fn conform(ctx: &Ctx, found: Tir, want: &Type) -> Tir {
    if &found.ty == want {
        reorder_record(ctx, found, want)
    } else {
        found
    }
}

/// Whether `a` and `b` are not just the same type (kantord/toylang#60: a record's fields are a
/// set, not a sequence) but physically identical -- same fields, same order, all the way into
/// any field that is itself a record. Every non-nominal backend reads a record by name and does
/// not care; the native backend and the columnar Vec layout read it by position, off whichever
/// type a value is currently checked against, so a value crossing into a differently-ordered
/// but equal position needs rebuilding before those readers see it. This is what decides
/// whether that rebuild is needed.
///
/// Also recurses into an enum's variant payloads (kantord/toylang#66): two instantiations of the
/// same generic enum are one `Type` (`Type::Enum`'s `PartialEq` compares `name` and `args`
/// only, and a generic argument's own order-insensitivity comes from `Type::Record`'s), so a
/// payload written in a different field order at each call site is exactly the same hazard a
/// bare record or a Vec element is. The variant list itself is never out of order -- it comes
/// from one declaration -- so zipping the two enums' variants positionally is safe, *except*
/// when one side's list is a recursive self-reference's placeholder (kantord/toylang#76): the
/// zip then runs zero pairs and this reports "no reorder needed" rather than crashing, which is
/// conservatively correct (nothing gets left mis-ordered that this function could have fixed)
/// though not necessarily complete (a reorder a full expansion would have found stays undone).
fn same_field_order(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Record(x), Type::Record(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((n1, t1), (n2, t2))| n1 == n2 && same_field_order(t1, t2))
        }
        (Type::Vec(x), Type::Vec(y)) | (Type::Stream(x), Type::Stream(y)) => same_field_order(x, y),
        (Type::Enum { variants: vx, .. }, Type::Enum { variants: vy, .. }) => {
            vx.iter().zip(vy).all(|((_, px), (_, py))| match (px, py) {
                (Some(tx), Some(ty)) => same_field_order(tx, ty),
                _ => true,
            })
        }
        _ => true,
    }
}

/// Rebuilds `found` -- already known equal to `want` -- so it is physically laid out in
/// `want`'s field order: a bound local read back out field by field into a fresh literal in
/// that order, recursing into any field that is itself a differently-ordered record. Also
/// recurses through a Vec or Stream wrapper, via a real `map` that rebuilds every element --
/// unlike the record case this is a runtime loop, not a free relabelling, since a Vec's
/// elements are not reachable one at a time the way a record's fields are (kantord/toylang#64).
///
/// A general enum reorders through `Match`/`EnumLit`: every arm rebuilds its variant with a
/// freshly reordered payload, which every backend can already run since it is exactly how a
/// user-written match over that enum already compiles. Opt is the one enum this can't reach
/// (kantord/toylang#66) -- three backends keep Opt in a representation of their own (`Option<T>`,
/// `tlOpt[T]{ok,v}`, the null-or-boxed native encoding) rather than the general tagged shape, so
/// `Match`'s tag-indexed codegen does not apply to it. `OptMap` is the dedicated node every
/// backend already has the pieces to run: it is exactly the presence branch each one's unwrap
/// and printer already take, generalised to rebuild the payload instead of rendering it.
fn reorder_enum(ctx: &Ctx, found: Tir, want: &Type) -> Tir {
    if let Some(want_inner) = want.as_opt() {
        let found_inner = found
            .ty
            .as_opt()
            .expect("found.ty == want: both Opt")
            .clone();
        let param = ctx.fresh();
        let body = reorder_record(ctx, Tir::new(found_inner, Kind::Local(param)), want_inner);
        return Tir::new(
            want.clone(),
            Kind::OptMap {
                source: Box::new(found),
                param,
                body: Box::new(body),
            },
        );
    }
    let Type::Enum {
        variants: want_variants,
        ..
    } = want
    else {
        unreachable!("reorder_enum is only called with an enum want")
    };
    let found_variants = match &found.ty {
        Type::Enum { variants, .. } => variants.clone(),
        _ => unreachable!("found.ty == want: both enums"),
    };
    let arms = want_variants
        .iter()
        .zip(&found_variants)
        .map(|((vname, want_payload), (_, found_payload))| {
            let payload = match (want_payload, found_payload) {
                (Some(want_p), Some(found_p)) => {
                    let pid = ctx.fresh();
                    let rebuilt =
                        reorder_record(ctx, Tir::new(found_p.clone(), Kind::Local(pid)), want_p);
                    (Some(pid), Some(Box::new(rebuilt)))
                }
                (None, None) => (None, None),
                _ => unreachable!("found.ty == want: variant shapes line up"),
            };
            tir::MatchArm {
                variant: Some(vname.clone()),
                guard: None,
                payload: payload.0,
                body: Tir::new(
                    want.clone(),
                    Kind::EnumLit {
                        variant: vname.clone(),
                        payload: payload.1,
                    },
                ),
            }
        })
        .collect();
    Tir::new(
        want.clone(),
        Kind::Match {
            subject: Box::new(found),
            arms,
            partial: false,
        },
    )
}
fn reorder_record(ctx: &Ctx, found: Tir, want: &Type) -> Tir {
    if same_field_order(&found.ty, want) {
        return found;
    }
    match want {
        Type::Record(want_fields) => {
            // A bare local can supply its own fields directly without being bound again: a
            // Field read reaches through it (a struct-of-arrays cursor, when it is a Vec
            // element, or an ordinary bound value otherwise) without ever asking for the whole
            // local as a value, which is what a redundant rebind here would do -- and which
            // the native backend refuses for a cursor, since a map body's element local is
            // never materialised as one.
            if let Kind::Local(local) = found.kind {
                return Tir::new(
                    want.clone(),
                    Kind::RecordLit {
                        fields: reorder_fields(ctx, local, &found.ty, want_fields),
                    },
                );
            }
            let local = ctx.fresh();
            let found_ty = found.ty.clone();
            let fields = reorder_fields(ctx, local, &found_ty, want_fields);
            Tir::new(
                want.clone(),
                Kind::Bind {
                    local,
                    value: Box::new(found),
                    body: Box::new(Tir::new(want.clone(), Kind::RecordLit { fields })),
                },
            )
        }
        Type::Vec(want_elem) | Type::Stream(want_elem) => {
            // `found.ty == want` already (the caller's own guard), so its wrapper is the same
            // kind as `want`'s and `runtime_elem` cannot miss.
            let elem_ty = tir::runtime_elem(&found.ty)
                .expect("want is Vec/Stream and found.ty == want")
                .clone();
            let param = ctx.fresh();
            let body = reorder_record(ctx, Tir::new(elem_ty, Kind::Local(param)), want_elem);
            Tir::new(
                want.clone(),
                Kind::Map {
                    source: Box::new(found),
                    param,
                    body: Box::new(body),
                },
            )
        }
        Type::Enum { .. } => reorder_enum(ctx, found, want),
        _ => found,
    }
}

/// `want_fields`, each read off `local` (of type `local_ty`) in `want_fields`'s order, recursing
/// into any field that is itself a differently-ordered record.
fn reorder_fields(
    ctx: &Ctx,
    local: LocalId,
    local_ty: &Type,
    want_fields: &[(String, Type)],
) -> Vec<(String, Tir)> {
    want_fields
        .iter()
        .map(|(name, field_ty)| {
            let access = Tir::new(
                field_ty.clone(),
                Kind::Field {
                    base: Box::new(Tir::new(local_ty.clone(), Kind::Local(local))),
                    name: name.clone(),
                },
            );
            (name.clone(), reorder_record(ctx, access, field_ty))
        })
        .collect()
}

/// `expect`, minus the final comparison. Each arm here is a form whose type can come from its
/// position rather than its contents; expectation only ever resolves what synthesis would have
/// refused -- a form that can synthesise falls through and is compared, never coerced.
/// The one error a type has no ratified wire form to read back, reported in the order the
/// existing checks have always listed them: absence, then Char, then Int64. `None` means
/// the type has a wire form to read.
fn wire_form_error(ty: &Type) -> Option<&'static str> {
    if ty.contains_opt() {
        Some("absence has no wire form to read")
    } else if ty.contains_char() {
        Some("Char has no wire form to read")
    } else if ty.contains_int64() {
        Some("how an Int64 crosses the wire is not decided yet")
    } else {
        None
    }
}

/// `parse(stdin)` against the type its position wants: the checked `Stream<Str> -> T` overload,
/// lowered to the `Input` node every backend already reads. The first use fills the program-wide
/// slot, and every later use must agree with it. A signature can spell Stream now, so this
/// position can ask for one; `parse(stdin)` is a whole value already in hand, which is exactly
/// what a stream is not.
fn input_read(ctx: &Ctx, span: Span, want: &Type) -> Result<Tir, Error> {
    if want.contains_stream() {
        return Err(Error::new(
            span,
            format!("`parse(stdin)` is one value read from stdin, but {want} is wanted here"),
        ));
    }
    if let Some(reason) = wire_form_error(want) {
        return Err(Error::new(
            span,
            format!("`parse(stdin)` cannot be read as {want}; {reason}"),
        ));
    }
    let mut slot = ctx.input.borrow_mut();
    match slot.as_ref() {
        None => *slot = Some(want.clone()),
        Some(prev) if prev != want => {
            return Err(Error::new(
                span,
                format!("`parse(stdin)` is used as {prev} here and as {want} elsewhere"),
            ));
        }
        Some(_) => {}
    }
    Ok(Tir::new(want.clone(), Kind::Input))
}

/// `stdin | map(parse(.))` against the type its position wants, which must be a Stream: the
/// checked spelling `inputs` retired into, lowered to the `Inputs` node every backend already
/// reads. The filled slot doubles as the single-use flag: a second such pipeline would be a
/// second stream claiming the same real stdin, the same mistake a second `stdin` is.
fn inputs_read(ctx: &Ctx, span: Span, want: &Type) -> Result<Tir, Error> {
    if let Some(func) = ctx.in_fn {
        return Err(source_in_fn(span, "stdin", func));
    }
    if ctx.in_mapper {
        return Err(Error::new(
            span,
            "`stdin` cannot be read inside a mapper body, which runs once per element".to_string(),
        ));
    }
    let Type::Stream(elem) = want else {
        return Err(Error::new(
            span,
            format!(
                "`stdin | map(parse(.))` is a stream, but {want} is wanted here; eager use is \
                 spelled `collect(stdin | map(parse(.)))`"
            ),
        ));
    };
    if let Some(reason) = wire_form_error(elem) {
        return Err(Error::new(
            span,
            format!("`stdin | map(parse(.))` cannot be read as {want}; {reason}"),
        ));
    }
    let mut slot = ctx.inputs.borrow_mut();
    if slot.is_some() {
        return Err(Error::new(
            span,
            "`stdin` has already been read; there is only one stdin".to_string(),
        ));
    }
    *slot = Some((**elem).clone());
    Ok(Tir::new(want.clone(), Kind::Inputs))
}

/// `parse(x)`, checked against the type its position wants. `parse(stdin)` takes the checked
/// `Stream<Str> -> T` overload:one value read from stdin, lowered to the `Input` node.
/// Every other argument goes through the checked `Str -> T` overload:the string is read as
/// one JSON value of type `T`. The result type comes only from this position, so `synth` refuses
/// any parse it cannot resolve the same way it always refused an untyped `input`.
fn parse_call(ctx: &Ctx, arg: &Expr, span: Span, want: &Type) -> Result<Tir, Error> {
    if matches!(arg, Expr::Stdin { .. }) {
        return input_read(ctx, span, want);
    }
    if want.contains_stream() {
        return Err(Error::new(
            span,
            format!("`parse` produces one value, but {want} is wanted here"),
        ));
    }
    let str = expect(ctx, arg, &Type::Str)?;
    if let Some(reason) = wire_form_error(want) {
        return Err(Error::new(
            span,
            format!("`parse` cannot produce {want}; {reason}"),
        ));
    }
    Ok(Tir::new(
        want.clone(),
        Kind::Builtin {
            which: tir::Builtin::Parse,
            arg: Box::new(str),
        },
    ))
}

/// Whether `expr` is `stdin | map(parse(.))`, the spelling `inputs` retired into. The checker
/// recognizes the exact shape -- a pipe from the `stdin` source into a `map` whose body is
/// `parse(.)` -- and lowers it to the existing `Inputs` node, so every backend's eager reader
/// and fused loop keep working unchanged.
fn inputs_shape(expr: &Expr) -> Option<Span> {
    let Expr::Pipe { lhs, rhs, span } = expr else {
        return None;
    };
    let Expr::Stdin { .. } = lhs.as_ref() else {
        return None;
    };
    let Expr::Call { func, arg, .. } = rhs.as_ref() else {
        return None;
    };
    if func != "map" {
        return None;
    }
    let arg = arg.as_ref()?;
    let Expr::Call {
        func: inner,
        arg: inner_arg,
        ..
    } = arg.as_ref()
    else {
        return None;
    };
    if inner != "parse" {
        return None;
    }
    let inner_arg = inner_arg.as_ref()?;
    if !matches!(inner_arg.as_ref(), Expr::Subject { .. }) {
        return None;
    }
    Some(*span)
}

/// A constructor in a position that wants its enum takes the wanted instantiation: this is
/// how a generic enum's unit variant -- which alone says nothing about the arguments --
/// gets built at all (`fn f() -> Pair<Int> = empty`). A bare name only counts when nothing
/// closer claims it (an arm's field, a local); a call only when no function does. Anything
/// that is not the wanted enum's variant returns `None` and falls through to synthesis,
/// which owns the errors.
fn wanted_variant(ctx: &Ctx, expr: &Expr, want: &Type) -> Option<Result<Tir, Error>> {
    let Type::Enum { name, args, .. } = want else {
        return None;
    };
    let variants = enum_variants(ctx, name, args);
    let owns = |n: &str| {
        variants
            .iter()
            .any(|(vn, _)| vn == n || is_constructor_of(vn, n))
    };
    match expr {
        Expr::Var { name, span }
            if owns(name)
                && !ctx.arm_fields.iter().any(|(n, ..)| n == name)
                && !ctx.scope.iter().any(|(n, _, _)| n == name) =>
        {
            Some(construct(ctx, want, name, *span, None, None, false))
        }
        Expr::Call {
            func,
            func_span,
            arg,
            ..
        } if owns(func) && !ctx.sigs.contains_key(func) => Some(construct(
            ctx,
            want,
            func,
            *func_span,
            arg.as_deref(),
            None,
            false,
        )),
        Expr::Variant {
            enum_name,
            variant,
            variant_span,
            payload,
            ..
        } if matches!(want, Type::Enum { name, .. } if name == enum_name) => Some(construct(
            ctx,
            want,
            variant,
            *variant_span,
            payload.as_deref(),
            None,
            false,
        )),
        _ => None,
    }
}

/// What a match chain's arms should be checked against. A chain with a pattern arm is checked
/// against `want` itself; a partial guard chain -- all guards, no default -- wraps whatever the
/// arms yield in `Opt` on its own (kantord/toylang#62), so `want` peels through `Opt` first
/// (kantord/toylang#48). `None` when a partial chain's `want` isn't `Opt`, leaving synthesis to
/// own that mismatch.
fn match_arm_want<'a>(arms: &[MatchArm], want: &'a Type) -> Option<&'a Type> {
    if arms.iter().all(|a| matches!(a.pattern, Pattern::Guard(_))) {
        want.as_opt()
    } else {
        Some(want)
    }
}

fn expect_inner(ctx: &Ctx, expr: &Expr, want: &Type) -> Result<Expected, Error> {
    // The forms whose type comes from their position rather than their contents. `parse`
    // is resolved by what it is checked against: `parse(stdin)` is the checked
    // `Stream<Str> -> T` overload (one value read from stdin), and `stdin | map(parse(.))`
    // is the checked spelling `inputs` retired into. Both are lowered to the existing `Input`/
    // `Inputs` nodes, whose single-read slots and element-type fixing the backends already
    // rely on.
    if let Expr::Call {
        func, arg, span, ..
    } = expr
        && func == "parse"
    {
        let arg = need_arg(arg, "parse", *span)?;
        return parse_call(ctx, arg, *span, want).map(Expected::Checked);
    }

    if let Some(span) = inputs_shape(expr) {
        return inputs_read(ctx, span, want).map(Expected::Checked);
    }

    // `collect(inputs)` in a Vec-wanted position: the honest eager spelling the decision
    // records. `collect` is otherwise synthesised argument-first, but `inputs` has no type of
    // its own until checked, so the wanted Vec<T> is pushed back through as Stream<T> here.
    if let Some(arg) = call_named(expr, "collect")
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
        return construct(ctx, want, text, *span, None, None, true).map(Expected::Checked);
    }

    // An Int64-expected literal and a wanted enum's variant are the same kind of arm: a form
    // resolved by its position, built by its own helper, `None` falling through.
    if let Some(built) = int64_resolved(ctx, expr, want).or_else(|| wanted_variant(ctx, expr, want))
    {
        return built.map(Expected::Checked);
    }

    // The pipe's value is the right side's, so the expectation flows into the right side --
    // the only road into the subject-fed forms, which exist only after `|`.
    if let Expr::Pipe { lhs, rhs, .. } = expr {
        return pipe(ctx, lhs, rhs, Some(want));
    }

    // A `let` block's value is its final expression, so the expectation flows into that. The
    // bindings themselves synthesise, since a local binding has no position to resolve one from.
    if let Expr::Let { bindings, body, .. } = expr {
        return let_bind(ctx, bindings, body, Some(want));
    }

    // A `map` whose position expects a Vec (over a Vec subject) or a Stream (over a Stream
    // subject) pushes the expected element type into its body. Any other want, or no subject
    // at all, falls through to synthesis, which owns those errors.
    if let Expr::Call { span, .. } = expr
        && let Some(arg) = call_named(expr, "map")
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

    // Every arm of a total match chain receives the expectation the same way; a partial
    // chain's peel is `match_arm_want`'s call.
    if let Expr::Match { arms, span } = expr
        && !want.contains_stream()
        && let Some(push) = match_arm_want(arms, want)
    {
        return match_chain(ctx, arms, *span, Some(push)).map(Expected::Checked);
    }

    // A record literal checked against a record type pushes each field's expected type into
    // its value. Matching is by name, not position (kantord/toylang#60: {a: Int, b: Int} and
    // {b: Int, a: Int} are one type), so a mis-fielded literal -- wrong names, wrong count --
    // falls back to synthesis, where the existing errors say what is wrong. The built fields
    // are written in `want`'s order regardless of how the literal spelled them: declaration
    // order is what the printer and the columnar layouts key on, and both read it off the
    // type, not the source text.
    if let Expr::RecordLit { fields, .. } = expr
        && let Type::Record(want_fields) = want
        && fields.len() == want_fields.len()
        && want_fields
            .iter()
            .all(|(wanted, _)| fields.iter().any(|(name, _, _)| name == wanted))
    {
        let mut built = Vec::new();
        for (wanted, field_ty) in want_fields {
            let (name, _, value) = fields.iter().find(|(name, _, _)| name == wanted).unwrap();
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
