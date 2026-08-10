/// Lowered form. Composition starts lowering to loops at step 4, and emitting Lua straight off
/// the checked AST would have to be undone to get there.
#[derive(Debug)]
pub enum Ir {
    ConstStr(String),
    Concat(Box<Ir>, Box<Ir>),
}

#[derive(Debug)]
pub struct Program {
    pub body: Ir,
}
