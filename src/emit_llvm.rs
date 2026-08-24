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
use crate::tir::{Func, Kind, LocalId, Program, Tir};
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
    read_input: FunctionValue<'ctx>,
    rec_from_vec: FunctionValue<'ctx>,
    at: FunctionValue<'ctx>,
    opt_is_some: FunctionValue<'ctx>,
    opt_get: FunctionValue<'ctx>,
    unwrap: FunctionValue<'ctx>,
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
            read_input: module.add_function(
                "tl_read_input",
                i64t.fn_type(&[ptr.into()], false),
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
            Type::Str => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Int => self.ctx.i64_type().into(),
            Type::Bool => self.ctx.bool_type().into(),
            Type::Vec(_) | Type::Opt(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
            Type::Record(_) => self.ctx.ptr_type(AddressSpace::default()).into(),
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
            Type::Str => "s".to_string(),
            Type::Int => "i".to_string(),
            Type::Bool => "b".to_string(),
            Type::Vec(elem) => format!("[{}", Self::descriptor(elem)),
            // Opt has no spelling in the type syntax, so an input type can never contain one.
            Type::Opt(_) => unreachable!("Opt cannot be declared, so input never has one"),
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
            Type::Int => value.into_int_value(),
            Type::Bool => self
                .builder
                .build_int_z_extend(value.into_int_value(), i64t, "slot")
                .map_err(|e| e.to_string())?,
            Type::Str | Type::Vec(_) | Type::Opt(_) | Type::Record(_) => self
                .builder
                .build_ptr_to_int(value.into_pointer_value(), i64t, "slot")
                .map_err(|e| e.to_string())?,
        })
    }

    fn read_slot(
        &self,
        slot: IntValue<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        Ok(match ty {
            Type::Int => slot.into(),
            Type::Bool => self
                .builder
                .build_int_truncate(slot, self.ctx.bool_type(), "elem")
                .map_err(|e| e.to_string())?
                .into(),
            Type::Str | Type::Vec(_) | Type::Opt(_) | Type::Record(_) => self
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
            let slot = self.to_slot(value, elem)?;
            self.builder
                .build_call(
                    self.rt.vec_set,
                    &[
                        vec.into(),
                        i64t.const_zero().into(),
                        i64t.const_int(index as u64, false).into(),
                        slot.into(),
                    ],
                    "",
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(vec.into())
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
                self.call_rt(
                    self.rt.vec_column,
                    &[value, i64t.const_int(index as u64, false).into()],
                    "column",
                )
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

    fn show(&mut self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match ty {
            Type::Str => self.call_rt(self.rt.quote, &[value], "quoted")?,
            Type::Int => self.call_rt(self.rt.int_to_str, &[value], "int_str")?,
            Type::Bool => {
                let t = self.string_const("true");
                let f = self.string_const("false");
                self.builder
                    .build_select(value.into_int_value(), t, f, "bool_str")
                    .map_err(|e| e.to_string())?
            }
            Type::Vec(elem) => {
                let i64t = self.ctx.i64_type();
                let src = value.into_pointer_value();
                let len = self.call_rt(self.rt.vec_len, &[src.into()], "len")?.into_int_value();
                let zero = i64t.const_zero();
                let parts = self
                    .call_rt(
                        self.rt.vec_new,
                        &[len.into(), i64t.const_int(1, false).into()],
                        "parts",
                    )?
                    .into_pointer_value();

                let elem_ty = (**elem).clone();
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
                        .build_call(e.rt.vec_set, &[parts.into(), zero.into(), i.into(), shown.into()], "")
                        .map_err(|err| err.to_string())?;
                    Ok(())
                })?;

                let open = self.string_const("[");
                let sep = self.string_const(",");
                let close = self.string_const("]");
                self.call_rt(
                    self.rt.join,
                    &[parts.into(), open.into(), sep.into(), close.into()],
                    "joined",
                )?
            }
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

            Kind::VecLit(items) => {
                let elem = t
                    .ty
                    .elem()
                    .ok_or_else(|| "a Vec literal that is not a Vec".to_string())?
                    .clone();
                self.vec_lit(items, &elem)?
            }

            Kind::Select { source, param, pred } => self.select(source, *param, pred)?,

            Kind::Input => {
                let descriptor = self.string_const(&Self::descriptor(&t.ty));
                let slot = self
                    .call_rt(self.rt.read_input, &[descriptor.into()], "input")?
                    .into_int_value();
                self.read_slot(slot, &t.ty)?
            }

            Kind::Field { base, name } => self.field(base, name, &t.ty)?,

            // The runtime returns the unwrapped slot as a pointer, so a scalar comes back
            // needing the integer put back; a pointer-shaped value is already right.
            Kind::IntToStr(n) => {
                let n = self.expr(n)?;
                self.call_rt(self.rt.int_to_str, &[n], "int_str")?
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
            BinOp::Add => return Err("Add is not a comparison".to_string()),
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

    let body = e.expr(&program.body)?;
    e.print(body, &program.body.ty)?;
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
