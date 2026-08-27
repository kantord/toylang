//! The native backend.
//!
//! Unlike Lua and JavaScript this does not end at a string of source. It ends at an object file,
//! which is not a program: linking is still someone else's job, so a native build shells out to
//! `cc`, which also compiles `runtime/toylang.c` alongside it.
//!
//! Everything it cannot compile yet returns a named error rather than being silently absent, so
//! the gap between this and the other two backends is a visible, shrinking list.

use std::collections::HashMap;
use std::path::Path;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};

use crate::ast::BinOp;
use crate::tir::{self, Builtin, Func, Fusion, Kind, LocalId, Program, Stage, Tir};
use crate::ty::Type;

/// The C source linked into every native binary.
///
/// Embedded rather than read from disk so a built `toylang` does not depend on its own source
/// tree still being there.
pub const RUNTIME_C: &str = include_str!("../runtime/toylang.c");

fn unsupported(what: &str) -> String {
    format!("the native backend cannot compile {what} yet")
}

struct Runtime<'ctx> {
    concat: FunctionValue<'ctx>,
    int_to_str: FunctionValue<'ctx>,
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
    vec_column: FunctionValue<'ctx>,
    rec_get: FunctionValue<'ctx>,
    rec_new: FunctionValue<'ctx>,
    collect_lines: FunctionValue<'ctx>,
    rec_set: FunctionValue<'ctx>,
    read_input: FunctionValue<'ctx>,
    read_inputs: FunctionValue<'ctx>,
    read_one_input: FunctionValue<'ctx>,
    rec_from_vec: FunctionValue<'ctx>,
    at: FunctionValue<'ctx>,
    opt_is_some: FunctionValue<'ctx>,
    opt_get: FunctionValue<'ctx>,
    unwrap: FunctionValue<'ctx>,
    div_by_zero: FunctionValue<'ctx>,
    range: FunctionValue<'ctx>,
    vec_tail: FunctionValue<'ctx>,
    vec_concat: FunctionValue<'ctx>,
}

/// What a compiler-introduced binding holds.
///
/// `select` over a Vec of records binds `.` to a position rather than to a value, because
/// struct-of-arrays spreads an element across columns and materialising it would undo the point
/// of the layout. A cursor is only ever consumed by field access, which reads one column.
#[derive(Clone, Copy)]
enum Slot<'ctx> {
    Value(BasicValueEnum<'ctx>),
    Cursor { vec: PointerValue<'ctx>, index: IntValue<'ctx> },
}

struct Emitter<'ctx> {
    ctx: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    rt: Runtime<'ctx>,
    funcs: HashMap<String, FunctionValue<'ctx>>,
    locals: HashMap<LocalId, Slot<'ctx>>,
    params: HashMap<String, BasicValueEnum<'ctx>>,
    next_global: usize,
}

impl<'ctx> Emitter<'ctx> {
    fn new(ctx: &'ctx Context) -> Emitter<'ctx> {
        let module = ctx.create_module("toylang");
        let ptr = ctx.ptr_type(AddressSpace::default());
        let i64t = ctx.i64_type();
        let i32t = ctx.i32_type();

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
                ctx.void_type().fn_type(&[ptr.into(), i64t.into(), i64t.into(), i64t.into()], false),
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
                ctx.void_type().fn_type(&[ptr.into(), i64t.into(), i64t.into()], false),
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
            rec_new: module.add_function(
                "tl_rec_new",
                ptr.fn_type(&[i64t.into()], false),
                None,
            ),
            collect_lines: module.add_function(
                "tl_collect_lines",
                ptr.fn_type(&[], false),
                None,
            ),
            rec_set: module.add_function(
                "tl_rec_set",
                ctx.void_type().fn_type(&[ptr.into(), i64t.into(), i64t.into()], false),
                None,
            ),
            read_input: module.add_function(
                "tl_read_input",
                i64t.fn_type(&[ptr.into()], false),
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
            vec_tail: module.add_function("tl_vec_tail", ptr.fn_type(&[ptr.into()], false), None),
            vec_concat: module.add_function(
                "tl_vec_concat",
                ptr.fn_type(&[ptr.into(), i64t.into()], false),
                None,
            ),
        };

        Emitter {
            ctx,
            module,
            builder: ctx.create_builder(),
            rt,
            funcs: HashMap::new(),
            locals: HashMap::new(),
            params: HashMap::new(),
            next_global: 0,
        }
    }

    fn str_struct(&self) -> StructType<'ctx> {
        self.ctx.struct_type(
            &[self.ctx.ptr_type(AddressSpace::default()).into(), self.ctx.i64_type().into()],
            false,
        )
    }

    fn llvm_type(&self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, String> {
        Ok(match ty {
            // Proven unreachable by the checker: Lines cannot be a function's parameter or
            // return type (unspellable in an annotation), cannot enter a Vec or a record, and a
            // Cond branch typed Lines would need `lines` written twice, which is refused.
            Type::Lines => unreachable!("Lines never reaches llvm_type"),
            Type::Str => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Int => self.ctx.i64_type().into(),
            Type::Bool => self.ctx.bool_type().into(),
            Type::Vec(_) | Type::Opt(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
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

    /// The type descriptor the runtime's JSON parser reads, so it only ever looks for the shape
    /// the program declared. See the grammar in runtime/toylang.c.
    fn descriptor(ty: &Type) -> String {
        match ty {
            // Lines is unspellable in a type annotation, so `input`'s declared type -- the only
            // thing this function is ever called on -- can never contain one.
            Type::Lines => unreachable!("Lines cannot be declared, so input never has one"),
            Type::Str => "s".to_string(),
            Type::Int => "i".to_string(),
            Type::Bool => "b".to_string(),
            Type::Vec(elem) => format!("[{}", Self::descriptor(elem)),
            // Opt has no spelling in the type syntax, so an input type can never contain one.
            Type::Opt(_) => unreachable!("Opt cannot be declared, so input never has one"),
            Type::Enum { .. } => unreachable!("enum-typed input is rejected by the checker"),
            Type::Record(fields) => {
                let body: Vec<String> = fields
                    .iter()
                    .map(|(name, t)| format!("{name}:{}", Self::descriptor(t)))
                    .collect();
                format!("{{{},{}}}", fields.len(), body.join(","))
            }
        }
    }

    /// A literal becomes a private constant for the bytes plus a private constant `tl_str`
    /// pointing at them. The length excludes the trailing NUL, which exists only so a debugger
    /// can print the bytes.
    fn string_const(&mut self, text: &str) -> PointerValue<'ctx> {
        let id = self.next_global;
        self.next_global += 1;

        let bytes = self.ctx.const_string(text.as_bytes(), true);
        let bytes_global =
            self.module.add_global(bytes.get_type(), None, &format!("bytes.{id}"));
        bytes_global.set_initializer(&bytes);
        bytes_global.set_constant(true);
        bytes_global.set_linkage(Linkage::Private);

        let str_ty = self.str_struct();
        let init = str_ty.const_named_struct(&[
            bytes_global.as_pointer_value().into(),
            self.ctx.i64_type().const_int(text.len() as u64, false).into(),
        ]);
        let global = self.module.add_global(str_ty, None, &format!("str.{id}"));
        global.set_initializer(&init);
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global.as_pointer_value()
    }

    fn declare(&mut self, func: &Func) -> Result<(), String> {
        let param = self.llvm_type(&func.param_ty)?;
        let ret = self.llvm_type(&func.body.ty)?;
        let args: [BasicMetadataTypeEnum; 1] = [param.into()];
        let sig = match ret {
            BasicTypeEnum::IntType(t) => t.fn_type(&args, false),
            BasicTypeEnum::PointerType(t) => t.fn_type(&args, false),
            other => return Err(unsupported(&format!("a {other:?} return"))),
        };
        // Names are prefixed for the same reason the other backends prefix: `main` is a legal
        // toylang function name and is already spoken for here.
        let value = self.module.add_function(&format!("v_{}", func.name), sig, None);
        self.funcs.insert(func.name.clone(), value);
        Ok(())
    }

    fn define(&mut self, func: &Func) -> Result<(), String> {
        let value = self.funcs[&func.name];
        let entry = self.ctx.append_basic_block(value, "entry");
        self.builder.position_at_end(entry);

        self.params.clear();
        self.locals.clear();
        let arg = value.get_nth_param(0).expect("unary");
        self.params.insert(func.param.clone(), arg);

        let body = self.expr(&func.body)?;
        self.builder.build_return(Some(&body)).map_err(|e| e.to_string())?;
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
            // Proven unreachable the same way as llvm_type: nothing can put a Lines value where
            // to_slot would be asked to pack one.
            Type::Lines => unreachable!("Lines never reaches to_slot"),
            Type::Int => value.into_int_value(),
            Type::Bool => self
                .builder
                .build_int_z_extend(value.into_int_value(), i64t, "slot")
                .map_err(|e| e.to_string())?,
            Type::Str | Type::Vec(_) | Type::Opt(_) | Type::Record(_) | Type::Enum { .. } => {
                self.builder
                    .build_ptr_to_int(value.into_pointer_value(), i64t, "slot")
                    .map_err(|e| e.to_string())?
            }
        })
    }

    fn read_slot(
        &self,
        slot: IntValue<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        Ok(match ty {
            Type::Lines => unreachable!("Lines never reaches read_slot"),
            Type::Int => slot.into(),
            Type::Bool => self
                .builder
                .build_int_truncate(slot, self.ctx.bool_type(), "elem")
                .map_err(|e| e.to_string())?
                .into(),
            Type::Str | Type::Vec(_) | Type::Opt(_) | Type::Record(_) | Type::Enum { .. } => self
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

        let counter = self.builder.build_alloca(i64t, "i").map_err(|e| e.to_string())?;
        self.builder.build_store(counter, i64t.const_zero()).map_err(|e| e.to_string())?;

        let cond = self.ctx.append_basic_block(function, "loop.cond");
        let loop_body = self.ctx.append_basic_block(function, "loop.body");
        let end = self.ctx.append_basic_block(function, "loop.end");

        self.builder.build_unconditional_branch(cond).map_err(|e| e.to_string())?;

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
        self.builder.build_conditional_branch(more, loop_body, end).map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_body);
        body(self, i)?;
        let next = self
            .builder
            .build_int_add(i, i64t.const_int(1, false), "next")
            .map_err(|e| e.to_string())?;
        self.builder.build_store(counter, next).map_err(|e| e.to_string())?;
        self.builder.build_unconditional_branch(cond).map_err(|e| e.to_string())?;

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
            if let Type::Record(fields) = elem {
                for col in 0..fields.len() {
                    let c = i64t.const_int(col as u64, false);
                    let got = self.call_rt(self.rt.rec_get, &[value, c.into()], "field")?;
                    self.builder
                        .build_call(self.rt.vec_set, &[vec.into(), c.into(), i.into(), got.into()], "")
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
        let elem_ty = source
            .ty
            .elem()
            .ok_or_else(|| "map over something that is not a Vec".to_string())?
            .clone();
        let out_elem = result
            .elem()
            .ok_or_else(|| "map did not produce a Vec".to_string())?
            .clone();

        let i64t = self.ctx.i64_type();
        let src = self.expr(source)?.into_pointer_value();
        let len = self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
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

    /// `select` builds a mask and then compacts, rather than growing an array.
    ///
    /// The predicate reads element `i` out of the column: nothing materialises an element, which
    /// is what keeps the loop in the shape that vectorises and is what the struct-of-arrays
    /// layout is for once a Vec of records has several columns.
    fn select(
        &mut self,
        source: &Tir,
        param: LocalId,
        pred: &Tir,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let elem_ty = source
            .ty
            .elem()
            .ok_or_else(|| "select on something that is not a Vec".to_string())?
            .clone();

        let src = self.expr(source)?.into_pointer_value();
        let len = self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
        let mask = self.call_rt(self.rt.mask_new, &[len.into()], "mask")?.into_pointer_value();
        let zero = self.ctx.i64_type().const_zero();

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

            let keep = e.expr(pred)?;
            let keep = e.to_slot(keep, &Type::Bool)?;
            e.builder
                .build_call(e.rt.mask_set, &[mask.into(), i.into(), keep.into()], "")
                .map_err(|err| err.to_string())?;
            Ok(())
        })?;

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

            Type::Vec(elem) if matches!(**elem, Type::Record(_)) => {
                let Type::Record(fields) = &**elem else { unreachable!("guarded") };
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
                let len =
                    self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
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
                        let got =
                            e.call_rt(e.rt.rec_get, &[record, c.into()], "sub")?;
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
            Type::Vec(elem) => {
                let inner_result = result
                    .elem()
                    .ok_or_else(|| "field access on a Vec did not yield a Vec".to_string())?
                    .clone();
                let elem_ty = (**elem).clone();
                let src = value.into_pointer_value();
                let len =
                    self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
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
        let len = self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
        let zero = i64t.const_zero();
        let parts = self
            .call_rt(self.rt.vec_new, &[len.into(), i64t.const_int(1, false).into()], "parts")?
            .into_pointer_value();

        let elem_ty = elem.clone();
        let gather = matches!(elem_ty, Type::Record(_));
        self.emit_loop(len, move |e, i| {
            let item = if gather {
                e.call_rt(e.rt.rec_from_vec, &[src.into(), i.into()], "elem")?
            } else {
                let slot =
                    e.call_rt(e.rt.vec_get, &[src.into(), zero.into(), i.into()], "slot")?.into_int_value();
                e.read_slot(slot, &elem_ty)?
            };
            let shown = e.show(item, &elem_ty)?;
            let shown = e.to_slot(shown, &Type::Str)?;
            e.builder
                .build_call(e.rt.vec_set, &[parts.into(), zero.into(), i.into(), shown.into()], "")
                .map_err(|err| err.to_string())?;
            Ok(())
        })?;

        let open = self.string_const(open);
        let sep = self.string_const(sep);
        let close = self.string_const(close);
        self.call_rt(self.rt.join, &[parts.into(), open.into(), sep.into(), close.into()], "joined")
    }

    fn show(&mut self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match ty {
            // The checker refuses a program whose result contains Lines, since there is
            // nothing to print: a stream has no value, only a promise that collect can redeem.
            Type::Lines => unreachable!("Lines cannot reach the printer"),
            Type::Str => self.call_rt(self.rt.quote, &[value], "quoted")?,
            Type::Int => self.call_rt(self.rt.int_to_str, &[value], "int_str")?,
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
            Type::Opt(inner) => {
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                let slot = self.builder.build_alloca(ptr_ty, "shown").map_err(|e| e.to_string())?;

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
                let raw = self.call_rt(self.rt.opt_get, &[value], "unwrapped")?.into_int_value();
                let item = self.read_slot(raw, inner)?;
                let shown = self.show(item, inner)?;
                self.builder.build_store(slot, shown).map_err(|e| e.to_string())?;
                self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;

                self.builder.position_at_end(none);
                let null = self.string_const("null");
                self.builder.build_store(slot, null).map_err(|e| e.to_string())?;
                self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;

                self.builder.position_at_end(done);
                self.builder
                    .build_load(ptr_ty, slot, "shown")
                    .map_err(|e| e.to_string())?
            }

            // The tag picks between the two JSON shapes (ADR 0009): a unit variant renders as
            // its quoted name, a payload variant as the single-key wrapper. A chain of branches
            // rather than a select, because rendering a payload allocates and loops; the last
            // variant needs no test, since the type says nothing else is left.
            Type::Enum { variants, .. } => {
                if variants.is_empty() {
                    return Err(unsupported("printing an enum with no variants"));
                }
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let i64t = self.ctx.i64_type();
                let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                let slot = self.builder.build_alloca(ptr_ty, "shown").map_err(|e| e.to_string())?;
                let tag = self
                    .call_rt(self.rt.rec_get, &[value, i64t.const_zero().into()], "tag")?
                    .into_int_value();
                let done = self.ctx.append_basic_block(function, "enum.done");

                for (i, (vname, payload)) in variants.iter().enumerate() {
                    let arm = self.ctx.append_basic_block(function, "enum.arm");
                    if i + 1 < variants.len() {
                        let next = self.ctx.append_basic_block(function, "enum.next");
                        let is = self
                            .builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                tag,
                                i64t.const_int(i as u64, false),
                                "is",
                            )
                            .map_err(|e| e.to_string())?;
                        self.builder
                            .build_conditional_branch(is, arm, next)
                            .map_err(|e| e.to_string())?;
                        self.builder.position_at_end(arm);
                        self.enum_arm(value, vname, payload, slot)?;
                        self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;
                        self.builder.position_at_end(next);
                    } else {
                        self.builder.build_unconditional_branch(arm).map_err(|e| e.to_string())?;
                        self.builder.position_at_end(arm);
                        self.enum_arm(value, vname, payload, slot)?;
                        self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;
                    }
                }

                self.builder.position_at_end(done);
                self.builder.build_load(ptr_ty, slot, "shown").map_err(|e| e.to_string())?
            }

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
                    .call_rt(self.rt.rec_get, &[value, i64t.const_int(1, false).into()], "payload")?
                    .into_int_value();
                let p = self.read_slot(raw, pty)?;
                let shown_p = self.show(p, pty)?;
                let key = self.string_const(&format!("{{\"{vname}\":"));
                let open = self.call_rt(self.rt.concat, &[key.into(), shown_p], "wrapped")?;
                let close = self.string_const("}");
                self.call_rt(self.rt.concat, &[open, close.into()], "wrapped")?
            }
        };
        self.builder.build_store(slot, shown).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn expr(&mut self, t: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match &t.kind {
            Kind::Str(text) => self.string_const(text).into(),
            Kind::Int(n) => self.ctx.i64_type().const_int(*n as u64, true).into(),

            Kind::Var(name) => *self
                .params
                .get(name)
                .ok_or_else(|| format!("`{name}` is not in scope in the native backend"))?,

            Kind::Local(id) => match self.locals.get(id) {
                Some(Slot::Value(v)) => *v,
                // The struct-of-arrays boundary. Nothing in the language asks for a whole
                // element out of a Vec, so this is unreachable until an indexing operator
                // exists; printing gathers through the runtime instead.
                Some(Slot::Cursor { .. }) => {
                    return Err(unsupported("using a Vec element as a whole value"))
                }
                None => return Err(format!("local {id} is not bound in the native backend")),
            },

            Kind::Bind { local, value, body } => {
                let value = self.expr(value)?;
                self.locals.insert(*local, Slot::Value(value));
                self.expr(body)?
            }

            Kind::Call { func, arg } => {
                let arg = self.expr(arg)?;
                let callee = *self
                    .funcs
                    .get(func)
                    .ok_or_else(|| format!("`{func}` was never declared"))?;
                self.builder
                    .build_call(callee, &[arg.into()], "call")
                    .map_err(|e| e.to_string())?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| "a toylang function returned nothing".to_string())?
            }

            Kind::Concat(l, r) => {
                let l = self.expr(l)?;
                let r = self.expr(r)?;
                self.call_rt(self.rt.concat, &[l, r], "concat")?
            }

            Kind::Compare { op, lhs, rhs } => self.compare(*op, lhs, rhs)?,

            // Fields are sorted, so field `i` here is field `i` of the type,
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
                            &[rec.into(), i64t.const_int(i as u64, false).into(), slot.into()],
                            "",
                        )
                        .map_err(|e| e.to_string())?;
                }
                rec.into()
            }

            // A two-slot box built with the record runtime: slot 0 the tag (the variant's
            // declaration index), slot 1 the payload, written only when one exists. Boxed
            // rather than immediate because an enum value has to fit the same 8-byte slot as
            // every other value while carrying two facts.
            Kind::EnumLit { variant, payload } => {
                let Type::Enum { variants, .. } = &t.ty else {
                    return Err("an EnumLit whose type is not an enum".to_string());
                };
                let tag = variants
                    .iter()
                    .position(|(n, _)| n == variant)
                    .ok_or_else(|| format!("`{variant}` is not a variant of {}", t.ty))?;
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
                rec.into()
            }

            Kind::VecLit(items) => {
                let elem = t
                    .ty
                    .elem()
                    .ok_or_else(|| "a Vec literal that is not a Vec".to_string())?
                    .clone();
                self.vec_lit(items, &elem)?
            }

            Kind::Select { source, param, pred } => self.select(source, *param, pred)?,

            Kind::Map { source, param, body } => self.map(source, *param, body, &t.ty)?,

            // No value to speak of: a promise that the real stdin has not been read yet, made
            // good only by collect. The constant is never actually inspected.
            Kind::Lines => self.ctx.i64_type().const_zero().into(),

            Kind::Input => {
                let descriptor = self.string_const(&Self::descriptor(&t.ty));
                let slot = self
                    .call_rt(self.rt.read_input, &[descriptor.into()], "input")?
                    .into_int_value();
                self.read_slot(slot, &t.ty)?
            }

            // Already a proper Vec pointer, unlike Input's raw slot: tl_read_inputs assembles
            // it itself, so there is nothing here for read_slot to unpack.
            Kind::Inputs => {
                let elem = t.ty.elem().expect("checked to be Vec<T>");
                let descriptor = self.string_const(&Self::descriptor(elem));
                self.call_rt(self.rt.read_inputs, &[descriptor.into()], "inputs")?
            }

            Kind::Field { base, name } => self.field(base, name, &t.ty)?,

            // The runtime returns the unwrapped slot as a pointer, so a scalar comes back
            // needing the integer put back; a pointer-shaped value is already right.
            // A real branch rather than a select, so only the taken side runs: either branch
            // may allocate, loop, or divide by zero.
            Kind::Cond { cond, then, otherwise } => {
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or("no function to branch in")?;
                let slot_ty = self.llvm_type(&t.ty)?;
                let slot =
                    self.builder.build_alloca(slot_ty, "cond").map_err(|e| e.to_string())?;

                let test = self.expr(cond)?.into_int_value();
                let yes = self.ctx.append_basic_block(function, "then");
                let no = self.ctx.append_basic_block(function, "else");
                let done = self.ctx.append_basic_block(function, "cond.done");
                self.builder
                    .build_conditional_branch(test, yes, no)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(yes);
                let v = self.expr(then)?;
                self.builder.build_store(slot, v).map_err(|e| e.to_string())?;
                self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;

                self.builder.position_at_end(no);
                let v = self.expr(otherwise)?;
                self.builder.build_store(slot, v).map_err(|e| e.to_string())?;
                self.builder.build_unconditional_branch(done).map_err(|e| e.to_string())?;

                self.builder.position_at_end(done);
                self.builder.build_load(slot_ty, slot, "cond").map_err(|e| e.to_string())?
            }

            Kind::Arith { op, lhs, rhs } => {
                let l = self.expr(lhs)?.into_int_value();
                let r = self.expr(rhs)?.into_int_value();
                self.arith(*op, l, r)?
            }

            Kind::Builtin { which, arg } => {
                let elem_ty = arg.ty.elem().cloned();
                let arg = self.expr(arg)?;
                match which {
                    Builtin::IntToStr => self.call_rt(self.rt.int_to_str, &[arg], "int_str")?,
                    Builtin::Range => self.call_rt(self.rt.range, &[arg], "range")?,
                    Builtin::JsonLines => {
                        let elem = elem_ty.expect("checked to be a Vec");
                        self.join_shown(arg, &elem, "", "\n", "")?
                    }
                    // `arg` is ignored: it is always Lines, directly or through a local bound
                    // to it, and there is only ever one real stdin, so nothing about the
                    // argument's value could change what this reads.
                    Builtin::Collect => self.call_rt(self.rt.collect_lines, &[], "lines")?,
                    // Already tracked on the Vec header; nothing to compute.
                    Builtin::Extent => self.call_rt(self.rt.vec_len, &[arg], "extent")?,
                    Builtin::Tail => self.call_rt(self.rt.vec_tail, &[arg], "tail")?,
                    Builtin::Concat => {
                        let elem = t.ty.elem().expect("checked to be Vec<Vec<T>> -> Vec<T>");
                        let ncols =
                            self.ctx.i64_type().const_int(Self::columns(elem), false);
                        self.call_rt(self.rt.vec_concat, &[arg, ncols.into()], "concat")?
                    }
                }
            }

            Kind::Unwrap { base } => {
                let depth = crate::tir::vec_depth(&base.ty);
                let inner = t.ty.clone();
                let base = self.expr(base)?;
                let raw = self
                    .call_rt(
                        self.rt.unwrap,
                        &[base, self.ctx.i64_type().const_int(depth as u64, false).into()],
                        "unwrapped",
                    )?
                    .into_pointer_value();
                if depth == 0 && matches!(inner, Type::Int | Type::Bool) {
                    let slot = self
                        .builder
                        .build_ptr_to_int(raw, self.ctx.i64_type(), "slot")
                        .map_err(|e| e.to_string())?;
                    self.read_slot(slot, &inner)?
                } else {
                    raw.into()
                }
            }

            Kind::Index { base, index, depth, elem_is_record } => {
                let i64t = self.ctx.i64_type();
                let base = self.expr(base)?;
                let index = self.expr(index)?;
                self.call_rt(
                    self.rt.at,
                    &[
                        base,
                        index,
                        i64t.const_int(*depth as u64, false).into(),
                        self.ctx.i32_type().const_int(*elem_is_record as u64, false).into(),
                    ],
                    "at",
                )?
            }
        })
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
            .build_int_compare(IntPredicate::EQ, divisor, self.ctx.i64_type().const_zero(), "zero")
            .map_err(|e| e.to_string())?;
        let fail = self.ctx.append_basic_block(function, "div.zero");
        let ok = self.ctx.append_basic_block(function, "div.ok");
        self.builder.build_conditional_branch(is_zero, fail, ok).map_err(|e| e.to_string())?;

        self.builder.position_at_end(fail);
        self.builder.build_call(self.rt.div_by_zero, &[], "").map_err(|e| e.to_string())?;
        self.builder.build_unreachable().map_err(|e| e.to_string())?;

        self.builder.position_at_end(ok);
        Ok(())
    }

    fn compare(&mut self, op: BinOp, lhs: &Tir, rhs: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        let operand_ty = lhs.ty.clone();
        let l = self.expr(lhs)?;
        let r = self.expr(rhs)?;

        let predicate = match op {
            BinOp::Eq => IntPredicate::EQ,
            BinOp::Ne => IntPredicate::NE,
            BinOp::Lt => IntPredicate::SLT,
            BinOp::Le => IntPredicate::SLE,
            BinOp::Gt => IntPredicate::SGT,
            BinOp::Ge => IntPredicate::SGE,
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
                    (call.into_int_value(), self.ctx.i64_type().const_int(1, false))
                }
                // tl_str_cmp returns -1, 0 or 1, so ordering is that against zero.
                _ => {
                    let call = self.call_rt(self.rt.str_cmp, &[l, r], "strcmp")?;
                    (call.into_int_value(), self.ctx.i64_type().const_zero())
                }
            },
            Type::Int => (l.into_int_value(), r.into_int_value()),
            Type::Bool => (l.into_int_value(), r.into_int_value()),
            other => return Err(unsupported(&format!("comparing {other}"))),
        };

        Ok(self
            .builder
            .build_int_compare(predicate, left, right, "cmp")
            .map_err(|e| e.to_string())?
            .into())
    }

    /// A `jsonlines(f(inputs))` program, compiled as a loop over `tl_read_one_input` instead of
    /// `self.expr(&program.body)` + one `print` at the end. `tl_print` already writes with a raw
    /// `write(1, ...)` syscall and needs no explicit flush, unlike every other backend that had
    /// to add one -- the one backend with no libc stdio buffering to fight.
    ///
    /// Each record is bound with `Slot::Value`, the same as `Kind::Input` binds a single read
    /// value, not `Slot::Cursor`: the struct-of-arrays optimisation `map`/`select` use for a
    /// cursor into an existing Vec's columns does not apply here, since there is no Vec -- each
    /// record arrives as its own one-off parsed value, exactly like `input` already is.
    fn fused_main(&mut self, program: &Program, fusion: &Fusion<'_>) -> Result<(), String> {
        let elem_ty = program
            .inputs
            .as_ref()
            .ok_or("fusion only matches an `inputs` source")?
            .clone();
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or("no function to emit a loop into")?;

        let i64t = self.ctx.i64_type();
        let i32t = self.ctx.i32_type();
        let descriptor = self.string_const(&Self::descriptor(&elem_ty));
        let out_slot = self.builder.build_alloca(i64t, "next_input").map_err(|e| e.to_string())?;

        let cond = self.ctx.append_basic_block(function, "fused.cond");
        let body = self.ctx.append_basic_block(function, "fused.body");
        let end = self.ctx.append_basic_block(function, "fused.end");

        self.builder.build_unconditional_branch(cond).map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond);
        let got = self
            .call_rt(self.rt.read_one_input, &[descriptor.into(), out_slot.into()], "got")?
            .into_int_value();
        let has_more = self
            .builder
            .build_int_compare(IntPredicate::NE, got, i32t.const_zero(), "has_more")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(has_more, body, end)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body);
        let slot =
            self.builder.build_load(i64t, out_slot, "slot").map_err(|e| e.to_string())?.into_int_value();
        let mut current = self.read_slot(slot, &elem_ty)?;
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
                    let keep_block = self.ctx.append_basic_block(function, &format!("fused.keep{i}"));
                    self.builder
                        .build_conditional_branch(keep, keep_block, cond)
                        .map_err(|e| e.to_string())?;
                    self.builder.position_at_end(keep_block);
                }
            }
        }

        self.print(current, &current_ty)?;
        self.builder.build_unconditional_branch(cond).map_err(|e| e.to_string())?;

        self.builder.position_at_end(end);
        Ok(())
    }

    /// A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
    fn print(&mut self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<(), String> {
        let as_str = match ty {
            Type::Str => value,
            other => self.show(value, other)?,
        };
        self.builder
            .build_call(self.rt.print, &[as_str.into()], "")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn build_module<'ctx>(ctx: &'ctx Context, program: &Program) -> Result<Module<'ctx>, String> {
    let mut e = Emitter::new(ctx);

    // Declared before any body, so a call to a function defined further down resolves. The
    // checker allows that, and it is where the Lua backend was wrong at prototype 1 step 3.
    for func in &program.funcs {
        e.declare(func)?;
    }
    for func in &program.funcs {
        e.define(func)?;
    }

    let i32t = ctx.i32_type();
    let main = e.module.add_function("main", i32t.fn_type(&[], false), None);
    e.builder.position_at_end(ctx.append_basic_block(main, "entry"));
    e.params.clear();
    e.locals.clear();

    if let Some(fusion) = tir::recognize_fusion(program) {
        e.fused_main(program, &fusion)?;
    } else {
        let body = e.expr(&program.body)?;
        e.print(body, &program.body.ty)?;
    }
    e.builder.build_return(Some(&i32t.const_zero())).map_err(|err| err.to_string())?;

    e.module.verify().map_err(|err| format!("LLVM rejected the module: {err}"))?;
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

    machine.write_to_file(&module, FileType::Object, object).map_err(|e| e.to_string())
}
