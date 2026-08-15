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
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
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
}

struct Emitter<'ctx> {
    ctx: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    rt: Runtime<'ctx>,
    funcs: HashMap<String, FunctionValue<'ctx>>,
    locals: HashMap<LocalId, BasicValueEnum<'ctx>>,
    params: HashMap<String, BasicValueEnum<'ctx>>,
    next_global: usize,
}

impl<'ctx> Emitter<'ctx> {
    fn new(ctx: &'ctx Context) -> Emitter<'ctx> {
        let module = ctx.create_module("toylang");
        let ptr = ctx.ptr_type(AddressSpace::default());
        let i64t = ctx.i64_type();

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
            Type::Vec(_) => return Err(unsupported(&format!("a {ty} value"))),
            Type::Record(_) => return Err(unsupported(&format!("a {ty} value"))),
        })
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

    fn expr(&mut self, t: &Tir) -> Result<BasicValueEnum<'ctx>, String> {
        Ok(match &t.kind {
            Kind::Str(text) => self.string_const(text).into(),
            Kind::Int(n) => self.ctx.i64_type().const_int(*n as u64, true).into(),

            Kind::Var(name) => *self
                .params
                .get(name)
                .ok_or_else(|| format!("`{name}` is not in scope in the native backend"))?,

            Kind::Local(id) => *self
                .locals
                .get(id)
                .ok_or_else(|| format!("local {id} is not bound in the native backend"))?,

            Kind::Bind { local, value, body } => {
                let value = self.expr(value)?;
                self.locals.insert(*local, value);
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

            Kind::Input => return Err(unsupported("input")),
            Kind::VecLit(_) => return Err(unsupported("a Vec literal")),
            Kind::Select { .. } => return Err(unsupported("select")),
            Kind::Field { .. } => return Err(unsupported("field access")),
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

    /// The printer is built from the result type, as in the other two backends. Here there is
    /// no alternative: a native binary has no value to interrogate at runtime.
    fn print(&mut self, value: BasicValueEnum<'ctx>, ty: &Type) -> Result<(), String> {
        let as_str = match ty {
            Type::Str => value,
            Type::Int => self.call_rt(self.rt.int_to_str, &[value], "int_str")?,
            Type::Bool => {
                let t = self.string_const("true");
                let f = self.string_const("false");
                self.builder
                    .build_select(value.into_int_value(), t, f, "bool_str")
                    .map_err(|e| e.to_string())?
            }
            other => return Err(unsupported(&format!("printing a {other}"))),
        };
        self.builder
            .build_call(self.rt.print, &[as_str.into()], "")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn build_module<'ctx>(ctx: &'ctx Context, program: &Program) -> Result<Module<'ctx>, String> {
    if program.input.is_some() {
        return Err(unsupported("input"));
    }

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
