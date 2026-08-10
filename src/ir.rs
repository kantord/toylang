/// Lowered form. Composition starts lowering to loops at step 4, and emitting Lua straight off
/// the checked AST would have to be undone to get there.
#[derive(Debug)]
pub enum Ir {
    ConstStr(String),
    ConstInt(i64),
    Concat(Box<Ir>, Box<Ir>),
    Var(String),
    Call { func: String, arg: Box<Ir> },
}

#[derive(Debug)]
pub struct Func {
    pub name: String,
    pub param: String,
    pub body: Ir,
}

#[derive(Debug)]
pub struct Program {
    pub funcs: Vec<Func>,
    pub body: Ir,
}
