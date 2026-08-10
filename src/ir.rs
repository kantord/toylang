/// Lowered form. One node today, but composition starts lowering to loops at step 4 and
/// emitting Lua straight off the checked AST would have to be undone to get there.
#[derive(Debug)]
pub enum Ir {
    ConstStr(String),
}

#[derive(Debug)]
pub struct Program {
    pub body: Ir,
}
