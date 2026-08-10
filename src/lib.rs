pub mod ast;
pub mod check;
pub mod emit_lua;
pub mod error;
pub mod input;
pub mod lex;
pub mod parse;
pub mod tir;
pub mod ty;

use std::cell::RefCell;
use std::rc::Rc;

use error::Error;
use ty::Type;

pub struct Compiled {
    pub lua: String,
    pub ty: Type,
    /// The type stdin must have, if the program reads it.
    pub input: Option<Type>,
}

pub fn compile(src: &str) -> Result<Compiled, Error> {
    let tokens = lex::lex(src)?;
    let file = parse::parse(&tokens)?;
    let program = check::check(&file)?;
    let lua = emit_lua::emit(&program);
    Ok(Compiled { lua, ty: program.body.ty.clone(), input: program.input })
}

pub fn run(src: &str) -> Result<String, Box<dyn std::error::Error>> {
    run_with_input(src, None)
}

/// Compile and run, capturing what the program printed.
///
/// Capturing rather than streaming keeps the tests to one function call. It also means output
/// is held in memory, which is fine while every program has statically known extent and will
/// not be once streaming input exists.
pub fn run_with_input(
    src: &str,
    stdin: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let compiled = compile(src)?;
    let lua = mlua::Lua::new();

    match (&compiled.input, stdin) {
        (Some(ty), Some(text)) => {
            let value: serde_json::Value = serde_json::from_str(text)?;
            input::validate(&value, ty, "input")?;
            lua.globals().set(emit_lua::INPUT, input::to_lua(&lua, &value)?)?;
        }
        (Some(ty), None) => return Err(format!("this program reads input, of type {ty}").into()),
        (None, _) => {}
    }

    let captured = Rc::new(RefCell::new(String::new()));
    let sink = Rc::clone(&captured);
    let print = lua.create_function(move |_, s: String| {
        sink.borrow_mut().push_str(&s);
        sink.borrow_mut().push('\n');
        Ok(())
    })?;
    lua.globals().set("print", print)?;

    lua.load(&compiled.lua).exec()?;

    let out = captured.borrow().clone();
    Ok(out)
}

impl std::error::Error for Error {}
