//! The Rust half of the native runtime links into a program and runs. No compiled program calls
//! into it yet, so this builds an object by hand: `main` returns `tl_rt_smoke(41)`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};

#[test]
fn rust_runtime_symbol_links_and_runs() {
    let dir = tempfile::tempdir().unwrap();
    let object = dir.path().join("smoke.o");

    let ctx = Context::create();
    let module = ctx.create_module("smoke");
    let builder = ctx.create_builder();
    let (i64t, i32t) = (ctx.i64_type(), ctx.i32_type());
    let smoke = module.add_function("tl_rt_smoke", i64t.fn_type(&[i64t.into()], false), None);
    let main = module.add_function("main", i32t.fn_type(&[], false), None);
    builder.position_at_end(ctx.append_basic_block(main, "entry"));
    let len = builder
        .build_call(smoke, &[i64t.const_int(41, false).into()], "len")
        .unwrap()
        .try_as_basic_value()
        .unwrap_basic()
        .into_int_value();
    let status = builder.build_int_truncate(len, i32t, "status").unwrap();
    builder.build_return(Some(&status)).unwrap();

    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap();
    machine
        .write_to_file(&module, FileType::Object, &object)
        .unwrap();

    let exe = dir.path().join("smoke");
    toylang::link_object(&object, &exe).unwrap();
    let code = std::process::Command::new(&exe).status().unwrap().code();
    // "41".len(), computed by the Rust runtime.
    assert_eq!(code, Some(2));
}
