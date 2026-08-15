//! The native backend.
//!
//! Unlike Lua and JavaScript this does not end at a string of source. It ends at an object file,
//! which is not a program: linking is still someone else's job, so a native build shells out to
//! `cc`.
//!
//! Everything it cannot compile yet returns a named error rather than being silently absent, so
//! the gap between this and the other two backends is a visible, shrinking list.

use std::path::Path;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::{AddressSpace, OptimizationLevel};

use crate::tir::{Kind, Program};
use crate::ty::Type;

fn unsupported(what: &str) -> String {
    format!("the native backend cannot compile {what} yet")
}

/// A toylang `Str` is a pointer and a length, not a null-terminated `char*`. Its bytes can
/// contain a zero and its length is not `strlen`, so the C answer would have to be undone the
/// first time a string round-trips through input. Printing therefore goes through `write`
/// rather than `puts`.
fn build_module<'ctx>(ctx: &'ctx Context, program: &Program) -> Result<Module<'ctx>, String> {
    if !program.funcs.is_empty() {
        return Err(unsupported("functions"));
    }
    if program.input.is_some() {
        return Err(unsupported("input"));
    }
    if program.body.ty != Type::Str {
        return Err(unsupported(&format!("a {} result", program.body.ty)));
    }
    let Kind::Str(text) = &program.body.kind else {
        return Err(unsupported("any expression but a string literal"));
    };

    let module = ctx.create_module("toylang");
    let builder = ctx.create_builder();

    let i32t = ctx.i32_type();
    let i64t = ctx.i64_type();
    let ptrt = ctx.ptr_type(AddressSpace::default());

    // ssize_t write(int fd, const void *buf, size_t count)
    let write_ty = i64t.fn_type(&[i32t.into(), ptrt.into(), i64t.into()], false);
    let write_fn = module.add_function("write", write_ty, None);

    let main_fn = module.add_function("main", i32t.fn_type(&[], false), None);
    builder.position_at_end(ctx.append_basic_block(main_fn, "entry"));

    // The trailing newline is part of what gets printed, matching the other backends' print.
    let bytes = format!("{text}\n");
    let global = builder
        .build_global_string_ptr(&bytes, "out")
        .map_err(|e| e.to_string())?;

    write_bytes(&builder, write_fn, global.as_pointer_value(), bytes.len(), i32t, i64t)?;

    builder
        .build_return(Some(&i32t.const_int(0, false)))
        .map_err(|e| e.to_string())?;
    Ok(module)
}

fn write_bytes<'ctx>(
    builder: &Builder<'ctx>,
    write_fn: inkwell::values::FunctionValue<'ctx>,
    ptr: inkwell::values::PointerValue<'ctx>,
    len: usize,
    i32t: inkwell::types::IntType<'ctx>,
    i64t: inkwell::types::IntType<'ctx>,
) -> Result<(), String> {
    builder
        .build_call(
            write_fn,
            &[
                i32t.const_int(1, false).into(),
                ptr.into(),
                i64t.const_int(len as u64, false).into(),
            ],
            "",
        )
        .map_err(|e| e.to_string())?;
    Ok(())
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
