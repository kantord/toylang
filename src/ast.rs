#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start, other.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }

    /// True for the operators that only ever mean arithmetic. `+` is missing because it also
    /// concatenates, which is the one place an operator's meaning depends on its operands.
    pub fn is_arithmetic(self) -> bool {
        matches!(self, BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem)
    }
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

/// The two Bool connectives. Kept out of `BinOp` because they share none of its rules: their
/// operands are Bool rather than a matched pair of anything, and they are the only operators
/// that may leave their right side unevaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

impl std::fmt::Display for LogicOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogicOp::And => "and",
            LogicOp::Or => "or",
        };
        write!(f, "{s}")
    }
}

/// A type as written in source, before it is resolved to a `Type`.
#[derive(Debug)]
pub enum TypeExpr {
    Named {
        name: String,
        /// `Pair<Int>`: the arguments a declared generic enum is applied to. Empty for a
        /// plain name. `Vec`/`Stream` keep their own variants below.
        args: Vec<TypeExpr>,
        span: Span,
    },
    Vec {
        elem: Box<TypeExpr>,
        span: Span,
    },
    /// `Stream<T>`, legal only as the whole of a parameter or return annotation: a stream is
    /// not a value, so the checker refuses it anywhere a type would describe something stored.
    Stream {
        elem: Box<TypeExpr>,
        span: Span,
    },
    Record {
        fields: Vec<(String, TypeExpr)>,
        span: Span,
    },
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named { span, .. }
            | TypeExpr::Vec { span, .. }
            | TypeExpr::Stream { span, .. }
            | TypeExpr::Record { span, .. } => *span,
        }
    }
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug)]
pub struct Def {
    pub name: String,
    /// `None` for a nullary function (`fn name() -> T = body`).
    pub param: Option<Param>,
    pub ret: TypeExpr,
    pub body: Expr,
    pub span: Span,
    /// Whether a module's prelude includes this definition when compiling a program. Meaningless
    /// outside a module today, since nothing yet imports from a program file.
    pub is_pub: bool,
}

/// `enum Shape { point, circle{r: Int} }`. The first declaration that creates a type identity
/// rather than abbreviating one: the name is what exhaustiveness will be proved against.
#[derive(Debug)]
pub struct EnumDecl {
    pub name: String,
    /// Type parameters, in declaration order: `enum Opt<T> { ... }`. Capitalized names,
    /// bound only inside this declaration's payloads.
    pub params: Vec<(String, Span)>,
    pub variants: Vec<Variant>,
    pub span: Span,
    /// Same meaning as `Def::is_pub`: whether a module exports this declaration.
    pub is_pub: bool,
}

/// One alternative of an enum. Variant names are data (they appear as JSON keys and strings),
/// so they are exempt from the capital-means-type casing rule, like record fields.
#[derive(Debug)]
pub struct Variant {
    pub name: String,
    pub span: Span,
    /// `None` for a unit variant. Any single type, spelled the way a call spells its argument:
    /// a record type directly in braces (`circle{r: Int}`), any type in parens (`celsius(Int)`).
    pub payload: Option<TypeExpr>,
}

/// Zero or more definitions followed by the expression that is the program.
#[derive(Debug)]
pub struct File {
    /// `type Db = {users: Vec<User>}`. An abbreviation and nothing more: the name and what it
    /// stands for are one type, so nothing distinguishes them once resolved.
    pub aliases: Vec<Alias>,
    pub enums: Vec<EnumDecl>,
    pub defs: Vec<Def>,
    /// `input <type>`: a declaration of what stdin holds, written after the definitions and
    /// before the body, the way a signature types a parameter. `None` when the program leaves
    /// the input untyped, in which case the first use of `input` in the body types it.
    pub input: Option<TypeExpr>,
    pub body: Expr,
}

/// What a module file holds: declarations only, no trailing expression.
#[derive(Debug)]
pub struct Module {
    pub defs: Vec<Def>,
    pub enums: Vec<EnumDecl>,
}

#[derive(Debug)]
pub struct Alias {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug)]
pub enum Expr {
    Str {
        text: String,
        span: Span,
    },
    Int {
        value: i64,
        span: Span,
    },
    VecLit {
        items: Vec<Expr>,
        span: Span,
    },
    /// `{name: expr, age: expr}`. A record literal, the inverse of a projection. Each field
    /// keeps its own span so a repeated one can be pointed at rather than described.
    RecordLit {
        fields: Vec<(String, Span, Expr)>,
        span: Span,
    },
    /// `.`, the value the enclosing pipeline or filter is currently working on.
    Subject {
        span: Span,
    },
    Var {
        name: String,
        span: Span,
    },
    Call {
        func: String,
        func_span: Span,
        /// `None` for a nullary call (`name()`).
        arg: Option<Box<Expr>>,
        span: Span,
    },
    /// `base[]`. A spec that keeps a dimension.
    Project {
        base: Box<Expr>,
        span: Span,
    },
    /// `base[i]`. A spec that collapses a dimension, so the entry may not be there.
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// `base[lo:hi]`, bounds optional. A spec that narrows a dimension by position, jq's
    /// `.[2:5]`; out-of-range bounds clamp to the valid range rather than answering `Opt` the
    /// way a collapsing index does (kantord/toylang#143).
    Slice {
        base: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        span: Span,
    },
    /// `base!`. Insist the value is there, and stop the program if it is not.
    Unwrap {
        base: Box<Expr>,
        span: Span,
    },
    /// `-base`.
    Neg {
        base: Box<Expr>,
        span: Span,
    },
    /// `not base`. Looser than every comparison, so `not a == b` negates the comparison, and
    /// tighter than `and`/`or`, so `not a and b` negates only `a`.
    Not {
        base: Box<Expr>,
        span: Span,
    },
    /// `then if cond else otherwise`. An expression, in a language that has only those.
    Cond {
        then: Box<Expr>,
        cond: Box<Expr>,
        otherwise: Box<Expr>,
        span: Span,
    },
    /// `base.name`. Distributes over a Vec rather than needing a map.
    Field {
        base: Box<Expr>,
        name: String,
        span: Span,
    },
    /// The value read from stdin. It has no type of its own and can only be checked against an
    /// expected one, which is the same rule the draft gives for lambdas.
    Input {
        span: Span,
    },
    /// Every remaining JSON value on stdin, one per line, collected eagerly into a `Vec<T>`.
    /// Like `input`, its element type comes only from where it is used.
    Inputs {
        span: Span,
    },
    /// The stream of lines read from stdin, born `Stream<Str>`. The checker rejects a second
    /// use rather than accepting a second stream, since there is only ever one real stdin.
    Lines {
        span: Span,
    },
    /// An `or` chain of match arms over the subject `.`: `point -> 0 or circle{r} -> r * r`.
    /// The subject is not part of the node; a match reads `.` the way `select` does, so it
    /// appears as a pipe stage.
    Match {
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `Shape.circle` or `Shape.circle{r: 1}`: a variant constructor spelled through its enum.
    /// Only the qualified form needs its own node; a bare `circle{r: 1}` is an ordinary `Call`
    /// and a bare `active` an ordinary `Var`, resolved through the enum registry by the checker.
    Variant {
        enum_name: String,
        enum_span: Span,
        variant: String,
        variant_span: Span,
        payload: Option<Box<Expr>>,
        span: Span,
    },
    /// `lhs | rhs`, which binds `.` in `rhs` to the value of `lhs`.
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `lhs and rhs`, `lhs or rhs`. A separate node from `Binary` for the same reason `LogicOp`
    /// is a separate enum: nothing about how these are typed or evaluated follows `Binary`'s
    /// rules.
    Logic {
        op: LogicOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `let <name> = <expr>` on its own line, repeated, then the expression that ends the block
    /// -- the local-binding form the ruling on #87 picked over `let ... in`: no keyword to pair,
    /// just a sequence of bindings followed by a result. Only reachable as a function body.
    Let {
        bindings: Vec<(String, Expr)>,
        body: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug)]
pub enum Pattern {
    /// A variant name, optionally destructuring its payload's fields. Bare names inside the
    /// braces bind fresh, and `rest` is the `..` marker that permits naming only some of them.
    Variant {
        name: String,
        span: Span,
        fields: Option<FieldsPattern>,
    },
    /// A Bool guard: the arm matches when the expression is true. `.` inside it (and inside
    /// the arm's body) is still the chain's subject, unlike a variant arm, which rebinds it.
    Guard(Expr),
    /// The default arm: `any()`, or the bare trailing expression the parser desugars into one
    /// (its body doubles as its span). Matches whatever is left.
    Default { span: Span },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Variant { span, .. } | Pattern::Default { span } => *span,
            Pattern::Guard(e) => e.span(),
        }
    }
}

#[derive(Debug)]
pub struct FieldsPattern {
    pub names: Vec<(String, Span)>,
    pub rest: bool,
    pub span: Span,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. }
            | Expr::Int { span, .. }
            | Expr::VecLit { span, .. }
            | Expr::Subject { span }
            | Expr::Var { span, .. }
            | Expr::Call { span, .. }
            | Expr::Project { span, .. }
            | Expr::Index { span, .. }
            | Expr::Slice { span, .. }
            | Expr::Unwrap { span, .. }
            | Expr::RecordLit { span, .. }
            | Expr::Neg { span, .. }
            | Expr::Not { span, .. }
            | Expr::Cond { span, .. }
            | Expr::Field { span, .. }
            | Expr::Input { span }
            | Expr::Inputs { span }
            | Expr::Lines { span }
            | Expr::Variant { span, .. }
            | Expr::Match { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Logic { span, .. }
            | Expr::Let { span, .. } => *span,
        }
    }
}
