//! The native backend.
//!
//! Unlike Lua and JavaScript this does not end at a string of source. It ends at an object file,
//! which is not a program: linking is still someone else's job, so a native build shells out to
//! `cc`, against the runtime archive built from `runtime-rs`.
//!
//! Everything it cannot compile yet returns a named error rather than being silently absent, so
//! the gap between this and the other two backends is a visible, shrinking list.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValueEnum, FloatValue, FunctionValue, IntValue, PhiValue,
    PointerValue,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate, OptimizationLevel};

use crate::ast::{BinOp, LogicOp};
use crate::tir::{self, Builtin, Func, Fusion, Kind, LocalId, Program, Source, Stage, Tir};
use crate::ty::{self, Enums, Type};

fn unsupported(what: &str) -> String {
    format!("the native backend cannot compile {what} yet")
}

struct Runtime<'ctx> {
    concat: FunctionValue<'ctx>,
    int_to_str: FunctionValue<'ctx>,
    float_to_str: FunctionValue<'ctx>,
    str_eq: FunctionValue<'ctx>,
    str_cmp: FunctionValue<'ctx>,
    print: FunctionValue<'ctx>,
    quote: FunctionValue<'ctx>,
    join: FunctionValue<'ctx>,
    vec_new: FunctionValue<'ctx>,
    vec_len: FunctionValue<'ctx>,
    vec_get: FunctionValue<'ctx>,
    vec_set: FunctionValue<'ctx>,
    vec_from_mask: FunctionValue<'ctx>,
    mask_new: FunctionValue<'ctx>,
    mask_set: FunctionValue<'ctx>,
    sel_len: FunctionValue<'ctx>,
    sel_at: FunctionValue<'ctx>,
    vec_column: FunctionValue<'ctx>,
    rec_get: FunctionValue<'ctx>,
    rec_new: FunctionValue<'ctx>,
    collect_lines: FunctionValue<'ctx>,
    split_lines: FunctionValue<'ctx>,
    rec_set: FunctionValue<'ctx>,
    read_input: FunctionValue<'ctx>,
    read_inputs: FunctionValue<'ctx>,
    parse_str: FunctionValue<'ctx>,
    read_one_input: FunctionValue<'ctx>,
    read_one_line: FunctionValue<'ctx>,
    rec_from_vec: FunctionValue<'ctx>,
    at: FunctionValue<'ctx>,
    opt_is_some: FunctionValue<'ctx>,
    opt_get: FunctionValue<'ctx>,
    opt_some: FunctionValue<'ctx>,
    unwrap: FunctionValue<'ctx>,
    div_by_zero: FunctionValue<'ctx>,
    range: FunctionValue<'ctx>,
    vec_tail: FunctionValue<'ctx>,
    vec_first: FunctionValue<'ctx>,
    vec_any: FunctionValue<'ctx>,
    vec_all: FunctionValue<'ctx>,
    vec_flatten: FunctionValue<'ctx>,
    vec_slice: FunctionValue<'ctx>,
    vec_concat: FunctionValue<'ctx>,
    chars: FunctionValue<'ctx>,
    vec_sort_int: FunctionValue<'ctx>,
    vec_sort_str: FunctionValue<'ctx>,
    vec_reverse: FunctionValue<'ctx>,
    vec_sum: FunctionValue<'ctx>,
    vec_transpose: FunctionValue<'ctx>,
    vec_max: FunctionValue<'ctx>,
    vec_sort_by: FunctionValue<'ctx>,
    vec_max_by: FunctionValue<'ctx>,
    sqrt: FunctionValue<'ctx>,
    pipe_through: FunctionValue<'ctx>,
}

/// What a compiler-introduced binding holds.
///
/// `select` over a Vec of records binds `.` to a position rather than to a value, because
/// struct-of-arrays spreads an element across columns and materialising it would undo the point
/// of the layout. A cursor is only ever consumed by field access, which reads one column.
#[derive(Clone, Copy)]
enum Slot<'ctx> {
    Value(BasicValueEnum<'ctx>),
    Cursor {
        vec: PointerValue<'ctx>,
        index: IntValue<'ctx>,
    },
}

/// One thing a closure body reads that its own text does not bind.
enum Capture {
    Local(LocalId, Type),
    /// A `.` bound to a Vec position rather than a value: both halves travel, so the body can
    /// still read a column off it.
    Cursor(LocalId),
    Param(String, Type),
}

impl Capture {
    fn width(&self) -> u64 {
        match self {
            Capture::Cursor(_) => 2,
            _ => 1,
        }
    }
}

struct Emitter<'ctx, 'p> {
    ctx: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    rt: Runtime<'ctx>,
    funcs: HashMap<String, FunctionValue<'ctx>>,
    locals: HashMap<LocalId, Slot<'ctx>>,
    params: HashMap<String, BasicValueEnum<'ctx>>,
    /// The one parsed stdin value, as the raw slot tl_read_input returns, stored in a global
    /// that main fills before anything else runs. Every other backend binds `input` to a name
    /// read once in its preamble; a call per `Input` node would re-read an already-drained
    /// stdin now that a program can spell `input` more than once.
    input_slot: Option<PointerValue<'ctx>>,
    next_global: usize,
    /// Every enum the program declared, for `ty::variants`: what a `Type::Enum` carries is a
    /// placeholder wherever a recursive enum's payload reaches back to itself.
    enums: &'p Enums,
    /// The printer each recursive enum has of its own, by `Type::ident`. See `declare_printers`.
    printers: HashMap<String, FunctionValue<'ctx>>,
}

impl<'ctx> Emitter<'ctx, '_> {
    fn new<'p>(ctx: &'ctx Context, enums: &'p Enums) -> Emitter<'ctx, 'p> {
        let module = ctx.create_module("toylang");
        let ptr = ctx.ptr_type(AddressSpace::default());
        let i64t = ctx.i64_type();
        let i32t = ctx.i32_type();
        let f64t = ctx.f64_type();

        // Nothing crosses into the runtime by value. A 16-byte struct is passed in registers
        // under the SysV ABI, but that lowering is a C frontend's job rather than LLVM's, and
        // hand-written IR that assumes it is guessing.
        let ptr_ptr = ptr.fn_type(&[ptr.into(), ptr.into()], false);
        let rt = Runtime {
            concat: module.add_function("tl_concat", ptr_ptr, None),
            int_to_str: module.add_function(
                "tl_int_to_str",
                ptr.fn_type(&[i64t.into()], false),
                None,
            ),
            float_to_str: module.add_function(
                "tl_float_to_str",
                ptr.fn_type(&[f64t.into()], false),
                None,
            ),
            str_eq: module.add_function(
                "tl_str_eq",
                i64t.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            str_cmp: module.add_function(
                "tl_str_cmp",
                i64t.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            print: module.add_function(
                "tl_print",
                ctx.void_type().fn_type(&[ptr.into()], false),
                None,
            ),
            quote: module.add_function("tl_quote", ptr.fn_type(&[ptr.into()], false), None),
            join: module.add_function(
                "tl_str_join",
                ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false),
                None,
            ),
            vec_new: module.add_function(
                "tl_vec_new",
                ptr.fn_type(&[i64t.into(), i64t.into()], false),
                None,
            ),
            vec_len: module.add_function("tl_vec_len", i64t.fn_type(&[ptr.into()], false), None),
            vec_get: module.add_function(
                "tl_vec_get",
                i64t.fn_type(&[ptr.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            vec_set: module.add_function(
                "tl_vec_set",
                ctx.void_type()
                    .fn_type(&[ptr.into(), i64t.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            vec_from_mask: module.add_function(
                "tl_vec_from_mask",
                ptr.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            mask_new: module.add_function("tl_mask_new", ptr.fn_type(&[i64t.into()], false), None),
            mask_set: module.add_function(
                "tl_mask_set",
                ctx.void_type()
                    .fn_type(&[ptr.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            sel_len: module.add_function(
                "tl_sel_len",
                i64t.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            sel_at: module.add_function(
                "tl_sel_at",
                ptr.fn_type(&[ptr.into(), ptr.into(), i64t.into(), i32t.into()], false),
                None,
            ),
            vec_column: module.add_function(
                "tl_vec_column",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            rec_get: module.add_function(
                "tl_rec_get",
                i64t.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            rec_new: module.add_function("tl_rec_new", ptr.fn_type(&[i64t.into()], false), None),
            collect_lines: module.add_function("tl_collect_lines", ptr.fn_type(&[], false), None),
            split_lines: module.add_function(
                "tl_split_lines",
                ptr.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            rec_set: module.add_function(
                "tl_rec_set",
                ctx.void_type()
                    .fn_type(&[ptr.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            read_input: module.add_function(
                "tl_read_input",
                i64t.fn_type(&[ptr.into()], false),
                None,
            ),
            parse_str: module.add_function(
                "tl_parse_str",
                i64t.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            read_inputs: module.add_function(
                "tl_read_inputs",
                ptr.fn_type(&[ptr.into()], false),
                None,
            ),
            read_one_input: module.add_function(
                "tl_read_one_input",
                i32t.fn_type(&[ptr.into(), ptr.into()], false),
                None,
            ),
            read_one_line: module.add_function(
                "tl_read_one_line",
                i32t.fn_type(&[ptr.into()], false),
                None,
            ),
            rec_from_vec: module.add_function(
                "tl_rec_from_vec",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            at: module.add_function(
                "tl_at",
                ptr.fn_type(&[ptr.into(), i64t.into(), i64t.into(), i32t.into()], false),
                None,
            ),
            opt_is_some: module.add_function(
                "tl_opt_is_some",
                i64t.fn_type(&[ptr.into()], false),
                None,
            ),
            opt_get: module.add_function("tl_opt_get", i64t.fn_type(&[ptr.into()], false), None),
            opt_some: module.add_function("tl_opt_some", ptr.fn_type(&[i64t.into()], false), None),
            unwrap: module.add_function(
                "tl_unwrap",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            div_by_zero: module.add_function(
                "tl_div_by_zero",
                ctx.void_type().fn_type(&[], false),
                None,
            ),
            range: module.add_function("tl_range", ptr.fn_type(&[i64t.into()], false), None),
            chars: module.add_function("tl_chars", ptr.fn_type(&[ptr.into()], false), None),
            vec_tail: module.add_function("tl_vec_tail", ptr.fn_type(&[ptr.into()], false), None),
            vec_first: module.add_function(
                "tl_vec_first",
                ptr.fn_type(&[ptr.into(), i32t.into()], false),
                None,
            ),
            vec_any: module.add_function("tl_vec_any", i64t.fn_type(&[ptr.into()], false), None),
            vec_all: module.add_function("tl_vec_all", i64t.fn_type(&[ptr.into()], false), None),
            vec_flatten: module.add_function(
                "tl_vec_flatten",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            vec_slice: module.add_function(
                "tl_vec_slice",
                ptr.fn_type(&[ptr.into(), i64t.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            vec_concat: module.add_function(
                "tl_vec_concat",
                ptr.fn_type(&[ptr.into(), ptr.into(), i64t.into()], false),
                None,
            ),
            vec_sort_int: module.add_function(
                "tl_vec_sort_int",
                ptr.fn_type(&[ptr.into()], false),
                None,
            ),
            vec_sort_str: module.add_function(
                "tl_vec_sort_str",
                ptr.fn_type(&[ptr.into()], false),
                None,
            ),
            vec_reverse: module.add_function(
                "tl_vec_reverse",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            vec_sum: module.add_function(
                "tl_vec_sum",
                i64t.fn_type(&[ptr.into(), i32t.into()], false),
                None,
            ),
            vec_transpose: module.add_function(
                "tl_vec_transpose",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
            pipe_through: module.add_function(
                "tl_pipe_through",
                ptr.fn_type(
                    &[ptr.into(), ptr.into(), ptr.into(), i64t.into(), i64t.into()],
                    false,
                ),
                None,
            ),
            vec_max: module.add_function("tl_vec_max", ptr.fn_type(&[ptr.into()], false), None),
            vec_sort_by: module.add_function(
                "tl_vec_sort_by",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32t.into()], false),
                None,
            ),
            vec_max_by: module.add_function(
                "tl_vec_max_by",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32t.into(), i32t.into()], false),
                None,
            ),
            // The LLVM intrinsic, not a runtime call: declaring a function under this exact
            // name is how LLVM recognizes it, and it lowers to the target's libm `sqrt`, the
            // same NaN-on-negative IEEE 754 behavior every other backend's native sqrt gives.
            sqrt: module.add_function("llvm.sqrt.f64", f64t.fn_type(&[f64t.into()], false), None),
        };

        Emitter {
            ctx,
            module,
            builder: ctx.create_builder(),
            rt,
            funcs: HashMap::new(),
            locals: HashMap::new(),
            params: HashMap::new(),
            input_slot: None,
            next_global: 0,
            enums,
            printers: HashMap::new(),
        }
    }

    fn str_struct(&self) -> StructType<'ctx> {
        self.ctx.struct_type(
            &[
                self.ctx.ptr_type(AddressSpace::default()).into(),
                self.ctx.i64_type().into(),
            ],
            false,
        )
    }

    fn llvm_type(&self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, String> {
        Ok(match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            // A pointer to the closure's environment record; see `Emitter::closure`.
            Type::Fn(..) => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Seq(..) => unreachable!(
                "a Seq value cannot reach a backend; no source produces one yet (ADR 0008 emission is a follow-up)"
            ),
            // Materialized eagerly as the Vec of its entries, so it is the same pointer a Vec
            // is. Fusion is what will remove this materialization.
            Type::Stream(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Str => self.ctx.ptr_type(AddressSpace::default()).into(),
            // A sink is a joined string at runtime, so a `-> Sink` function returns one here too.
            Type::Sink => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Int => self.ctx.i64_type().into(),
            // The same i64 an Int already lives in: an Int is computed wide and narrowed after
            // every operation, and an Int64 is that representation with the narrowing left off
            // (kantord/toylang#83).
            Type::Int64 => self.ctx.i64_type().into(),
            Type::Float => self.ctx.f64_type().into(),
            Type::Bool => self.ctx.bool_type().into(),
            // Same width as Int: a Char is a codepoint, and the checker already refuses to mix
            // the two.
            Type::Char => self.ctx.i64_type().into(),
            Type::Vec(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Record(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
            // A pointer to a two-slot box: the tag, then the payload. See `Kind::EnumLit`.
            Type::Enum { .. } => self.ctx.ptr_type(AddressSpace::default()).into(),
        })
    }

    /// How many columns a Vec of this element type has. Struct of arrays: a record contributes
    /// one column per field, anything else one column.
    fn columns(elem: &Type) -> u64 {
        match elem {
            Type::Record(fields) => fields.len() as u64,
            _ => 1,
        }
    }

    /// A literal becomes a private constant for the bytes plus a private constant `tl_str`
    /// pointing at them. The length excludes the trailing NUL, which exists only so a debugger
    /// can print the bytes.
    fn string_const(&mut self, text: &str) -> PointerValue<'ctx> {
        let id = self.next_global;
        self.next_global += 1;

        let bytes = self.ctx.const_string(text.as_bytes(), true);
        let bytes_global = self
            .module
            .add_global(bytes.get_type(), None, &format!("bytes.{id}"));
        bytes_global.set_initializer(&bytes);
        bytes_global.set_constant(true);
        bytes_global.set_linkage(Linkage::Private);

        let str_ty = self.str_struct();
        let init = str_ty.const_named_struct(&[
            bytes_global.as_pointer_value().into(),
            self.ctx
                .i64_type()
                .const_int(text.len() as u64, false)
                .into(),
        ]);
        let global = self.module.add_global(str_ty, None, &format!("str.{id}"));
        global.set_initializer(&init);
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global.as_pointer_value()
    }

    fn declare(&mut self, func: &Func) -> Result<(), String> {
        let ret = self.llvm_type(&func.body.ty)?;
        let args: Vec<BasicMetadataTypeEnum> = match &func.param_ty {
            Some(param_ty) => vec![self.llvm_type(param_ty)?.into()],
            None => Vec::new(),
        };
        let sig = match ret {
            BasicTypeEnum::IntType(t) => t.fn_type(&args, false),
            BasicTypeEnum::PointerType(t) => t.fn_type(&args, false),
            BasicTypeEnum::FloatType(t) => t.fn_type(&args, false),
            other => return Err(unsupported(&format!("a {other:?} return"))),
        };
        // Names are prefixed for the same reason the other backends prefix: `main` is a legal
        // toylang function name and is already spoken for here.
        let value =
            self.module
                .add_function(&format!("v_{}", tir::escape_name(&func.name)), sig, None);
        self.funcs.insert(func.name.clone(), value);
        Ok(())
    }

    fn define(&mut self, func: &Func) -> Result<(), String> {
        let value = self.funcs[&func.name];
        let entry = self.ctx.append_basic_block(value, "entry");
        self.builder.position_at_end(entry);

        self.params.clear();
        self.locals.clear();
        let arg = func.param.as_ref().map(|param| {
            (
                param,
                value.get_nth_param(0).expect("declared with one param"),
            )
        });
        if !tir::has_tail_call(&func.name, &func.body) {
            if let Some((param, arg)) = arg {
                self.params.insert(param.clone(), arg);
            }
            let body = self.expr(&func.body)?;
            self.builder
                .build_return(Some(&body))
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // A self tail call branches back here instead of calling (kantord/toylang#141), so the
        // parameter is a phi: the argument on entry, each tail call's new argument on the back
        // edge. Locals are SSA values defined after the header, so a closure built in one
        // iteration keeps the values of that iteration, which the copy-at-build env already
        // guarantees.
        let header = self.ctx.append_basic_block(value, "tail.loop");
        self.builder
            .build_unconditional_branch(header)
            .map_err(|e| e.to_string())?;
        self.builder.position_at_end(header);
        let phi = match arg {
            Some((param, arg)) => {
                let phi = self
                    .builder
                    .build_phi(arg.get_type(), "tail.arg")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[(&arg, entry)]);
                self.params.insert(param.clone(), phi.as_basic_value());
                Some(phi)
            }
            None => None,
        };
        self.tail_stmts(&func.name, header, phi, &func.body)
    }

    /// `t` in the tail position of a function whose entry is a loop: a base case returns, a
    /// self tail call adds its argument to the parameter phi and branches to `header`. The tail
    /// positions are the ones `tir::has_tail_call` counts, a total match's arm bodies and a
    /// Bind's body, so unlike `expr`'s Match nothing joins afterwards and no result slot is
    /// needed.
    fn tail_stmts(
        &mut self,
        name: &str,
        header: BasicBlock<'ctx>,
        phi: Option<PhiValue<'ctx>>,
        t: &Tir,
    ) -> Result<(), String> {
        match &t.kind {
            Kind::Call { func, arg } if func == name => {
                let next = arg.as_ref().map(|a| self.expr(a)).transpose()?;
                if let (Some(phi), Some(next)) = (phi, next) {
                    let from = self
                        .builder
                        .get_insert_block()
                        .ok_or("no block to branch from")?;
                    phi.add_incoming(&[(&next, from)]);
                }
                self.builder
                    .build_unconditional_branch(header)
                    .map_err(|e| e.to_string())?;
            }
            Kind::Bind { local, value, body } => {
                let value = self.expr(value)?;
                self.locals.insert(*local, Slot::Value(value));
                self.tail_stmts(name, header, phi, body)?;
            }
            Kind::Match {
                subject,
                arms,
                partial: false,
            } => {
                let variants = match &subject.ty {
                    Type::Enum { .. } => ty::variants(self.enums, &subject.ty),
                    _ => Vec::new(),
                };
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let i64t = self.ctx.i64_type();
                let subj = arms
                    .iter()
                    .any(|a| a.variant.is_some())
                    .then(|| self.expr(subject))
                    .transpose()?;
                let tag = match subj {
                    Some(subj) => Some(
                        self.call_rt(self.rt.rec_get, &[subj, i64t.const_zero().into()], "tag")?
                            .into_int_value(),
                    ),
                    None => None,
                };
                for (i, arm) in arms.iter().enumerate() {
                    let last = i + 1 == arms.len();
                    let next = if last {
                        None
                    } else {
                        let is = self.arm_test(tag, &variants, arm)?;
                        let arm_block = self.ctx.append_basic_block(function, "match.arm");
                        let next = self.ctx.append_basic_block(function, "match.next");
                        self.builder
                            .build_conditional_branch(is, arm_block, next)
                            .map_err(|e| e.to_string())?;
                        self.builder.position_at_end(arm_block);
                        Some(next)
                    };
                    self.bind_payload(subj, &variants, arm)?;
                    self.tail_stmts(name, header, phi, &arm.body)?;
                    if let Some(next) = next {
                        self.builder.position_at_end(next);
                    }
                }
            }
            _ => {
                let v = self.expr(t)?;
                self.builder
                    .build_return(Some(&v))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Convert a value to the raw 8-byte slot a Vec column stores, and back.
    ///
    /// Every scalar toylang has fits one slot: an Int is an i64, a Str and a nested Vec are
    /// pointers, a Bool widens. That is what lets one set of runtime functions serve every
    /// element type instead of one per width.
    fn to_slot(&self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<IntValue<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        Ok(match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Stream(_) => unreachable!("the grammar keeps a stream out of every slot"),
            Type::Seq(..) => unreachable!(
                "a Seq value cannot reach a backend; no source produces one yet (ADR 0008 emission is a follow-up)"
            ),
            Type::Sink => unreachable!("the grammar keeps a sink out of every slot"),
            Type::Int | Type::Int64 | Type::Char => value.into_int_value(),
            // A Float is the same 8 bytes as the i64 every other slot holds, reinterpreted
            // rather than converted: the bit pattern round-trips exactly, which a numeric
            // int<->float conversion would not.
            Type::Float => self
                .builder
                .build_bit_cast(value.into_float_value(), i64t, "slot")
                .map_err(|e| e.to_string())?
                .into_int_value(),
            Type::Bool => self
                .builder
                .build_int_z_extend(value.into_int_value(), i64t, "slot")
                .map_err(|e| e.to_string())?,
            Type::Str | Type::Vec(_) | Type::Record(_) | Type::Enum { .. } | Type::Fn(..) => self
                .builder
                .build_ptr_to_int(value.into_pointer_value(), i64t, "slot")
                .map_err(|e| e.to_string())?,
        })
    }

    fn read_slot(&self, slot: IntValue<'ctx>, ty: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        Ok(match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Stream(_) => unreachable!("the grammar keeps a stream out of every slot"),
            Type::Seq(..) => unreachable!(
                "a Seq value cannot reach a backend; no source produces one yet (ADR 0008 emission is a follow-up)"
            ),
            Type::Sink => unreachable!("the grammar keeps a sink out of every slot"),
            Type::Int | Type::Int64 | Type::Char => slot.into(),
            Type::Float => self
                .builder
                .build_bit_cast(slot, self.ctx.f64_type(), "elem")
                .map_err(|e| e.to_string())?,
            Type::Bool => self
                .builder
                .build_int_truncate(slot, self.ctx.bool_type(), "elem")
                .map_err(|e| e.to_string())?
                .into(),
            Type::Str | Type::Vec(_) | Type::Record(_) | Type::Enum { .. } | Type::Fn(..) => self
                .builder
                .build_int_to_ptr(slot, ptr, "elem")
                .map_err(|e| e.to_string())?
                .into(),
        })
    }

    /// Emit `for i in 0..len`, calling `body` to fill the loop body with the index in hand.
    ///
    /// The counter is an alloca rather than a phi. At OptimizationLevel::None it stays a stack
    /// slot, which costs a load and a store per iteration and keeps the emitter from having to
    /// thread incoming blocks through every nested construct.
    fn emit_loop<F>(&mut self, len: IntValue<'ctx>, mut body: F) -> Result<(), String>
    where
        F: FnMut(&mut Self, IntValue<'ctx>) -> Result<(), String>,
    {
        let i64t = self.ctx.i64_type();
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or("no function to emit a loop into")?;

        let counter = self
            .builder
            .build_alloca(i64t, "i")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter, i64t.const_zero())
            .map_err(|e| e.to_string())?;

        let cond = self.ctx.append_basic_block(function, "loop.cond");
        let loop_body = self.ctx.append_basic_block(function, "loop.body");
        let end = self.ctx.append_basic_block(function, "loop.end");

        self.builder
            .build_unconditional_branch(cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond);
        let i = self
            .builder
            .build_load(i64t, counter, "iv")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i, len, "more")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(more, loop_body, end)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_body);
        body(self, i)?;
        let next = self
            .builder
            .build_int_add(i, i64t.const_int(1, false), "next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter, next)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(end);
        Ok(())
    }

    fn vec_lit(&mut self, items: &[Tir], elem: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        let vec = self
            .call_rt(
                self.rt.vec_new,
                &[
                    i64t.const_int(items.len() as u64, false).into(),
                    i64t.const_int(Self::columns(elem), false).into(),
                ],
                "vec",
            )?
            .into_pointer_value();

        for (index, item) in items.iter().enumerate() {
            let value = self.expr(item)?;
            let i = i64t.const_int(index as u64, false);
            // A Vec of records is one column per field, same invariant as everywhere else this
            // layout appears; writing the whole record into column 0 is the bug 363710f fixed
            // for field access, here at the other site that builds a Vec of records: a literal
            // never exercised it, since nothing in the corpus wrote `[{...}, {...}]` directly.
            //
            // `elem` is the Vec's own declared order (the first item's), but an item is only
            // guaranteed to share its field set (kantord/toylang#60), not its spelling: `mk()`
            // may hand back `{b, a}` where the Vec is `{a, b}`. So each column is read out of
            // `value` by looking up its field's name in the item's own type, not by assuming
            // column `col` means the same thing on both sides.
            if let Type::Record(fields) = elem {
                let Type::Record(item_fields) = &item.ty else {
                    return Err(format!("a Vec<{elem}> element that is not a record"));
                };
                for (col, (name, _)) in fields.iter().enumerate() {
                    let src = item_fields
                        .iter()
                        .position(|(n, _)| n == name)
                        .ok_or_else(|| format!("no field `{name}` on {}", item.ty))?;
                    let src = i64t.const_int(src as u64, false);
                    let got = self.call_rt(self.rt.rec_get, &[value, src.into()], "field")?;
                    let dst = i64t.const_int(col as u64, false);
                    self.builder
                        .build_call(
                            self.rt.vec_set,
                            &[vec.into(), dst.into(), i.into(), got.into()],
                            "",
                        )
                        .map_err(|e| e.to_string())?;
                }
            } else {
                let slot = self.to_slot(value, elem)?;
                self.builder
                    .build_call(
                        self.rt.vec_set,
                        &[vec.into(), i64t.const_zero().into(), i.into(), slot.into()],
                        "",
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(vec.into())
    }

    /// `float`, unlike `sqrt`, also takes a Float (identity); only an Int argument needs the
    /// bridge, the same split `emit_rs`/`emit_go` make.
    fn float_of(
        &mut self,
        arg: BasicValueEnum<'ctx>,
        arg_ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match arg_ty {
            Type::Int => self
                .builder
                .build_signed_int_to_float(arg.into_int_value(), self.ctx.f64_type(), "float_of")
                .map_err(|e| e.to_string())
                .map(Into::into),
            _ => Ok(arg),
        }
    }

    /// `fields(r)`'s literal: LLVM's record layout carries no field names at runtime at all
    /// (`vec_lit`'s own Record branch above reads them off the type the same way), so the names
    /// come from `record_ty` rather than any value.
    fn fields_lit(&mut self, record_ty: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        let Type::Record(fields) = record_ty else {
            unreachable!("checked to be a record")
        };
        let items: Vec<Tir> = fields
            .iter()
            .map(|(name, _)| Tir::new(Type::Str, Kind::Str(name.clone())))
            .collect();
        self.vec_lit(&items, &Type::Str)
    }

    /// Every element replaced by the body.
    ///
    /// The result has one column whatever the source had, because the body produces a single
    /// value; a Vec of records going in does not mean a Vec of records coming out. The element
    /// binding is the same cursor `select` uses, so a record source still reads its fields out
    /// of the columns rather than being gathered.
    fn map(
        &mut self,
        source: &Tir,
        param: LocalId,
        body: &Tir,
        result: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let elem_ty = crate::tir::runtime_elem(&source.ty)
            .ok_or_else(|| "map over something that has no dimension".to_string())?
            .clone();
        let out_elem = crate::tir::runtime_elem(result)
            .ok_or_else(|| "map did not produce a dimension".to_string())?
            .clone();

        let i64t = self.ctx.i64_type();
        let src = self.expr(source)?.into_pointer_value();
        let len = self
            .call_rt(self.rt.vec_len, &[src.into()], "len")?
            .into_int_value();
        // One column per field when the body builds a record, because that is what a Vec
        // of products is. Allocating one column here would store record pointers where the
        // layout says field values go, which is the same break that field access had.
        let ncols = Self::columns(&out_elem);
        let out = self
            .call_rt(
                self.rt.vec_new,
                &[len.into(), i64t.const_int(ncols, false).into()],
                "mapped",
            )?
            .into_pointer_value();
        let zero = i64t.const_zero();

        self.emit_loop(len, move |e, i| {
            if matches!(elem_ty, Type::Record(_)) {
                e.locals.insert(param, Slot::Cursor { vec: src, index: i });
            } else {
                let slot = e
                    .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?
                    .into_int_value();
                let elem = e.read_slot(slot, &elem_ty)?;
                e.locals.insert(param, Slot::Value(elem));
            }
            let value = e.expr(body)?;
            if let Type::Record(fields) = &out_elem {
                for c in 0..fields.len() {
                    let c = i64t.const_int(c as u64, false);
                    let got = e.call_rt(e.rt.rec_get, &[value, c.into()], "field")?;
                    e.builder
                        .build_call(
                            e.rt.vec_set,
                            &[out.into(), c.into(), i.into(), got.into()],
                            "",
                        )
                        .map_err(|err| err.to_string())?;
                }
            } else {
                let value = e.to_slot(value, &out_elem)?;
                e.builder
                    .build_call(
                        e.rt.vec_set,
                        &[out.into(), zero.into(), i.into(), value.into()],
                        "",
                    )
                    .map_err(|err| err.to_string())?;
            }
            Ok(())
        })?;
        Ok(out.into())
    }

    /// Build the survivor mask for a `select` without compacting it into a Vec.
    ///
    /// The predicate reads element `i` out of the column: nothing materialises an element, which
    /// is what keeps the loop in the shape that vectorises and is what the struct-of-arrays
    /// layout is for once a Vec of records has several columns. The mask is what a consumer that
    /// only needs to observe the selection (its length, one of its elements) reads through, so
    /// those consumers can avoid the compaction `vec_from_mask` would otherwise do.
    fn select_mask(
        &mut self,
        source: &Tir,
        param: LocalId,
        pred: &Tir,
    ) -> Result<(PointerValue<'ctx>, PointerValue<'ctx>, IntValue<'ctx>, Type), String> {
        let elem_ty = crate::tir::runtime_elem(&source.ty)
            .ok_or_else(|| "select on something that has no dimension".to_string())?
            .clone();

        let src = self.expr(source)?.into_pointer_value();
        let len = self
            .call_rt(self.rt.vec_len, &[src.into()], "len")?
            .into_int_value();
        let mask = self
            .call_rt(self.rt.mask_new, &[len.into()], "mask")?
            .into_pointer_value();
        let zero = self.ctx.i64_type().const_zero();
        let elem_ty_loop = elem_ty.clone();

        self.emit_loop(len, move |e, i| {
            if matches!(elem_ty_loop, Type::Record(_)) {
                e.locals.insert(param, Slot::Cursor { vec: src, index: i });
            } else {
                let slot = e
                    .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?
                    .into_int_value();
                let elem = e.read_slot(slot, &elem_ty_loop)?;
                e.locals.insert(param, Slot::Value(elem));
            }

            let keep = e.expr(pred)?;
            let keep = e.to_slot(keep, &Type::Bool)?;
            e.builder
                .build_call(e.rt.mask_set, &[mask.into(), i.into(), keep.into()], "")
                .map_err(|err| err.to_string())?;
            Ok(())
        })?;

        Ok((src, mask, len, elem_ty))
    }

    /// `sort_by` and `max_by`: project every entry to its key with `body`, then let the runtime
    /// order by the keys. The projection loop binds the element the way `map` does, so a record
    /// source is read out of its columns rather than gathered; the runtime never sees `body`,
    /// only the one-column Vec of keys, and Str keys are the only ones needing a pointer compare.
    fn keyed(
        &mut self,
        source: &Tir,
        param: LocalId,
        body: &Tir,
        max: bool,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let elem_ty = crate::tir::runtime_elem(&source.ty)
            .ok_or_else(|| "sort_by/max_by over something that has no dimension".to_string())?
            .clone();
        let key_ty = body.ty.clone();

        let i64t = self.ctx.i64_type();
        let i32t = self.ctx.i32_type();
        let src = self.expr(source)?.into_pointer_value();
        let len = self
            .call_rt(self.rt.vec_len, &[src.into()], "len")?
            .into_int_value();
        let keys = self
            .call_rt(
                self.rt.vec_new,
                &[len.into(), i64t.const_int(1, false).into()],
                "keys",
            )?
            .into_pointer_value();
        let zero = i64t.const_zero();

        self.emit_loop(len, move |e, i| {
            if matches!(elem_ty, Type::Record(_)) {
                e.locals.insert(param, Slot::Cursor { vec: src, index: i });
            } else {
                let slot = e
                    .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?
                    .into_int_value();
                let elem = e.read_slot(slot, &elem_ty)?;
                e.locals.insert(param, Slot::Value(elem));
            }
            let key = e.expr(body)?;
            let key = e.to_slot(key, &key_ty)?;
            e.builder
                .build_call(
                    e.rt.vec_set,
                    &[keys.into(), zero.into(), i.into(), key.into()],
                    "",
                )
                .map_err(|err| err.to_string())?;
            Ok(())
        })?;

        let is_str = i32t.const_int((body.ty == Type::Str) as u64, false);
        if max {
            let is_record = i32t.const_int(
                matches!(crate::tir::runtime_elem(&source.ty), Some(Type::Record(_))) as u64,
                false,
            );
            self.call_rt(
                self.rt.vec_max_by,
                &[src.into(), keys.into(), is_str.into(), is_record.into()],
                "max_by",
            )
        } else {
            self.call_rt(
                self.rt.vec_sort_by,
                &[src.into(), keys.into(), is_str.into()],
                "sort_by",
            )
        }
    }

    /// `select` builds a mask and then compacts it into a dense Vec.
    ///
    /// The compaction is the one part a consumer that only observes the result (its length, a
    /// single element) can skip; those consumers call `select_mask` and read through the mask
    /// instead of arriving here. This is the fallback that materialises the concrete Vec every
    /// other consumer needs.
    fn select(
        &mut self,
        source: &Tir,
        param: LocalId,
        pred: &Tir,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let (src, mask, _len, _elem_ty) = self.select_mask(source, param, pred)?;
        self.call_rt(self.rt.vec_from_mask, &[src.into(), mask.into()], "kept")
    }

    /// The JSON rendering of a value, built from its type. A native binary has no value to
    /// interrogate at runtime, so this is the only way it could work.
    /// Read a field, descending through however many Vec layers the base type has.
    ///
    /// On a record it is one slot. On a Vec of records it is the column, shared rather than
    /// copied, which is the payoff of the struct-of-arrays layout: `.name` on a Vec<User> costs
    /// one header and no element work. Deeper nesting loops over the outer Vec and recurses.
    fn field(
        &mut self,
        base: &Tir,
        name: &str,
        result: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        // A field off a cursor is one column read: `.age` inside select is `ages[i]`, with no
        // element materialised. This is what the struct-of-arrays layout is for.
        if let Kind::Local(id) = &base.kind
            && let Some(Slot::Cursor { vec, index }) = self.locals.get(id).copied()
        {
            {
                let Type::Record(fields) = &base.ty else {
                    return Err("a cursor whose type is not a record".to_string());
                };
                let column = fields
                    .iter()
                    .position(|(n, _)| n == name)
                    .ok_or_else(|| format!("no field `{name}` on {}", base.ty))?;
                let slot = self
                    .call_rt(
                        self.rt.vec_get,
                        &[
                            vec.into(),
                            self.ctx.i64_type().const_int(column as u64, false).into(),
                            index.into(),
                        ],
                        "column_read",
                    )?
                    .into_int_value();
                return self.read_slot(slot, result);
            }
        }

        let base_ty = base.ty.clone();
        let value = self.expr(base)?;
        self.field_of(value, &base_ty, name, result)
    }

    fn field_of(
        &mut self,
        value: BasicValueEnum<'ctx>,
        base_ty: &Type,
        name: &str,
        result: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        match base_ty {
            Type::Record(fields) => {
                let index = fields
                    .iter()
                    .position(|(n, _)| n == name)
                    .ok_or_else(|| format!("no field `{name}` on {base_ty}"))?;
                let slot = self
                    .call_rt(
                        self.rt.rec_get,
                        &[value, i64t.const_int(index as u64, false).into()],
                        "field",
                    )?
                    .into_int_value();
                self.read_slot(slot, result)
            }

            Type::Vec(elem) | Type::Stream(elem) if matches!(**elem, Type::Record(_)) => {
                let Type::Record(fields) = &**elem else {
                    unreachable!("guarded")
                };
                let index = fields
                    .iter()
                    .position(|(n, _)| n == name)
                    .ok_or_else(|| format!("no field `{name}` on {elem}"))?;
                let field_ty = fields[index].1.clone();
                let column = self.call_rt(
                    self.rt.vec_column,
                    &[value, i64t.const_int(index as u64, false).into()],
                    "column",
                )?;

                // Sharing the column is the whole point of the layout, and it is right for every
                // field whose slot is the value: an Int, a Str pointer, a Vec pointer. It is
                // wrong for a field that is itself a record, because a Vec of records is one
                // column per field and a column of record pointers is one column of pointers.
                // Reading `.b` off that later walks the pointer as if it were data.
                let Type::Record(sub) = field_ty else {
                    return Ok(column);
                };
                let ncols = sub.len();
                let src = column.into_pointer_value();
                let len = self
                    .call_rt(self.rt.vec_len, &[src.into()], "len")?
                    .into_int_value();
                let out = self
                    .call_rt(
                        self.rt.vec_new,
                        &[len.into(), i64t.const_int(ncols as u64, false).into()],
                        "spread",
                    )?
                    .into_pointer_value();
                let zero = i64t.const_zero();
                let record_ty = Type::Record(sub);

                self.emit_loop(len, move |e, i| {
                    let slot = e
                        .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "record")?
                        .into_int_value();
                    let record = e.read_slot(slot, &record_ty)?;
                    for c in 0..ncols {
                        let c = i64t.const_int(c as u64, false);
                        let got = e.call_rt(e.rt.rec_get, &[record, c.into()], "sub")?;
                        e.builder
                            .build_call(
                                e.rt.vec_set,
                                &[out.into(), c.into(), i.into(), got.into()],
                                "",
                            )
                            .map_err(|err| err.to_string())?;
                    }
                    Ok(())
                })?;
                Ok(out.into())
            }

            // A Vec of Vecs: one result per element, so the layer has to be walked.
            Type::Vec(elem) | Type::Stream(elem) => {
                let inner_result = crate::tir::runtime_elem(result)
                    .ok_or_else(|| "field access on a dimension did not keep it".to_string())?
                    .clone();
                let elem_ty = (**elem).clone();
                let src = value.into_pointer_value();
                let len = self
                    .call_rt(self.rt.vec_len, &[src.into()], "len")?
                    .into_int_value();
                let out = self
                    .call_rt(
                        self.rt.vec_new,
                        &[len.into(), i64t.const_int(1, false).into()],
                        "fields",
                    )?
                    .into_pointer_value();
                let zero = i64t.const_zero();
                let name = name.to_string();

                self.emit_loop(len, move |e, i| {
                    let slot = e
                        .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?
                        .into_int_value();
                    let item = e.read_slot(slot, &elem_ty)?;
                    let got = e.field_of(item, &elem_ty, &name, &inner_result)?;
                    let got = e.to_slot(got, &inner_result)?;
                    e.builder
                        .build_call(
                            e.rt.vec_set,
                            &[out.into(), zero.into(), i.into(), got.into()],
                            "",
                        )
                        .map_err(|err| err.to_string())?;
                    Ok(())
                })?;
                Ok(out.into())
            }

            other => Err(format!("no field `{name}` on {other}")),
        }
    }

    /// Show every element of a `Vec<elem>`, joined between `open` and `close` with `sep`. What
    /// the top-level printer uses for a `Vec` (`[`, `,`, `]`) and what `jsonlines` uses for one
    /// (``, `\n`, ``) are the same loop with different punctuation, and the loop is intricate
    /// enough -- gather-vs-scalar branching, a slot conversion, loop emission -- that it is
    /// worth not writing twice.
    fn join_shown(
        &mut self,
        value: BasicValueEnum<'ctx>,
        elem: &Type,
        open: &str,
        sep: &str,
        close: &str,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        let src = value.into_pointer_value();
        let len = self
            .call_rt(self.rt.vec_len, &[src.into()], "len")?
            .into_int_value();
        let zero = i64t.const_zero();
        let parts = self
            .call_rt(
                self.rt.vec_new,
                &[len.into(), i64t.const_int(1, false).into()],
                "parts",
            )?
            .into_pointer_value();

        let elem_ty = elem.clone();
        let gather = matches!(elem_ty, Type::Record(_));
        self.emit_loop(len, move |e, i| {
            let item = if gather {
                e.call_rt(e.rt.rec_from_vec, &[src.into(), i.into()], "elem")?
            } else {
                let slot = e
                    .call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?
                    .into_int_value();
                e.read_slot(slot, &elem_ty)?
            };
            let shown = e.show(item, &elem_ty)?;
            let shown = e.to_slot(shown, &Type::Str)?;
            e.builder
                .build_call(
                    e.rt.vec_set,
                    &[parts.into(), zero.into(), i.into(), shown.into()],
                    "",
                )
                .map_err(|err| err.to_string())?;
            Ok(())
        })?;

        let open = self.string_const(open);
        let sep = self.string_const(sep);
        let close = self.string_const(close);
        self.call_rt(
            self.rt.join,
            &[parts.into(), open.into(), sep.into(), close.into()],
            "joined",
        )
    }

    fn show(
        &mut self,
        value: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Fn(..) => unreachable!("a closure's type never reaches a backend"),
            // The checker refuses a program whose result contains a stream, since there is
            // nothing to print: a stream has no value, only a promise that collect can redeem.
            Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
            Type::Seq(..) => unreachable!(
                "a Seq value cannot reach the printer; no source produces one yet (ADR 0008 emission is a follow-up)"
            ),
            Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
            Type::Sink => unreachable!("a sink only ever prints raw, never through the printer"),
            Type::Str => self.call_rt(self.rt.quote, &[value], "quoted")?,
            // tl_int_to_str already takes the full i64, so both widths print through it.
            Type::Int | Type::Int64 => self.call_rt(self.rt.int_to_str, &[value], "int_str")?,
            Type::Float => self.call_rt(self.rt.float_to_str, &[value], "float_str")?,
            Type::Bool => {
                let t = self.string_const("true");
                let f = self.string_const("false");
                self.builder
                    .build_select(value.into_int_value(), t, f, "bool_str")
                    .map_err(|e| e.to_string())?
            }
            Type::Vec(elem) => self.join_shown(value, elem, "[", ",", "]")?,
            // Absence needs a branch rather than a select, because rendering what is present
            // may itself emit a loop, and a select would evaluate both sides.
            Type::Enum { .. } if ty.as_opt().is_some() => {
                let inner = ty.as_opt().expect("guarded");
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                let slot = self
                    .builder
                    .build_alloca(ptr_ty, "shown")
                    .map_err(|e| e.to_string())?;

                let present = self.call_rt(self.rt.opt_is_some, &[value], "some")?;
                let cond = self
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        present.into_int_value(),
                        self.ctx.i64_type().const_zero(),
                        "is_some",
                    )
                    .map_err(|e| e.to_string())?;

                let some = self.ctx.append_basic_block(function, "some");
                let none = self.ctx.append_basic_block(function, "none");
                let done = self.ctx.append_basic_block(function, "shown.done");
                self.builder
                    .build_conditional_branch(cond, some, none)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(some);
                let raw = self
                    .call_rt(self.rt.opt_get, &[value], "unwrapped")?
                    .into_int_value();
                let item = self.read_slot(raw, inner)?;
                let shown = self.show(item, inner)?;
                self.builder
                    .build_store(slot, shown)
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(none);
                let null = self.string_const("null");
                self.builder
                    .build_store(slot, null)
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(done);
                self.builder
                    .build_load(ptr_ty, slot, "shown")
                    .map_err(|e| e.to_string())?
            }

            // A recursive enum prints through a function of its own (`declare_printers`),
            // because expanding one here has no bottom: its payload leads back to the same type.
            Type::Enum { .. } if self.printers.contains_key(&ty.ident()) => {
                let f = self.printers[&ty.ident()];
                self.call_rt(f, &[value], "shown")?
            }
            Type::Enum { .. } => self.show_enum(value, ty)?,
            // Keys are known and ordered at compile time, exactly as on the other two
            // backends, so nothing enumerates fields at runtime.
            Type::Record(fields) => {
                let i64t = self.ctx.i64_type();
                let parts = self
                    .call_rt(
                        self.rt.vec_new,
                        &[
                            i64t.const_int(fields.len() as u64, false).into(),
                            i64t.const_int(1, false).into(),
                        ],
                        "parts",
                    )?
                    .into_pointer_value();

                for (index, (name, fty)) in fields.iter().enumerate() {
                    let got = self.field_of(value, ty, name, fty)?;
                    let shown = self.show(got, fty)?;
                    let key = self.string_const(&format!("\"{name}\":"));
                    let part = self.call_rt(self.rt.concat, &[key.into(), shown], "pair")?;
                    let part = self.to_slot(part, &Type::Str)?;
                    self.builder
                        .build_call(
                            self.rt.vec_set,
                            &[
                                parts.into(),
                                i64t.const_zero().into(),
                                i64t.const_int(index as u64, false).into(),
                                part.into(),
                            ],
                            "",
                        )
                        .map_err(|e| e.to_string())?;
                }

                let open = self.string_const("{");
                let sep = self.string_const(",");
                let close = self.string_const("}");
                self.call_rt(
                    self.rt.join,
                    &[parts.into(), open.into(), sep.into(), close.into()],
                    "joined",
                )?
            }
        })
    }

    /// The printer for one enum, inline. The tag picks between the two JSON shapes (ADR 0009):
    /// a unit variant renders as its quoted name, a payload variant as the single-key wrapper.
    /// A chain of branches rather than a select, because rendering a payload allocates and
    /// loops.
    fn show_enum(
        &mut self,
        value: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let variants = ty::variants(self.enums, ty);
        if variants.is_empty() {
            return Err(unsupported("printing an enum with no variants"));
        }
        let function = self.enclosing_function()?;
        let i64t = self.ctx.i64_type();
        let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
        let slot = self
            .builder
            .build_alloca(ptr_ty, "shown")
            .map_err(|e| e.to_string())?;
        let tag = self
            .call_rt(self.rt.rec_get, &[value, i64t.const_zero().into()], "tag")?
            .into_int_value();
        let done = self.ctx.append_basic_block(function, "enum.done");

        self.dispatch_on_tag(
            "enum",
            tag,
            &variants,
            done,
            slot,
            |e, vname, payload, slot| e.enum_arm(value, vname, payload, slot),
        )?;

        self.builder.position_at_end(done);
        self.builder
            .build_load(ptr_ty, slot, "shown")
            .map_err(|e| e.to_string())
    }

    /// Render one variant's string into `slot`: the quoted name for a unit variant, the
    /// single-key wrapper around the shown payload otherwise.
    fn enum_arm(
        &mut self,
        value: BasicValueEnum<'ctx>,
        vname: &str,
        payload: &Option<Type>,
        slot: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let shown = match payload {
            None => self.string_const(&format!("\"{vname}\"")).into(),
            Some(pty) => {
                let i64t = self.ctx.i64_type();
                let raw = self
                    .call_rt(
                        self.rt.rec_get,
                        &[value, i64t.const_int(1, false).into()],
                        "payload",
                    )?
                    .into_int_value();
                let p = self.read_slot(raw, pty)?;
                let shown_p = self.show(p, pty)?;
                let key = self.string_const(&format!("{{\"{vname}\":"));
                let open = self.call_rt(self.rt.concat, &[key.into(), shown_p], "wrapped")?;
                let close = self.string_const("}");
                self.call_rt(self.rt.concat, &[open, close.into()], "wrapped")?
            }
        };
        self.builder
            .build_store(slot, shown)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// A call's argument list: one value, or none for a nullary function.
    /// A closure value is a record built with the runtime's own `tl_rec_*`: slot 0 the address of
    /// this site's function, the rest whatever the body reads from outside, so the environment and
    /// the code pointer travel as the one pointer every other value already is. The function
    /// takes that record and the argument. Captures are copied in when the closure is built, which
    /// is what keeps one carried through a tail call on the values it was built with: a callee's
    /// locals are fresh SSA values per call, so nothing here is rebound underneath it.
    fn closure(
        &mut self,
        ty: &Type,
        param: LocalId,
        body: &Tir,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let Type::Fn(input, output) = ty else {
            unreachable!("a Closure node is typed Type::Fn")
        };
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        let i64t = self.ctx.i64_type();
        let captures = self.captures(param, body)?;

        let sig = self
            .llvm_type(output)?
            .fn_type(&[ptr.into(), self.llvm_type(input)?.into()], false);
        let id = self.next_global;
        self.next_global += 1;
        let function = self
            .module
            .add_function(&format!("closure.{id}"), sig, None);
        function.set_linkage(Linkage::Private);

        // The env: one slot for the code pointer, then each capture's own slot or slots.
        let width = 1 + captures.iter().map(Capture::width).sum::<u64>();
        let env = self
            .call_rt(
                self.rt.rec_new,
                &[i64t.const_int(width, false).into()],
                "env",
            )?
            .into_pointer_value();
        let code = self
            .builder
            .build_ptr_to_int(function.as_global_value().as_pointer_value(), i64t, "code")
            .map_err(|e| e.to_string())?;
        self.rec_slot(env, 0, code)?;
        let mut at = 1;
        for capture in &captures {
            for slot in self.capture_slots(capture)? {
                self.rec_slot(env, at, slot)?;
                at += 1;
            }
        }

        // The body is emitted mid-way through the enclosing function, so its builder position and
        // scope are set aside and put back.
        let resume = self.builder.get_insert_block();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_params = std::mem::take(&mut self.params);
        self.builder
            .position_at_end(self.ctx.append_basic_block(function, "entry"));
        let env_in = function.get_nth_param(0).expect("declared with an env");
        let mut at = 1;
        for capture in &captures {
            match capture {
                Capture::Local(id, ty) => {
                    let v = self.env_slot(env_in, at, ty)?;
                    self.locals.insert(*id, Slot::Value(v));
                }
                Capture::Cursor(id) => {
                    let vec = self.env_slot(env_in, at, &Type::Str)?.into_pointer_value();
                    let index = self.env_slot(env_in, at + 1, &Type::Int)?.into_int_value();
                    self.locals.insert(*id, Slot::Cursor { vec, index });
                }
                Capture::Param(name, ty) => {
                    let v = self.env_slot(env_in, at, ty)?;
                    self.params.insert(name.clone(), v);
                }
            }
            at += capture.width();
        }
        let arg = function
            .get_nth_param(1)
            .expect("declared with an argument");
        self.locals.insert(param, Slot::Value(arg));
        let result = self.expr(body)?;
        self.builder
            .build_return(Some(&result))
            .map_err(|e| e.to_string())?;

        self.locals = outer_locals;
        self.params = outer_params;
        self.builder
            .position_at_end(resume.expect("a closure is built inside a function"));
        Ok(env.into())
    }

    fn apply_closure(
        &mut self,
        closure: &Tir,
        arg: &Tir,
        result: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let Type::Fn(input, _) = &closure.ty else {
            unreachable!("an ApplyClosure's callee is typed Type::Fn")
        };
        let env = self.expr(closure)?;
        let arg = self.expr(arg)?;
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        let sig = self
            .llvm_type(result)?
            .fn_type(&[ptr.into(), self.llvm_type(input)?.into()], false);
        let code = self
            .call_rt(
                self.rt.rec_get,
                &[env, self.ctx.i64_type().const_zero().into()],
                "code",
            )?
            .into_int_value();
        let code = self
            .builder
            .build_int_to_ptr(code, ptr, "code")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_indirect_call(sig, code, &[env.into(), arg.into()], "apply")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| "a closure returned nothing".to_string())
    }

    /// What `body` reads from outside its own text, resolved against the current scope: a local
    /// bound to a value, a local bound to a cursor (its Vec and its index both travel), or a
    /// function parameter. `input` is a module global, so it needs no capture.
    fn captures(&self, param: LocalId, body: &Tir) -> Result<Vec<Capture>, String> {
        let mut bound = vec![param];
        let mut locals = BTreeMap::new();
        let mut params = BTreeMap::new();
        tir::each_node(body, &mut |t| match &t.kind {
            Kind::Local(id) => {
                locals.insert(*id, t.ty.clone());
            }
            Kind::Var(name) => {
                params.insert(name.clone(), t.ty.clone());
            }
            Kind::Bind { local, .. } => bound.push(*local),
            Kind::Map { param, .. }
            | Kind::OptMap { param, .. }
            | Kind::Select { param, .. }
            | Kind::SortBy { param, .. }
            | Kind::MaxBy { param, .. }
            | Kind::Closure { param, .. } => bound.push(*param),
            Kind::Match { arms, .. } => bound.extend(arms.iter().filter_map(|a| a.payload)),
            _ => {}
        });
        for id in bound {
            locals.remove(&id);
        }
        let mut captures = Vec::new();
        for (id, ty) in locals {
            captures.push(match self.locals.get(&id) {
                Some(Slot::Value(_)) => Capture::Local(id, ty),
                Some(Slot::Cursor { .. }) => Capture::Cursor(id),
                None => return Err(format!("local {id} is not bound in the native backend")),
            });
        }
        for (name, ty) in params {
            if !self.params.contains_key(&name) {
                return Err(format!("`{name}` is not in scope in the native backend"));
            }
            captures.push(Capture::Param(name, ty));
        }
        Ok(captures)
    }

    /// The raw slots one capture contributes to a closure's env, in the order `closure` reads
    /// them back.
    fn capture_slots(&self, capture: &Capture) -> Result<Vec<IntValue<'ctx>>, String> {
        Ok(match capture {
            Capture::Local(id, ty) => {
                let Some(Slot::Value(v)) = self.locals.get(id) else {
                    unreachable!("`captures` resolved this as a value")
                };
                vec![self.to_slot(*v, ty)?]
            }
            Capture::Cursor(id) => {
                let Some(Slot::Cursor { vec, index }) = self.locals.get(id) else {
                    unreachable!("`captures` resolved this as a cursor")
                };
                let vec = self
                    .builder
                    .build_ptr_to_int(*vec, self.ctx.i64_type(), "slot")
                    .map_err(|e| e.to_string())?;
                vec![vec, *index]
            }
            Capture::Param(name, ty) => vec![self.to_slot(self.params[name], ty)?],
        })
    }

    fn rec_slot(
        &self,
        rec: PointerValue<'ctx>,
        at: u64,
        slot: IntValue<'ctx>,
    ) -> Result<(), String> {
        let at = self.ctx.i64_type().const_int(at, false);
        self.builder
            .build_call(self.rt.rec_set, &[rec.into(), at.into(), slot.into()], "")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn env_slot(
        &self,
        env: BasicValueEnum<'ctx>,
        at: u64,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let at = self.ctx.i64_type().const_int(at, false);
        let raw = self
            .call_rt(self.rt.rec_get, &[env, at.into()], "captured")?
            .into_int_value();
        self.read_slot(raw, ty)
    }

    fn call_args(
        &mut self,
        arg: &Option<Box<Tir>>,
    ) -> Result<Vec<BasicMetadataValueEnum<'ctx>>, String> {
        match arg {
            Some(arg) => Ok(vec![self.expr(arg)?.into()]),
            None => Ok(Vec::new()),
        }
    }

    /// Opt's constructors keep the encoding Opt always had here: `none` is the null
    /// pointer, `some(x)` the one-slot box `tl_opt_some` builds -- the same values `tl_at`
    /// produces, and already tagged (a box holding null is not null).
    fn opt_lit(&mut self, payload: Option<&Tir>) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match payload {
            None => {
                let ptr = self.ctx.ptr_type(AddressSpace::default());
                ptr.const_null().into()
            }
            Some(p) => {
                let built = self.expr(p)?;
                let slot = self.to_slot(built, &p.ty)?;
                self.call_rt(self.rt.opt_some, &[slot.into()], "some")?
            }
        })
    }

    /// A two-slot box built with the record runtime: slot 0 the tag (the variant's declaration
    /// index), slot 1 the payload, written only when one exists. Boxed rather than immediate
    /// because an enum value has to fit the same 8-byte slot as every other value while
    /// carrying two facts.
    fn enum_lit(
        &mut self,
        ty: &Type,
        variant: &str,
        payload: Option<&Tir>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let tag = ty::variants(self.enums, ty)
            .iter()
            .position(|(n, _)| n == variant)
            .ok_or_else(|| format!("`{variant}` is not a variant of {ty}"))?;
        let i64t = self.ctx.i64_type();
        let rec = self
            .call_rt(self.rt.rec_new, &[i64t.const_int(2, false).into()], "enum")?
            .into_pointer_value();
        self.builder
            .build_call(
                self.rt.rec_set,
                &[
                    rec.into(),
                    i64t.const_zero().into(),
                    i64t.const_int(tag as u64, false).into(),
                ],
                "",
            )
            .map_err(|e| e.to_string())?;
        if let Some(p) = payload {
            let built = self.expr(p)?;
            let slot = self.to_slot(built, &p.ty)?;
            self.builder
                .build_call(
                    self.rt.rec_set,
                    &[rec.into(), i64t.const_int(1, false).into(), slot.into()],
                    "",
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(rec.into())
    }

    /// The record literal is taken apart here rather than built: the runtime wants the three
    /// fields as arguments, not a box. The runtime builds the `PipeLine` boxes itself, so it is
    /// told the two variants' tags (their declaration indices, as `enum_lit` assigns them).
    fn pipe_through(&mut self, arg: &Tir, result: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        let Kind::RecordLit { fields } = &arg.kind else {
            unreachable!("pipe_through's argument is checked to be the record literal")
        };
        let mut field = |name: &str| {
            let (_, value) = fields
                .iter()
                .find(|(n, _)| n == name)
                .expect("pipe_through's record is checked to carry all three fields");
            self.expr(value)
        };
        let (cmd, args, lines) = (field("cmd")?, field("args")?, field("lines")?);
        let pipeline = crate::tir::runtime_elem(result).expect("pipe_through returns a stream");
        let variants = ty::variants(self.enums, pipeline);
        let tag = |name: &str| {
            let index = variants
                .iter()
                .position(|(n, _)| n == name)
                .expect("the prelude's PipeLine has Stdout and Stderr variants");
            self.ctx.i64_type().const_int(index as u64, false)
        };
        let (stdout_tag, stderr_tag) = (tag("Stdout"), tag("Stderr"));
        self.call_rt(
            self.rt.pipe_through,
            &[cmd, args, lines, stdout_tag.into(), stderr_tag.into()],
            "pipe",
        )
    }

    fn expr(&mut self, t: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match &t.kind {
            Kind::Str(text) => self.string_const(text).into(),
            Kind::Int(n) => self.ctx.i64_type().const_int(*n as u64, true).into(),
            Kind::Float(n) => self.ctx.f64_type().const_float(*n).into(),
            // The same i1 a comparison yields (`compare` below), so a literal and a computed
            // Bool are interchangeable everywhere one is consumed.
            Kind::Bool(b) => self.ctx.bool_type().const_int(u64::from(*b), false).into(),

            Kind::Var(name) => *self
                .params
                .get(name)
                .ok_or_else(|| format!("`{name}` is not in scope in the native backend"))?,

            Kind::Local(id) => match self.locals.get(id) {
                Some(Slot::Value(v)) => *v,
                // The struct-of-arrays boundary: a Vec of records is stored as columns, so a
                // record element read as a whole value is gathered back out by the runtime.
                Some(&Slot::Cursor { vec, index }) => {
                    self.call_rt(self.rt.rec_from_vec, &[vec.into(), index.into()], "elem")?
                }
                None => return Err(format!("local {id} is not bound in the native backend")),
            },

            Kind::Bind { local, value, body } => {
                let value = self.expr(value)?;
                self.locals.insert(*local, Slot::Value(value));
                self.expr(body)?
            }

            Kind::Call { func, arg } => {
                let args = self.call_args(arg)?;
                let callee = *self
                    .funcs
                    .get(func)
                    .ok_or_else(|| format!("`{func}` was never declared"))?;
                self.builder
                    .build_call(callee, &args, "call")
                    .map_err(|e| e.to_string())?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| "a toylang function returned nothing".to_string())?
            }

            Kind::Concat(l, r) => self.concat(&t.ty, l, r)?,

            Kind::Compare { op, lhs, rhs } => self.compare(*op, lhs, rhs)?,

            Kind::Logic { op, lhs, rhs } => self.logic(*op, lhs, rhs)?,

            // A Bool is an i1 here, so a bitwise complement is the whole of it.
            Kind::Not(base) => {
                let v = self.expr(base)?.into_int_value();
                self.builder
                    .build_not(v, "not")
                    .map_err(|e| e.to_string())?
                    .into()
            }

            // Fields are in declaration order, so field `i` here is field `i` of the type,
            // which is what every reader of the record relies on.
            Kind::RecordLit { fields } => {
                let i64t = self.ctx.i64_type();
                let rec = self
                    .call_rt(
                        self.rt.rec_new,
                        &[i64t.const_int(fields.len() as u64, false).into()],
                        "rec",
                    )?
                    .into_pointer_value();
                for (i, (_, value)) in fields.iter().enumerate() {
                    let built = self.expr(value)?;
                    let slot = self.to_slot(built, &value.ty)?;
                    self.builder
                        .build_call(
                            self.rt.rec_set,
                            &[
                                rec.into(),
                                i64t.const_int(i as u64, false).into(),
                                slot.into(),
                            ],
                            "",
                        )
                        .map_err(|e| e.to_string())?;
                }
                rec.into()
            }

            Kind::EnumLit { variant, payload } => {
                if t.ty.as_opt().is_some() {
                    return self.opt_lit(payload.as_deref());
                }
                self.enum_lit(&t.ty, variant, payload.as_deref())?
            }

            Kind::VecLit(items) => {
                let elem =
                    t.ty.elem()
                        .ok_or_else(|| "a Vec literal that is not a Vec".to_string())?
                        .clone();
                self.vec_lit(items, &elem)?
            }

            Kind::Select {
                source,
                param,
                pred,
            } => self.select(source, *param, pred)?,

            Kind::SortBy {
                source,
                param,
                body,
            } => self.keyed(source, *param, body, false)?,

            Kind::MaxBy {
                source,
                param,
                body,
            } => self.keyed(source, *param, body, true)?,

            Kind::Map {
                source,
                param,
                body,
            } => self.map(source, *param, body, &t.ty)?,

            // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
            // works on the Vec of its entries.
            Kind::Lines => self.call_rt(self.rt.collect_lines, &[], "lines")?,

            // Same raw lines as `lines`, each split on the delimiter into one row. The delimiter
            // rides as a string constant, so the split is literal on every backend.
            Kind::Dsv { delim } => {
                let lines = self.call_rt(self.rt.collect_lines, &[], "lines")?;
                let sep = self.string_const(delim);
                self.call_rt(self.rt.split_lines, &[lines, sep.into()], "rows")?
            }

            Kind::Input => {
                let global = self
                    .input_slot
                    .ok_or_else(|| "an `input` node the module never read stdin for".to_string())?;
                let slot = self
                    .builder
                    .build_load(self.ctx.i64_type(), global, "input_slot")
                    .map_err(|e| e.to_string())?
                    .into_int_value();
                self.read_slot(slot, &t.ty)?
            }

            // Already a proper Vec pointer, unlike Input's raw slot: tl_read_inputs assembles
            // it itself, so there is nothing here for read_slot to unpack.
            Kind::Inputs => {
                let elem =
                    crate::tir::runtime_elem(&t.ty).expect("checked to be Vec<T> or Stream<T>");
                let descriptor = self.string_const(&descriptor(self.enums, elem));
                self.call_rt(self.rt.read_inputs, &[descriptor.into()], "inputs")?
            }

            Kind::Field { base, name } => self.field(base, name, &t.ty)?,

            Kind::Arith { op, lhs, rhs } if t.ty == Type::Float => {
                let l = self.expr(lhs)?.into_float_value();
                let r = self.expr(rhs)?.into_float_value();
                self.arith_float(*op, l, r)?
            }
            Kind::Arith { op, lhs, rhs } => {
                let l = self.expr(lhs)?.into_int_value();
                let r = self.expr(rhs)?.into_int_value();
                self.arith_at(&t.ty, *op, l, r)?
            }

            Kind::Builtin {
                which: Builtin::PipeThrough,
                arg,
            } => self.pipe_through(arg, &t.ty)?,

            Kind::Builtin { which, arg } => {
                let elem_ty = crate::tir::runtime_elem(&arg.ty).cloned();
                // Read before `arg` below shadows the node with its computed value: `Fields`
                // wants the checked type, not the record value.
                let record_ty = arg.ty.clone();
                let arg_node = arg;
                let arg = self.expr(arg_node)?;
                match which {
                    Builtin::IntToStr => self.call_rt(self.rt.int_to_str, &[arg], "int_str")?,
                    // The descriptor tells the runtime parser what the result type is, the
                    // same string `tl_read_input` carries for stdin; here it parses a value
                    // already in hand instead of reading a stream.
                    Builtin::Parse => {
                        let descriptor = self.string_const(&descriptor(self.enums, &t.ty));
                        let slot =
                            self.call_rt(self.rt.parse_str, &[arg, descriptor.into()], "parse")?;
                        self.read_slot(slot.into_int_value(), &t.ty)?
                    }
                    // An Int already lives in an i64 here (see `llvm_type`), so the bridge
                    // is the identity: the value is its own widening.
                    Builtin::IntToI64 => arg,
                    Builtin::Range => self.call_rt(self.rt.range, &[arg], "range")?,
                    Builtin::Chars => self.call_rt(self.rt.chars, &[arg], "chars")?,
                    Builtin::JsonLines => {
                        let elem = elem_ty.expect("checked to be a Vec");
                        self.join_shown(arg, &elem, "", "\n", "")?
                    }
                    // The source already materialized, so the exit has nothing left to do.
                    Builtin::Collect => arg,
                    // A length directly on a Select stays lazy: it counts the survivors in
                    // the mask instead of compacting the selection into a dense Vec first.
                    Builtin::Length => {
                        if let Kind::Select {
                            source,
                            param,
                            pred,
                        } = &arg_node.kind
                        {
                            let (src, mask, _len, _elem_ty) =
                                self.select_mask(source, *param, pred)?;
                            self.call_rt(self.rt.sel_len, &[src.into(), mask.into()], "sel_len")?
                        } else {
                            self.call_rt(self.rt.vec_len, &[arg], "length")?
                        }
                    }
                    Builtin::Tail => self.call_rt(self.rt.vec_tail, &[arg], "tail")?,
                    // A record entry is spread across columns, so `is_record` is what tells the
                    // runtime to gather it back, the same flag an Index collapse carries.
                    Builtin::First => {
                        let elem = elem_ty.expect("checked to be a Vec");
                        let is_record = self
                            .ctx
                            .i32_type()
                            .const_int(matches!(elem, Type::Record(_)) as u64, false);
                        self.call_rt(self.rt.vec_first, &[arg, is_record.into()], "first")?
                    }
                    // The runtime answers in a slot (an i64), so the result is truncated to the
                    // i1 a Bool is here, the same width an Int-to-Bool narrowing does.
                    Builtin::Any | Builtin::All => {
                        let rt = if *which == Builtin::Any {
                            self.rt.vec_any
                        } else {
                            self.rt.vec_all
                        };
                        let call = self.call_rt(rt, &[arg], "cut")?;
                        self.builder
                            .build_int_truncate(call.into_int_value(), self.ctx.bool_type(), "cut")
                            .map_err(|e| e.to_string())?
                            .into()
                    }
                    Builtin::Flatten => {
                        let elem = t.ty.elem().expect("checked to be Vec<Vec<T>> -> Vec<T>");
                        let ncols = self.ctx.i64_type().const_int(Self::columns(elem), false);
                        self.call_rt(self.rt.vec_flatten, &[arg, ncols.into()], "flatten")?
                    }
                    // The checker's `orderable` already narrowed the element type to Int,
                    // Int64, Str, or Char; only Str's raw slot is a pointer needing its own
                    // comparator, so those three collapse into the same int64 sort.
                    Builtin::Sort => {
                        let elem = elem_ty.expect("checked to be a Vec");
                        let rt = Self::sort_runtime(&self.rt, &elem);
                        self.call_rt(rt, &[arg], "sort")?
                    }
                    Builtin::Reverse => {
                        let elem = elem_ty.expect("checked to be a Vec");
                        let ncols = self.ctx.i64_type().const_int(Self::columns(&elem), false);
                        self.call_rt(self.rt.vec_reverse, &[arg, ncols.into()], "reverse")?
                    }
                    // Both widths live in the same i64 slot, so the C function's `narrow` flag
                    // is what tells Int (32-bit wrap per addition) from Int64. An `i32` like
                    // `tl_at`'s `is_record` flag, since C's `int` is 32 bits.
                    Builtin::Sum => {
                        let narrow = self
                            .ctx
                            .i32_type()
                            .const_int((elem_ty.as_ref() == Some(&Type::Int)) as u64, false);
                        self.call_rt(self.rt.vec_sum, &[arg, narrow.into()], "sum")?
                    }
                    // The result is also a Vec of Vecs, so its element (an inner Vec) carries
                    // the innermost type's column count -- the same reason `tl_vec_flatten`
                    // takes `ncols` rather than reading it off either Vec.
                    Builtin::Transpose => {
                        let inner =
                            t.ty.elem()
                                .and_then(Type::elem)
                                .expect("checked to be Vec<Vec<T>>");
                        let ncols = self.ctx.i64_type().const_int(Self::columns(inner), false);
                        self.call_rt(self.rt.vec_transpose, &[arg, ncols.into()], "transpose")?
                    }
                    // NULL on an empty Vec is exactly the absent Opt a partial Index yields.
                    Builtin::Max => self.call_rt(self.rt.vec_max, &[arg], "max")?,
                    // `arg` above ran only for whatever else it does; its value is unused here.
                    Builtin::Fields => self.fields_lit(&record_ty)?,
                    Builtin::Sqrt => self.call_rt(self.rt.sqrt, &[arg], "sqrt")?,
                    Builtin::FloatOf => self.float_of(arg, &arg_node.ty)?,
                    _ => unreachable!("not yet implemented for this backend"),
                }
            }

            Kind::Unwrap { base } => {
                let depth = crate::tir::vec_depth(&base.ty);
                let inner = t.ty.clone();
                let base = self.expr(base)?;
                let raw = self
                    .call_rt(
                        self.rt.unwrap,
                        &[
                            base,
                            self.ctx.i64_type().const_int(depth as u64, false).into(),
                        ],
                        "unwrapped",
                    )?
                    .into_pointer_value();
                // A scalar rides in the slot itself, so its bits come back through `read_slot`
                // (a Float's as its i64 bit pattern, the way `to_slot` stored it); everything
                // else is a pointer to the boxed value, which is already the value.
                if depth == 0
                    && matches!(
                        inner,
                        Type::Int | Type::Int64 | Type::Bool | Type::Char | Type::Float
                    )
                {
                    let slot = self
                        .builder
                        .build_ptr_to_int(raw, self.ctx.i64_type(), "slot")
                        .map_err(|e| e.to_string())?;
                    self.read_slot(slot, &inner)?
                } else {
                    raw.into()
                }
            }

            // Same presence branch `show`'s Opt arm takes, generalised to rebuild the payload
            // (via `opt_some`) instead of rendering it. The reorder pass (kantord/toylang#66)
            // is the only caller: it is how a value crossing into a differently-ordered
            // `Opt<Record>` gets its payload rebuilt, since Opt's null-or-boxed encoding here
            // is not the tagged box `Kind::Match` reads a tag off of.
            Kind::OptMap {
                source,
                param,
                body,
            } => {
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let source_inner = source
                    .ty
                    .as_opt()
                    .expect("an OptMap's source is Opt")
                    .clone();
                let src = self.expr(source)?;
                let present = self.call_rt(self.rt.opt_is_some, &[src], "some")?;
                let cond = self
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        present.into_int_value(),
                        self.ctx.i64_type().const_zero(),
                        "is_some",
                    )
                    .map_err(|e| e.to_string())?;
                let some_block = self.ctx.append_basic_block(function, "optmap.some");
                let none_block = self.ctx.append_basic_block(function, "optmap.none");
                let done = self.ctx.append_basic_block(function, "optmap.done");
                let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                let slot = self
                    .builder
                    .build_alloca(ptr_ty, "optmapped")
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_conditional_branch(cond, some_block, none_block)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(some_block);
                let raw = self
                    .call_rt(self.rt.opt_get, &[src], "unwrapped")?
                    .into_int_value();
                let item = self.read_slot(raw, &source_inner)?;
                self.locals.insert(*param, Slot::Value(item));
                let mapped = self.expr(body)?;
                let mapped_slot = self.to_slot(mapped, &body.ty)?;
                let wrapped = self.call_rt(self.rt.opt_some, &[mapped_slot.into()], "some")?;
                self.builder
                    .build_store(slot, wrapped)
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(none_block);
                self.builder
                    .build_store(slot, ptr_ty.const_null())
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(done);
                self.builder
                    .build_load(ptr_ty, slot, "optmapped")
                    .map_err(|e| e.to_string())?
            }

            Kind::Index {
                base,
                index,
                depth,
                elem_is_record,
            } => {
                let i64t = self.ctx.i64_type();
                // A depth-0 index directly on a Select stays lazy: it walks the survivor mask
                // to the i-th kept element instead of compacting the selection into a dense
                // Vec first.
                if *depth == 0 {
                    if let Kind::Select {
                        source,
                        param,
                        pred,
                    } = &base.kind
                    {
                        let (src, mask, _len, _elem_ty) = self.select_mask(source, *param, pred)?;
                        let index = self.expr(index)?;
                        return self.call_rt(
                            self.rt.sel_at,
                            &[
                                src.into(),
                                mask.into(),
                                index,
                                self.ctx
                                    .i32_type()
                                    .const_int(*elem_is_record as u64, false)
                                    .into(),
                            ],
                            "sel_at",
                        );
                    }
                }
                let base = self.expr(base)?;
                let index = self.expr(index)?;
                self.call_rt(
                    self.rt.at,
                    &[
                        base,
                        index,
                        i64t.const_int(*depth as u64, false).into(),
                        self.ctx
                            .i32_type()
                            .const_int(*elem_is_record as u64, false)
                            .into(),
                    ],
                    "at",
                )?
            }

            Kind::Slice {
                base,
                start,
                end,
                depth,
            } => {
                let i64t = self.ctx.i64_type();
                let base = self.expr(base)?;
                let lo = match start {
                    Some(s) => self.expr(s)?,
                    // A bound left out folds to the array's own boundary inside the clamp.
                    None => i64t.const_int(i64::MIN as u64, true).into(),
                };
                let hi = match end {
                    Some(e) => self.expr(e)?,
                    None => i64t.const_int(i64::MAX as u64, true).into(),
                };
                self.call_rt(
                    self.rt.vec_slice,
                    &[base, lo, hi, i64t.const_int(*depth as u64, false).into()],
                    "slice",
                )?
            }

            // The same tag chain the enum printer uses, but each arm computes the body instead
            // of a rendering: compare the box's tag against a variant arm's index, evaluate a
            // guard arm's own Bool, bind the payload slot where the arm asks for it. A total
            // chain's last arm runs without a test, the checker having proved nothing else can
            // reach it; a partial chain tests every arm, stores each body as the present Opt,
            // and falls through to the null pointer that is the absent one.
            Kind::Match {
                subject,
                arms,
                partial,
            } => {
                let variants = match &subject.ty {
                    Type::Enum { .. } => ty::variants(self.enums, &subject.ty),
                    // A pure guard chain needs no variant table; nothing below looks one up.
                    _ => Vec::new(),
                };
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let i64t = self.ctx.i64_type();
                let result_ty = self.llvm_type(&t.ty)?;
                let slot = self
                    .builder
                    .build_alloca(result_ty, "matched")
                    .map_err(|e| e.to_string())?;
                // The subject is only read as a whole value when a variant arm needs its tag or
                // payload. A pure guard chain never touches it, which matters when the subject is
                // a struct-of-arrays cursor: `.` bound inside `map`/`select` over a Vec of records
                // cannot be materialised, only its fields read, and a guard chain over `.` reads
                // fields exclusively.
                let needs_subject = arms.iter().any(|a| a.variant.is_some());
                let subj = needs_subject.then(|| self.expr(subject)).transpose()?;
                let tag = match subj {
                    Some(subj) => Some(
                        self.call_rt(self.rt.rec_get, &[subj, i64t.const_zero().into()], "tag")?
                            .into_int_value(),
                    ),
                    None => None,
                };
                let done = self.ctx.append_basic_block(function, "match.done");

                for (i, arm) in arms.iter().enumerate() {
                    let arm_block = self.ctx.append_basic_block(function, "match.arm");
                    if *partial || i + 1 < arms.len() {
                        let is = self.arm_test(tag, &variants, arm)?;
                        let next = self.ctx.append_basic_block(function, "match.next");
                        self.builder
                            .build_conditional_branch(is, arm_block, next)
                            .map_err(|e| e.to_string())?;
                        self.builder.position_at_end(arm_block);
                        self.match_arm(subj, &variants, arm, slot, *partial)?;
                        self.builder
                            .build_unconditional_branch(done)
                            .map_err(|e| e.to_string())?;
                        self.builder.position_at_end(next);
                    } else {
                        self.builder
                            .build_unconditional_branch(arm_block)
                            .map_err(|e| e.to_string())?;
                        self.builder.position_at_end(arm_block);
                        self.match_arm(subj, &variants, arm, slot, *partial)?;
                        self.builder
                            .build_unconditional_branch(done)
                            .map_err(|e| e.to_string())?;
                    }
                }

                // Every arm declined: the builder sits in the last `match.next` block, and the
                // absent Opt is the null pointer, the same encoding tl_at uses.
                if *partial {
                    let none = self.ctx.ptr_type(AddressSpace::default()).const_null();
                    self.builder
                        .build_store(slot, none)
                        .map_err(|e| e.to_string())?;
                    self.builder
                        .build_unconditional_branch(done)
                        .map_err(|e| e.to_string())?;
                }

                self.builder.position_at_end(done);
                self.builder
                    .build_load(result_ty, slot, "matched")
                    .map_err(|e| e.to_string())?
            }
            Kind::Closure { param, body } => self.closure(&t.ty, *param, body)?,

            Kind::ApplyClosure { closure, arg } => self.apply_closure(closure, arg, &t.ty)?,
        })
    }

    /// The Bool that says whether `arm` is the one to run: a variant arm compares the subject's
    /// tag, a guard arm is its own guard.
    fn arm_test(
        &mut self,
        tag: Option<IntValue<'ctx>>,
        variants: &[(String, Option<Type>)],
        arm: &tir::MatchArm,
    ) -> Result<IntValue<'ctx>, String> {
        Ok(match (&arm.variant, &arm.guard) {
            (Some(variant), _) => {
                let vi = variants
                    .iter()
                    .position(|(n, _)| n == variant)
                    .ok_or_else(|| format!("`{variant}` is not a variant"))?;
                self.builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        tag.ok_or("a variant arm with no tag read")?,
                        self.ctx.i64_type().const_int(vi as u64, false),
                        "is",
                    )
                    .map_err(|e| e.to_string())?
            }
            (None, Some(g)) => self.expr(g)?.into_int_value(),
            (None, None) => return Err("a default arm that is not last".into()),
        })
    }

    /// Bind the payload local an arm asks for, read out of the subject's box.
    fn bind_payload(
        &mut self,
        subj: Option<BasicValueEnum<'ctx>>,
        variants: &[(String, Option<Type>)],
        arm: &tir::MatchArm,
    ) -> Result<(), String> {
        if let Some(pid) = arm.payload {
            let variant = arm.variant.as_ref().ok_or("a payload on a default arm")?;
            let pty = variants
                .iter()
                .find(|(n, _)| n == variant)
                .and_then(|(_, p)| p.as_ref())
                .ok_or_else(|| format!("`{variant}` has no payload"))?;
            let subj = subj.ok_or("a payload arm with no subject value")?;
            let raw = self
                .call_rt(
                    self.rt.rec_get,
                    &[subj, self.ctx.i64_type().const_int(1, false).into()],
                    "payload",
                )?
                .into_int_value();
            let p = self.read_slot(raw, pty)?;
            self.locals.insert(pid, Slot::Value(p));
        }
        Ok(())
    }

    /// Compute one arm's body into `slot`, binding the payload local first when the arm has
    /// one. In a partial chain the slot holds an Opt, so the body is boxed into the present
    /// one on its way in.
    fn match_arm(
        &mut self,
        subj: Option<BasicValueEnum<'ctx>>,
        variants: &[(String, Option<Type>)],
        arm: &tir::MatchArm,
        slot: PointerValue<'ctx>,
        partial: bool,
    ) -> Result<(), String> {
        self.bind_payload(subj, variants, arm)?;
        let mut v = self.expr(&arm.body)?;
        if partial {
            let raw = self.to_slot(v, &arm.body.ty)?;
            v = self.call_rt(self.rt.opt_some, &[raw.into()], "some")?;
        }
        self.builder
            .build_store(slot, v)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Which runtime sorter a Vec element needs: only Str's raw slot is a pointer wanting
    /// its own comparator; Int, Int64, and Char share the int64 sort. Split out of `expr`'s
    /// match arm to keep the selection from deepening it (kantord/toylang#86), the
    /// `fields_lit` precedent.
    fn sort_runtime<'r>(rt: &Runtime<'r>, elem: &Type) -> FunctionValue<'r> {
        if *elem == Type::Str {
            rt.vec_sort_str
        } else {
            rt.vec_sort_int
        }
    }

    fn call_rt(
        &self,
        f: FunctionValue<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let args: Vec<_> = args.iter().map(|a| (*a).into()).collect();
        self.builder
            .build_call(f, &args, name)
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("{name} returned nothing"))
    }

    /// Wrapping 32-bit arithmetic over an i64 representation.
    ///
    /// Computing in i64 and then narrowing is what makes this total. `MIN / -1` overflows i32 but
    /// not i64, so it produces 2^31 and narrows back to MIN, with no branch and no hardware trap
    /// -- the case that costs C and Rust a check costs nothing here. Only a zero divisor is left,
    /// and it is the only way arithmetic can fail.
    fn arith(
        &mut self,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        if matches!(op, BinOp::Div | BinOp::Rem) {
            self.trap_if_zero(rhs)?;
        }
        let wide = match op {
            BinOp::Add => self.builder.build_int_add(lhs, rhs, "add"),
            BinOp::Sub => self.builder.build_int_sub(lhs, rhs, "sub"),
            BinOp::Mul => self.builder.build_int_mul(lhs, rhs, "mul"),
            BinOp::Div => self.builder.build_int_signed_div(lhs, rhs, "div"),
            BinOp::Rem => self.builder.build_int_signed_rem(lhs, rhs, "rem"),
            other => return Err(format!("{other} is not arithmetic")),
        }
        .map_err(|e| e.to_string())?;
        Ok(self.narrow_to_i32(wide)?.into())
    }

    /// One arithmetic operation at the width `ty` names: `arith`'s narrowing 32-bit path, or
    /// `arith64`'s native one.
    fn arith_at(
        &mut self,
        ty: &Type,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        if *ty == Type::Int64 {
            self.arith64(op, lhs, rhs)
        } else {
            self.arith(op, lhs, rhs)
        }
    }

    /// Wrapping 64-bit arithmetic, the width `arith` computes in with the narrowing left off.
    ///
    /// `+`, `-` and `*` wrap on their own: an LLVM add without the nsw flag is defined to.
    /// Division cannot borrow `arith`'s compute-wide trick at this width -- there is no wider
    /// register to hide `MIN / -1` in, and `sdiv` on exactly that pair is undefined (a
    /// hardware trap on x86) -- so the pair is computed in i128 and truncated, the same
    /// "computing wide and narrowing is what makes this total" argument one level up. LLVM
    /// lowers i128 division to a compiler-rt call `cc` already links.
    fn arith64(
        &mut self,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let done = match op {
            BinOp::Add => self.builder.build_int_add(lhs, rhs, "add"),
            BinOp::Sub => self.builder.build_int_sub(lhs, rhs, "sub"),
            BinOp::Mul => self.builder.build_int_mul(lhs, rhs, "mul"),
            BinOp::Div | BinOp::Rem => {
                self.trap_if_zero(rhs)?;
                let i128t = self.ctx.i128_type();
                let lw = self
                    .builder
                    .build_int_s_extend(lhs, i128t, "lw")
                    .map_err(|e| e.to_string())?;
                let rw = self
                    .builder
                    .build_int_s_extend(rhs, i128t, "rw")
                    .map_err(|e| e.to_string())?;
                let wide = match op {
                    BinOp::Div => self.builder.build_int_signed_div(lw, rw, "div"),
                    _ => self.builder.build_int_signed_rem(lw, rw, "rem"),
                }
                .map_err(|e| e.to_string())?;
                self.builder
                    .build_int_truncate(wide, self.ctx.i64_type(), "narrow")
            }
            other => return Err(format!("{other} is not arithmetic")),
        }
        .map_err(|e| e.to_string())?;
        Ok(done.into())
    }

    /// Native IEEE 754 arithmetic, unlike `arith`/`arith64`: a Float is hardware-width already,
    /// so there is no wrapping or narrowing to do, and division by zero is not this backend's
    /// problem to trap -- `fdiv` already returns Infinity or NaN, the ADR 0007 / Q37 ruling's
    /// own answer (division by zero returns Infinity, matching IEEE).
    fn arith_float(
        &mut self,
        op: BinOp,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let done = match op {
            BinOp::Add => self.builder.build_float_add(lhs, rhs, "fadd"),
            BinOp::Sub => self.builder.build_float_sub(lhs, rhs, "fsub"),
            BinOp::Mul => self.builder.build_float_mul(lhs, rhs, "fmul"),
            BinOp::Div => self.builder.build_float_div(lhs, rhs, "fdiv"),
            other => return Err(format!("{other} is not arithmetic on Float")),
        }
        .map_err(|e| e.to_string())?;
        Ok(done.into())
    }

    /// Sign-extend the low 32 bits, which is the wrap.
    fn narrow_to_i32(&self, wide: IntValue<'ctx>) -> Result<IntValue<'ctx>, String> {
        let narrow = self
            .builder
            .build_int_truncate(wide, self.ctx.i32_type(), "narrow")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_int_s_extend(narrow, self.ctx.i64_type(), "wrapped")
            .map_err(|e| e.to_string())
    }

    fn trap_if_zero(&mut self, divisor: IntValue<'ctx>) -> Result<(), String> {
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or("no function to branch in")?;
        let is_zero = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                divisor,
                self.ctx.i64_type().const_zero(),
                "zero",
            )
            .map_err(|e| e.to_string())?;
        let fail = self.ctx.append_basic_block(function, "div.zero");
        let ok = self.ctx.append_basic_block(function, "div.ok");
        self.builder
            .build_conditional_branch(is_zero, fail, ok)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(fail);
        self.builder
            .build_call(self.rt.div_by_zero, &[], "")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unreachable()
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(ok);
        Ok(())
    }

    /// Structural equality on a composite, walked at compile time because the type is known:
    /// a record compares field by field, an enum compares tags and then the payload of the
    /// variant they share (kantord/toylang#68). Every case is one i1 in the block the builder
    /// is left positioned at, so a caller can `and` the result straight into another.
    ///
    /// A Vec cannot appear anywhere inside `ty` -- the checker refuses a comparison whose
    /// operand carries one, since whether a Vec compares as a whole value is Q2 -- so nothing
    /// here has to loop.
    fn equal(
        &mut self,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<IntValue<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        match ty {
            Type::Str => {
                let same = self.call_rt(self.rt.str_eq, &[l, r], "streq")?;
                self.builder
                    .build_int_compare(
                        IntPredicate::NE,
                        same.into_int_value(),
                        i64t.const_zero(),
                        "streq",
                    )
                    .map_err(|e| e.to_string())
            }
            Type::Int | Type::Int64 | Type::Bool | Type::Char => self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    l.into_int_value(),
                    r.into_int_value(),
                    "eq",
                )
                .map_err(|e| e.to_string()),
            // OEQ, the same ordered predicate the top-level `==` uses: NaN is not equal to
            // itself here either, matching JS's `===` on two numbers (JS has no separate float
            // path -- every JS number already carries these semantics).
            Type::Float => self
                .builder
                .build_float_compare(
                    FloatPredicate::OEQ,
                    l.into_float_value(),
                    r.into_float_value(),
                    "feq",
                )
                .map_err(|e| e.to_string()),
            Type::Record(fields) => {
                let mut acc = self.ctx.bool_type().const_int(1, false);
                for (name, fty) in fields {
                    let lf = self.field_of(l, ty, name, fty)?;
                    let rf = self.field_of(r, ty, name, fty)?;
                    let same = self.equal(lf, rf, fty)?;
                    acc = self
                        .builder
                        .build_and(acc, same, "eq.field")
                        .map_err(|e| e.to_string())?;
                }
                Ok(acc)
            }
            Type::Enum { .. } if ty.as_opt().is_some() => {
                let inner = ty.as_opt().expect("guarded").clone();
                self.equal_opt(l, r, &inner)
            }
            Type::Enum { variants, .. } => self.equal_enum(l, r, variants),
            other => unreachable!("the checker refuses `==` on {other}"),
        }
    }

    /// The Opt case of `equal`. Absence is the null pointer here, so two Opts agree when they
    /// agree about being present, and, when both are, about what they hold.
    fn equal_opt(
        &mut self,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
        inner: &Type,
    ) -> Result<IntValue<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        let function = self.enclosing_function()?;
        let boolt = self.ctx.bool_type();
        let slot = self
            .builder
            .build_alloca(boolt, "eq")
            .map_err(|e| e.to_string())?;

        let mut present = Vec::new();
        for (v, name) in [(l, "l.some"), (r, "r.some")] {
            let some = self.call_rt(self.rt.opt_is_some, &[v], name)?;
            present.push(
                self.builder
                    .build_int_compare(
                        IntPredicate::NE,
                        some.into_int_value(),
                        i64t.const_zero(),
                        name,
                    )
                    .map_err(|e| e.to_string())?,
            );
        }
        let agree = self
            .builder
            .build_int_compare(IntPredicate::EQ, present[0], present[1], "eq.presence")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(slot, agree)
            .map_err(|e| e.to_string())?;
        let both = self
            .builder
            .build_and(present[0], present[1], "eq.both")
            .map_err(|e| e.to_string())?;

        let inside = self.ctx.append_basic_block(function, "eq.some");
        let done = self.ctx.append_basic_block(function, "eq.done");
        self.builder
            .build_conditional_branch(both, inside, done)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(inside);
        let lp = self
            .call_rt(self.rt.opt_get, &[l], "l.payload")?
            .into_int_value();
        let rp = self
            .call_rt(self.rt.opt_get, &[r], "r.payload")?
            .into_int_value();
        let lp = self.read_slot(lp, inner)?;
        let rp = self.read_slot(rp, inner)?;
        let same = self.equal(lp, rp, inner)?;
        self.builder
            .build_store(slot, same)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(done)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(done);
        Ok(self
            .builder
            .build_load(boolt, slot, "eq")
            .map_err(|e| e.to_string())?
            .into_int_value())
    }

    /// The declared-enum case of `equal`. Different tags settle it; equal tags leave one variant
    /// to compare, so the payload walk runs only over the same two-slot box `show` reads.
    fn equal_enum(
        &mut self,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
        variants: &[(String, Option<Type>)],
    ) -> Result<IntValue<'ctx>, String> {
        if variants.is_empty() {
            return Err(unsupported("comparing an enum with no variants"));
        }
        let i64t = self.ctx.i64_type();
        let boolt = self.ctx.bool_type();

        let lt = self
            .call_rt(self.rt.rec_get, &[l, i64t.const_zero().into()], "l.tag")?
            .into_int_value();
        let rt = self
            .call_rt(self.rt.rec_get, &[r, i64t.const_zero().into()], "r.tag")?
            .into_int_value();
        let same_tag = self
            .builder
            .build_int_compare(IntPredicate::EQ, lt, rt, "eq.tag")
            .map_err(|e| e.to_string())?;
        // An enum whose variants all stand for themselves is settled by the tag, with no second
        // slot to look at.
        if variants.iter().all(|(_, payload)| payload.is_none()) {
            return Ok(same_tag);
        }

        let function = self.enclosing_function()?;
        let slot = self
            .builder
            .build_alloca(boolt, "eq")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(slot, same_tag)
            .map_err(|e| e.to_string())?;

        let payloads = self.ctx.append_basic_block(function, "eq.payload");
        let done = self.ctx.append_basic_block(function, "eq.done");
        self.builder
            .build_conditional_branch(same_tag, payloads, done)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(payloads);
        self.dispatch_on_tag("eq", lt, variants, done, slot, |e, _, payload, slot| {
            e.equal_payload(l, r, payload.as_ref(), slot)
        })?;

        self.builder.position_at_end(done);
        Ok(self
            .builder
            .build_load(boolt, slot, "eq")
            .map_err(|e| e.to_string())?
            .into_int_value())
    }

    /// One arm of the enum walk above: a unit variant is settled by its tag alone, so the
    /// `true` already in `slot` stands; a payload variant compares what the second slot holds.
    fn equal_payload(
        &mut self,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
        payload: Option<&Type>,
        slot: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let Some(pty) = payload else {
            return Ok(());
        };
        let payload_slot = self.ctx.i64_type().const_int(1, false);
        let lp = self
            .call_rt(self.rt.rec_get, &[l, payload_slot.into()], "l.payload")?
            .into_int_value();
        let rp = self
            .call_rt(self.rt.rec_get, &[r, payload_slot.into()], "r.payload")?
            .into_int_value();
        let lp = self.read_slot(lp, pty)?;
        let rp = self.read_slot(rp, pty)?;
        let same = self.equal(lp, rp, pty)?;
        self.builder
            .build_store(slot, same)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn enclosing_function(&self) -> Result<FunctionValue<'ctx>, String> {
        self.builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "no function to branch in".to_string())
    }

    /// The tag walk shared by `show_enum` and `equal_enum`: one arm block per variant running
    /// `arm`, each arm reached by testing the tag against the variant's index and joined at
    /// `done`. The last variant needs no test, since the type says nothing else is left; only
    /// the block-name prefix and the arm body tell the two walks apart.
    fn dispatch_on_tag<F>(
        &mut self,
        prefix: &str,
        tag: IntValue<'ctx>,
        variants: &[(String, Option<Type>)],
        done: BasicBlock<'ctx>,
        slot: PointerValue<'ctx>,
        mut arm: F,
    ) -> Result<(), String>
    where
        F: FnMut(&mut Self, &str, &Option<Type>, PointerValue<'ctx>) -> Result<(), String>,
    {
        let function = self.enclosing_function()?;
        let i64t = self.ctx.i64_type();
        for (i, (vname, payload)) in variants.iter().enumerate() {
            let arm_block = self
                .ctx
                .append_basic_block(function, &format!("{prefix}.arm"));
            if i + 1 < variants.len() {
                let next = self
                    .ctx
                    .append_basic_block(function, &format!("{prefix}.next"));
                let is = self
                    .builder
                    .build_int_compare(IntPredicate::EQ, tag, i64t.const_int(i as u64, false), "is")
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_conditional_branch(is, arm_block, next)
                    .map_err(|e| e.to_string())?;
                self.builder.position_at_end(arm_block);
                arm(self, vname, payload, slot)?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;
                self.builder.position_at_end(next);
            } else {
                self.builder
                    .build_unconditional_branch(arm_block)
                    .map_err(|e| e.to_string())?;
                self.builder.position_at_end(arm_block);
                arm(self, vname, payload, slot)?;
                self.builder
                    .build_unconditional_branch(done)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// `+`, whether it means Str concatenation (`tl_concat`) or Vec concatenation
    /// (`tl_vec_concat`, needing the element's column count the way `Flatten` does).
    fn concat(&mut self, ty: &Type, l: &Tir, r: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        let elem = ty.elem().cloned();
        let l = self.expr(l)?;
        let r = self.expr(r)?;
        match elem {
            Some(elem) => {
                let ncols = self.ctx.i64_type().const_int(Self::columns(&elem), false);
                self.call_rt(self.rt.vec_concat, &[l, r, ncols.into()], "concat")
            }
            None => self.call_rt(self.rt.concat, &[l, r], "concat"),
        }
    }

    /// `and` / `or`, as real blocks rather than LLVM's own `and`/`or`, which are bitwise and
    /// take both operands already evaluated: the right side of `x != 0 and 10 / x > 1` must not
    /// run when the left is false, and only a branch can promise that.
    fn logic(&mut self, op: LogicOp, lhs: &Tir, rhs: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        let function = self.enclosing_function()?;
        let slot_ty = self.llvm_type(&Type::Bool)?;
        let slot = self
            .builder
            .build_alloca(slot_ty, "logic")
            .map_err(|e| e.to_string())?;

        let left = self.expr(lhs)?;
        self.builder
            .build_store(slot, left)
            .map_err(|e| e.to_string())?;
        let rest = self.ctx.append_basic_block(function, "logic.rhs");
        let done = self.ctx.append_basic_block(function, "logic.done");
        // `and` reaches its right side only when the left is true, `or` only when it is false;
        // either way the left value is already in the slot for the path that skips it.
        let (yes, no) = match op {
            LogicOp::And => (rest, done),
            LogicOp::Or => (done, rest),
        };
        self.builder
            .build_conditional_branch(left.into_int_value(), yes, no)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(rest);
        let right = self.expr(rhs)?;
        self.builder
            .build_store(slot, right)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(done)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(done);
        self.builder
            .build_load(slot_ty, slot, "logic")
            .map_err(|e| e.to_string())
    }

    fn compare(&mut self, op: BinOp, lhs: &Tir, rhs: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        let operand_ty = lhs.ty.clone();
        let l = self.expr(lhs)?;
        let r = self.expr(rhs)?;

        // The checker refuses ordering on a composite, so only `==` and `!=` reach one.
        if operand_ty.is_composite() {
            let same = self.equal(l, r, &operand_ty)?;
            return Ok(match op {
                BinOp::Ne => self
                    .builder
                    .build_not(same, "ne")
                    .map_err(|e| e.to_string())?,
                _ => same,
            }
            .into());
        }

        // Float gets its own predicate table rather than joining the Int one below: ordered
        // (`O*`) predicates are false whenever either operand is NaN, which is what IEEE and
        // this language's own Q37 ruling (NaN admitted, comparisons follow IEEE) want for `==`,
        // `<`, `<=`, `>`, `>=` -- but `!=` needs the unordered predicate (`UNE`), since `NaN !=
        // NaN` is true. Reusing `OEQ`'s negation for `!=` would get that backwards.
        if operand_ty == Type::Float {
            let predicate = match op {
                BinOp::Eq => FloatPredicate::OEQ,
                BinOp::Ne => FloatPredicate::UNE,
                BinOp::Lt => FloatPredicate::OLT,
                BinOp::Le => FloatPredicate::OLE,
                BinOp::Gt => FloatPredicate::OGT,
                BinOp::Ge => FloatPredicate::OGE,
                other => return Err(format!("{other} is not a comparison")),
            };
            return Ok(self
                .builder
                .build_float_compare(
                    predicate,
                    l.into_float_value(),
                    r.into_float_value(),
                    "fcmp",
                )
                .map_err(|e| e.to_string())?
                .into());
        }

        // A Bool is an i1, which a signed predicate reads as -1 or 0, so `true > false` came
        // out false; the unsigned ones put false before true like every other backend.
        let signed = operand_ty != Type::Bool;
        let predicate = match op {
            BinOp::Eq => IntPredicate::EQ,
            BinOp::Ne => IntPredicate::NE,
            BinOp::Lt if signed => IntPredicate::SLT,
            BinOp::Le if signed => IntPredicate::SLE,
            BinOp::Gt if signed => IntPredicate::SGT,
            BinOp::Ge if signed => IntPredicate::SGE,
            BinOp::Lt => IntPredicate::ULT,
            BinOp::Le => IntPredicate::ULE,
            BinOp::Gt => IntPredicate::UGT,
            BinOp::Ge => IntPredicate::UGE,
            other => return Err(format!("{other} is not a comparison")),
        };

        // Comparing two integers is one instruction. Comparing two strings is a runtime call
        // whose result is then compared to zero, which makes every operator fall out of the
        // same predicate table.
        let (left, right) = match operand_ty {
            Type::Str => match op {
                // tl_str_eq answers equality directly, so its result is compared against 1 and
                // the EQ/NE predicate then reads correctly for both operators.
                BinOp::Eq | BinOp::Ne => {
                    let call = self.call_rt(self.rt.str_eq, &[l, r], "streq")?;
                    (
                        call.into_int_value(),
                        self.ctx.i64_type().const_int(1, false),
                    )
                }
                // tl_str_cmp returns -1, 0 or 1, so ordering is that against zero.
                _ => {
                    let call = self.call_rt(self.rt.str_cmp, &[l, r], "strcmp")?;
                    (call.into_int_value(), self.ctx.i64_type().const_zero())
                }
            },
            Type::Int | Type::Int64 | Type::Bool | Type::Char => {
                (l.into_int_value(), r.into_int_value())
            }
            other => unreachable!("the checker refuses `{op}` on {other}"),
        };

        Ok(self
            .builder
            .build_int_compare(predicate, left, right, "cmp")
            .map_err(|e| e.to_string())?
            .into())
    }

    /// A stream-typed `jsonlines` program, compiled as a loop over `tl_read_one_input` (or, for
    /// a `lines` source, `tl_read_one_line`) instead of
    /// `self.expr(&program.body)` + one `print` at the end. `tl_print` already writes with a raw
    /// `write(1, ...)` syscall and needs no explicit flush, unlike every other backend that had
    /// to add one -- the one backend with no libc stdio buffering to fight.
    ///
    /// Each record is bound with `Slot::Value`, the same as `Kind::Input` binds a single read
    /// value, not `Slot::Cursor`: the struct-of-arrays optimisation `map`/`select` use for a
    /// cursor into an existing Vec's columns does not apply here, since there is no Vec -- each
    /// record arrives as its own one-off parsed value, exactly like `input` already is.
    fn fused_main(&mut self, program: &Program, fusion: &Fusion<'_>) -> Result<(), String> {
        // A `range` source brings its own bound to count against; the stdin sources read from
        // the runtime instead. Both settle the element type the same way the eager path does.
        let (elem_ty, bound) = match fusion.source {
            crate::tir::Source::Inputs => (
                program
                    .inputs
                    .as_ref()
                    .ok_or("an inputs source recorded its element")?
                    .clone(),
                None,
            ),
            crate::tir::Source::Lines => (Type::Str, None),
            crate::tir::Source::Range(bound) => {
                let bound = self.expr(bound)?.into_int_value();
                (Type::Int, Some(bound))
            }
        };
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or("no function to emit a loop into")?;

        let i64t = self.ctx.i64_type();
        let out_slot = self
            .builder
            .build_alloca(i64t, "next_input")
            .map_err(|e| e.to_string())?;
        // The range counter, -1 so the loop condition reads 0 on the first pass.
        let counter = self
            .builder
            .build_alloca(i64t, "range_i")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter, i64t.const_int(-1i64 as u64, true))
            .map_err(|e| e.to_string())?;

        let cond = self.ctx.append_basic_block(function, "fused.cond");
        let body = self.ctx.append_basic_block(function, "fused.body");
        let end = self.ctx.append_basic_block(function, "fused.end");

        self.builder
            .build_unconditional_branch(cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond);
        let has_more = match (fusion.source, bound) {
            (crate::tir::Source::Range(_), Some(n)) => self.range_has_more(counter, n)?,
            (source, None) => self.source_has_more(source, &elem_ty, out_slot)?,
            _ => unreachable!("the source and bound match the arm that produced them"),
        };
        self.builder
            .build_conditional_branch(has_more, body, end)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body);
        let mut current = match (fusion.source, bound) {
            (crate::tir::Source::Range(_), _) => self
                .builder
                .build_load(i64t, counter, "range_elem")
                .map_err(|e| e.to_string())?
                .into_int_value()
                .into(),
            _ => {
                let slot = self
                    .builder
                    .build_load(i64t, out_slot, "slot")
                    .map_err(|e| e.to_string())?
                    .into_int_value();
                self.read_slot(slot, &elem_ty)?
            }
        };
        let mut current_ty = elem_ty;

        for (i, stage) in fusion.stages.iter().enumerate() {
            match stage {
                Stage::Map { param, body } => {
                    self.locals.insert(*param, Slot::Value(current));
                    current = self.expr(body)?;
                    current_ty = body.ty.clone();
                }
                Stage::Select { param, pred } => {
                    self.locals.insert(*param, Slot::Value(current));
                    let keep = self.expr(pred)?.into_int_value();
                    let keep_block = self
                        .ctx
                        .append_basic_block(function, &format!("fused.keep{i}"));
                    self.builder
                        .build_conditional_branch(keep, keep_block, cond)
                        .map_err(|e| e.to_string())?;
                    self.builder.position_at_end(keep_block);
                }
            }
        }

        // Always the JSON rendering, never `print`'s top-level raw-Str rule: each line is one
        // JSON value (that is what jsonlines promises), so a Str element prints quoted here
        // exactly as the eager path's per-element `show` does.
        let shown = self.show(current, &current_ty)?;
        self.builder
            .build_call(self.rt.print, &[shown.into()], "")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(end);
        Ok(())
    }

    /// The loop condition for a stdin-backed fused source: reads one entry into `out_slot`
    /// and reports whether there was one.
    fn source_has_more(
        &mut self,
        source: Source<'_>,
        elem_ty: &Type,
        out_slot: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, String> {
        let i32t = self.ctx.i32_type();
        let got = match source {
            Source::Inputs => {
                let descriptor = self.string_const(&descriptor(self.enums, elem_ty));
                self.call_rt(
                    self.rt.read_one_input,
                    &[descriptor.into(), out_slot.into()],
                    "got",
                )?
                .into_int_value()
            }
            Source::Lines => self
                .call_rt(self.rt.read_one_line, &[out_slot.into()], "got")?
                .into_int_value(),
            Source::Range(_) => unreachable!("a range source has its own loop condition"),
        };
        self.builder
            .build_int_compare(IntPredicate::NE, got, i32t.const_zero(), "has_more")
            .map_err(|e| e.to_string())
    }

    /// The loop condition for a `range` source: increments the counter and reports whether the
    /// new value is below the bound. Incrementing in the condition (rather than at the end of
    /// the body) is what keeps a `select`'s continue -- a branch back to the condition -- from
    /// skipping a count.
    fn range_has_more(
        &mut self,
        counter: PointerValue<'ctx>,
        n: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, String> {
        let i64t = self.ctx.i64_type();
        let raw = self
            .builder
            .build_load(i64t, counter, "raw")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let i = self
            .builder
            .build_int_add(raw, i64t.const_int(1, false), "i")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter, i)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_int_compare(IntPredicate::SLT, i, n, "has_more")
            .map_err(|e| e.to_string())
    }

    /// One printer function per recursive enum the program prints, declared before any body is
    /// emitted so that a body -- a printer's own included -- can call one.
    ///
    /// The recursion in the type becomes recursion in the emitted program rather than in this
    /// compiler (kantord/toylang#94): expanding a recursive enum inline, the way `show` expands
    /// every other type, has no bottom. Every value here is a pointer, so one signature covers
    /// all of them: the enum's box in, the rendered string out.
    fn declare_printers(&mut self, program: &Program) -> Result<(), String> {
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        let printed = tir::printed_recursive_enums(program);
        for ty in &printed {
            let f =
                self.module
                    .add_function(&ty.show_fn(), ptr.fn_type(&[ptr.into()], false), None);
            self.printers.insert(ty.ident(), f);
        }
        for ty in &printed {
            let f = self.printers[&ty.ident()];
            let entry = self.ctx.append_basic_block(f, "entry");
            self.builder.position_at_end(entry);
            let value = f.get_nth_param(0).expect("declared with one parameter");
            let shown = self.show_enum(value, ty)?;
            self.builder
                .build_return(Some(&shown))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
    fn print(&mut self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<(), String> {
        let as_str = match ty {
            Type::Str | Type::Sink => value,
            other => self.show(value, other)?,
        };
        self.builder
            .build_call(self.rt.print, &[as_str.into()], "")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// The type descriptor the runtime's JSON parser reads, so it only ever looks for the shape the
/// program declared. See the grammar at the top of runtime-rs/src/json.rs.
fn descriptor(enums: &Enums, ty: &Type) -> String {
    /// `open` is the enums whose descriptors this one is already inside. A recursive enum names
    /// itself back as `@Name` rather than being spelled out again, which is what the runtime's
    /// own back-reference resolves (kantord/toylang#94) -- and what keeps this from writing a
    /// string that has no end.
    fn walk(enums: &Enums, ty: &Type, open: &mut Vec<Type>) -> String {
        match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Fn(..) => unreachable!("a closure's type never reaches a backend"),
            // Stream is unspellable in a type annotation, so `input`'s declared type -- the only
            // thing this function is ever called on -- can never contain one.
            Type::Stream(_) => unreachable!("Stream cannot be declared, so input never has one"),
            Type::Seq(..) => unreachable!(
                "a Seq value cannot reach a backend; no source produces one yet (ADR 0008 emission is a follow-up)"
            ),
            Type::Sink => unreachable!("input cannot be a sink, refused by the checker"),
            Type::Str => "s".to_string(),
            Type::Int => "i".to_string(),
            // The checker refuses Int64 anywhere in an input type: its wire codec is undecided.
            Type::Int64 => unreachable!("input cannot contain an Int64, refused by the checker"),
            Type::Float => "f".to_string(),
            Type::Bool => "b".to_string(),
            // The checker refuses Char anywhere in an input type: it has no wire form.
            Type::Char => unreachable!("input cannot contain a Char, refused by the checker"),
            Type::Vec(elem) => format!("[{}", walk(enums, elem, open)),
            // Opt has no spelling in the type syntax, so an input type can never contain one.
            // Declaration order, because the entry's position is the tag the compiled code
            // tests against. The name rides along for the runtime's mismatch messages, and is
            // what a back-reference to this enum names.
            Type::Enum { name, .. } => {
                if open.contains(ty) {
                    return format!("@{name}");
                }
                open.push(ty.clone());
                let variants = ty::variants(enums, ty);
                let body: Vec<String> = variants
                    .iter()
                    .map(|(v, payload)| match payload {
                        None => v.clone(),
                        Some(p) => format!("{v}:{}", walk(enums, p, open)),
                    })
                    .collect();
                open.pop();
                format!("e{{{},{name},{}}}", variants.len(), body.join(","))
            }
            Type::Record(fields) => {
                let body: Vec<String> = fields
                    .iter()
                    .map(|(name, t)| format!("{name}:{}", walk(enums, t, open)))
                    .collect();
                format!("{{{},{}}}", fields.len(), body.join(","))
            }
        }
    }
    walk(enums, ty, &mut Vec::new())
}

fn build_module<'ctx>(ctx: &'ctx Context, program: &Program) -> Result<Module<'ctx>, String> {
    let mut e = Emitter::new(ctx, &program.enums);

    // The global exists before any body is emitted, since a function body may hold an `Input`
    // node; main stores into it below, before anything can run.
    if program.input.is_some() {
        let i64t = ctx.i64_type();
        let global = e.module.add_global(i64t, None, "tl_input_slot");
        global.set_initializer(&i64t.const_zero());
        e.input_slot = Some(global.as_pointer_value());
    }

    // Declared before any body, so a call to a function defined further down resolves. The
    // checker allows that, and it is where the Lua backend was wrong at prototype 1 step 3.
    for func in &program.funcs {
        e.declare(func)?;
    }
    e.declare_printers(program)?;
    for func in &program.funcs {
        e.define(func)?;
    }

    let i32t = ctx.i32_type();
    let main = e
        .module
        .add_function("main", i32t.fn_type(&[], false), None);
    e.builder
        .position_at_end(ctx.append_basic_block(main, "entry"));
    e.params.clear();
    e.locals.clear();

    if let Some(ty) = &program.input {
        let global = e.input_slot.expect("created above whenever input is Some");
        let descriptor = e.string_const(&descriptor(&program.enums, ty));
        let slot = e.call_rt(e.rt.read_input, &[descriptor.into()], "input")?;
        e.builder
            .build_store(global, slot.into_int_value())
            .map_err(|err| err.to_string())?;
    }

    if let Some(fusion) = tir::fusion(program) {
        e.fused_main(program, &fusion)?;
    } else {
        let body = e.expr(&program.body)?;
        e.print(body, &program.body.ty)?;
    }
    e.builder
        .build_return(Some(&i32t.const_zero()))
        .map_err(|err| err.to_string())?;

    e.module
        .verify()
        .map_err(|err| format!("LLVM rejected the module: {err}"))?;
    Ok(e.module)
}

pub fn to_ir(program: &Program) -> Result<String, String> {
    let ctx = Context::create();
    let module = build_module(&ctx, program)?;
    Ok(module.print_to_string().to_string())
}

pub fn compile_to_object(program: &Program, object: &Path) -> Result<(), String> {
    let ctx = Context::create();
    let module = build_module(&ctx, program)?;

    Target::initialize_native(&InitializationConfig::default())?;
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).map_err(|e| e.to_string())?;
    let cpu = TargetMachine::get_host_cpu_name();
    let features = TargetMachine::get_host_cpu_features();
    let machine = target
        .create_target_machine(
            &triple,
            cpu.to_str().map_err(|e| e.to_string())?,
            features.to_str().map_err(|e| e.to_string())?,
            OptimizationLevel::None,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .ok_or_else(|| "LLVM has no target machine for this host".to_string())?;

    machine
        .write_to_file(&module, FileType::Object, object)
        .map_err(|e| e.to_string())
}
