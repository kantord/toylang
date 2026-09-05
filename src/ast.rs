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

/// What a parameter's left side binds: either one name or a record destructured the way a match
/// arm's brace pattern destructures one. The type annotation stays fully explicit either way.
#[derive(Debug)]
pub enum ParamShape {
    /// `name` in `fn f(name: T) -> R`.
    Name(String, Span),
    /// `{a, b, ..}` in `fn f({a, b}: T) -> R`, binding each named field of the record `T`.
    Fields(FieldsPattern),
}

impl ParamShape {
    pub fn span(&self) -> Span {
        match self {
            ParamShape::Name(_, span) => *span,
            ParamShape::Fields(f) => f.span,
        }
    }
}

#[derive(Debug)]
pub struct Param {
    pub shape: ParamShape,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Which source file a definition came from. Only two exist today: the program's own file and
/// the single prelude module. A non-`pub` definition is callable only from its own file, so
/// this is what the checker keys visibility on at each call site (gh:166).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The program's own file.
    Program,
    /// The prelude module, `prelude.toy`.
    Prelude,
}

#[derive(Debug)]
pub struct Def {
    pub name: String,
    /// `None` for a nullary function (`fn name() -> T = body`).
    pub param: Option<Param>,
    /// `None` for a hoisted definition (`fn name = body`, gh:152): no return type is written, so
    /// the signature -- parameter and return both -- is inferred from the body. A hoisted
    /// function's parameter is the implicit `.` the body matches against, and its type comes from
    /// what the body matches; the return type is what the body synthesises.
    pub ret: Option<TypeExpr>,
    pub body: Expr,
    pub span: Span,
    /// Whether a module's prelude includes this definition when compiling a program. Meaningless
    /// outside a module today, since nothing yet imports from a program file.
    pub is_pub: bool,
    /// The file this definition was written in. A program's own definitions get `Program` from
    /// the parser; the prelude's get `Prelude` from `prelude::module`.
    pub origin: Origin,
    /// `fn name = body` (gh:152): no parameter list, no return annotation. `param` and `ret` are
    /// both `None`, and the checker infers the signature from the body. The first slice accepts
    /// only a match-call body (`Msg(Ping -> ...)`), which fixes the parameter type to the named
    /// enum and the return type to the arms' common type.
    pub hoisted: bool,
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

/// `trait Name { fn sig(param: Type) -> Type }`:a named collection of method signatures,
/// with no bodies. The receiver type is spelled `Self` in a signature, and binds to whatever
/// concrete type an `impl` block targets. Parsed only for now: checking and dispatch are later
/// slices.
#[derive(Debug)]
pub struct TraitDecl {
    pub name: String,
    /// The method signatures, in declaration order.
    pub methods: Vec<TraitMethodSig>,
    pub span: Span,
    /// Same meaning as `Def::is_pub`: whether a module exports this declaration.
    pub is_pub: bool,
}

/// One method signature of a trait declaration:the same `fn name(param: Type) -> Type`
/// spine a function uses, minus the body an `impl` provides.
#[derive(Debug)]
pub struct TraitMethodSig {
    pub name: String,
    /// `None` for a nullary method (`fn name() -> T`).
    pub param: Option<Param>,
    pub ret: TypeExpr,
    pub span: Span,
}

/// `impl Trait for Type { fn sig(param: Type) -> Type = body }`: concrete bodies for one
/// trait's methods, one block per (trait, type) pair. Parsed only for now:the checker does
/// not use this yet.
#[derive(Debug)]
pub struct ImplDecl {
    pub trait_name: String,
    /// The concrete type the trait's `Self` substitutes to.
    pub ty: TypeExpr,
    /// The method bodies, each against the trait's signature of the same name.
    pub methods: Vec<ImplMethod>,
    pub span: Span,
}
/// One method of an impl block:the same spine a trait signature has, plus the body that makes
/// it a definition.

#[derive(Debug)]
pub struct ImplMethod {
    pub name: String,
    pub param: Option<Param>,
    pub ret: TypeExpr,
    pub body: Expr,
    pub span: Span,
}

/// Zero or more definitions followed by the expression that is the program.
#[derive(Debug)]
pub struct File {
    /// `type Db = {users: Vec<User>}`. An abbreviation and nothing more: the name and what it
    /// stands for are one type, so nothing distinguishes them once resolved.
    pub aliases: Vec<Alias>,
    pub enums: Vec<EnumDecl>,
    /// `trait Name { ... }`:a named collection of method signatures. Parsed only for now.
    pub traits: Vec<TraitDecl>,
    /// `impl Trait for Type { ... }`: concrete bodies for one trait's methods. Parsed only for now.
    pub impls: Vec<ImplDecl>,
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
    /// A decimal-point literal (ADR 0007): an IEEE 754 binary64 double, the same
    /// representation every JavaScript engine carries. The dot is what makes a number a Float,
    /// and there is no alternative width or decimal type to guess at.
    Float {
        value: f64,
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
    /// Stdin read as raw lines, each split on a delimiter, born `Vec<Vec<Str>>`: the
    /// parameterized DSV source. `csv` and `tsv` are the same node with the delimiter fixed.
    Dsv {
        delim: String,
        span: Span,
    },
    /// An `or` chain of match arms over the subject `.`: `point -> 0 or circle{r} -> r * r`.
    /// The subject is not part of the node; a match reads `.` the way `select` does, so it
    /// appears as a pipe stage.
    Match {
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `Msg(Ping -> "pong", Quit -> "bye")`: a type name used as a match call (gh:152). The
    /// parens hold comma-separated arms over the subject `.`, the same arms a `Match` carries;
    /// the enum name is the assertion that the subject is one of this enum's values, and the
    /// checker resolves each variant against it. Sugar for a `Match` whose subject is `.`, kept
    /// as its own node so the surface spelling survives formatting.
    MatchCall {
        enum_name: String,
        enum_span: Span,
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
    /// `lhs |> callee`, the tail-pipeline marker: the one way a sink call is written, `callee`
    /// applied to `lhs`. Only a Sink-typed callee is legal here, and the production is parsed
    /// only at a program's outermost position, so this node never appears nested.
    TailPipe {
        lhs: Box<Expr>,
        callee: String,
        callee_span: Span,
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
            | Expr::Float { span, .. }
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
            | Expr::Field { span, .. }
            | Expr::Input { span }
            | Expr::Inputs { span }
            | Expr::Lines { span }
            | Expr::Dsv { span, .. }
            | Expr::Variant { span, .. }
            | Expr::Match { span, .. }
            | Expr::MatchCall { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::TailPipe { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Logic { span, .. }
            | Expr::Let { span, .. } => *span,
        }
    }
}
