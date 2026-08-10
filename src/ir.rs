/// Lowered form. Every name is already a valid, collision-free Lua local by this point, so the
/// emitter does no scoping and no mangling.
#[derive(Debug)]
pub enum Ir {
    ConstStr(String),
    ConstInt(i64),
    VecLit(Vec<Ir>),
    Local(String),
    Call { func: String, arg: Box<Ir> },
    Concat(Box<Ir>, Box<Ir>),
    Compare { op: &'static str, lhs: Box<Ir>, rhs: Box<Ir> },
    /// `let name = value in body`, which is what a pipe becomes once `.` has a name.
    Bind { name: String, value: Box<Ir>, body: Box<Ir> },
    /// Keep the elements of `source` for which `pred` holds, with `param` bound to each.
    Select { source: Box<Ir>, param: String, pred: Box<Ir> },
    /// Read `name` off `base`, descending through `depth` Vec layers first. The depth comes
    /// from the checker, so the runtime never inspects a value to decide what to do.
    Field { base: Box<Ir>, name: String, depth: usize },
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
