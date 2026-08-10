pub mod ast;
pub mod check;
pub mod emit_lua;
pub mod error;
pub mod ir;
pub mod lex;
pub mod lower;
pub mod parse;
pub mod ty;

use std::cell::RefCell;
use std::rc::Rc;

use error::Error;
use ty::Type;

/// Source to Lua source. Returns the program's type alongside it so the CLI can report what
/// it compiled without running it.
pub fn compile(src: &str) -> Result<(String, Type), Error> {
    let tokens = lex::lex(src)?;
    let expr = parse::parse(&tokens)?;
    let ty = check::check(&expr)?;
    let program = lower::lower(&expr);
    Ok((emit_lua::emit(&program), ty))
}

/// Compile and run, capturing what the program printed.
///
/// Capturing rather than streaming keeps the tests to one function call. It also means output
/// is held in memory, which is fine while every program has statically known extent and will
/// not be once streaming input exists.
pub fn run(src: &str) -> Result<String, Box<dyn std::error::Error>> {
    let (lua_src, _) = compile(src)?;
    let lua = mlua::Lua::new();

    let captured = Rc::new(RefCell::new(String::new()));
    let sink = Rc::clone(&captured);
    let print = lua.create_function(move |_, s: String| {
        sink.borrow_mut().push_str(&s);
        sink.borrow_mut().push('\n');
        Ok(())
    })?;
    lua.globals().set("print", print)?;

    lua.load(&lua_src).exec()?;

    let out = captured.borrow().clone();
    Ok(out)
}

impl std::error::Error for Error {}
