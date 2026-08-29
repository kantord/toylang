use winnow::error::ParserError;
use winnow::prelude::*;
use winnow::stream::{LocatingSlice, Location, Stream};
use winnow::token::take_while;

use crate::ast::{
    Alias, BinOp, Def, EnumDecl, Expr, FieldsPattern, File, MatchArm, Module, Param, Pattern, Span,
    TypeExpr, Variant,
};
use crate::error::Error;
use crate::ty;

/// The stream this whole module parses over: a source string with byte-offset tracking built
/// in, so every sub-parser gets `Span`s for free from `current_token_start` rather than threading
/// position by hand.
type Input<'i> = LocatingSlice<&'i str>;

/// Lets every winnow combinator used below (`take_while`, and anything built on it) report
/// failure as this crate's own `Error` directly -- there is no separate winnow error type to
/// translate out of. `from_input` is winnow's fallback for a failure it synthesises itself
/// rather than one this module constructs explicitly; nothing here is expected to hit it; the
/// hand-written parsers below always build their own `Error` with a specific message instead.
impl<'i> ParserError<Input<'i>> for Error {
    type Inner = Error;

    fn from_input(input: &Input<'i>) -> Error {
        let at = input.current_token_start();
        Error::new(Span::new(at, at), "expected a token".to_string())
    }

    fn into_inner(self) -> Result<Error, Error> {
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Int(i64),
    Ident(String),
    Fn,
    Pub,
    Type,
    Enum,
    If,
    Else,
    Input,
    Inputs,
    Lines,
    Or,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pipe,
    Comma,
    Dot,
    Eq,
    EqEq,
    Ne,
    Bang,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Arrow,
    Eof,
}

impl std::fmt::Display for Tok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Tok::Str(_) => return write!(f, "a string"),
            Tok::Int(n) => return write!(f, "`{n}`"),
            Tok::Ident(name) => return write!(f, "`{name}`"),
            Tok::Fn => "`fn`",
            Tok::Pub => "`pub`",
            Tok::Type => "`type`",
            Tok::Enum => "`enum`",
            Tok::If => "`if`",
            Tok::Else => "`else`",
            Tok::Input => "`input`",
            Tok::Inputs => "`inputs`",
            Tok::Lines => "`lines`",
            Tok::Or => "`or`",
            Tok::Plus => "`+`",
            Tok::Minus => "`-`",
            Tok::Star => "`*`",
            Tok::Slash => "`/`",
            Tok::Percent => "`%`",
            Tok::Pipe => "`|`",
            Tok::Comma => "`,`",
            Tok::Dot => "`.`",
            Tok::Eq => "`=`",
            Tok::EqEq => "`==`",
            Tok::Ne => "`!=`",
            Tok::Bang => "`!`",
            Tok::Lt => "`<`",
            Tok::Le => "`<=`",
            Tok::Gt => "`>`",
            Tok::Ge => "`>=`",
            Tok::LParen => "`(`",
            Tok::RParen => "`)`",
            Tok::LBracket => "`[`",
            Tok::RBracket => "`]`",
            Tok::LBrace => "`{`",
            Tok::RBrace => "`}`",
            Tok::Colon => "`:`",
            Tok::Arrow => "`->`",
            Tok::Eof => "end of program",
        };
        write!(f, "{s}")
    }
}

/// Zero or more ASCII spaces/tabs/newlines, then a `#`-to-newline comment if one follows,
/// repeated: a run of trivia can mix any number of either in any order.
fn skip_trivia(input: &mut Input) {
    loop {
        take_while::<_, _, Error>(0.., |c: char| c.is_ascii_whitespace())
            .parse_next(input)
            .expect("a lower bound of zero never fails");
        if input.peek_token() != Some('#') {
            return;
        }
        input.next_token();
        while !matches!(input.peek_token(), None | Some('\n')) {
            input.next_token();
        }
    }
}

/// Skips trivia, then reads exactly one token from the front of `input`. Called fresh for every
/// `peek`/`peek2`/`advance` rather than once up front into a `Vec`, so a lexical error (a bad
/// escape, an out-of-range integer) surfaces at the point parsing actually reaches it instead of
/// always winning over a parse error earlier in the file the way a separate up-front pass would.
fn read_tok<'i>(input: &mut Input<'i>) -> Result<(Tok, Span), Error> {
    skip_trivia(input);
    let start = input.current_token_start();
    let Some(c) = input.peek_token() else {
        return Ok((Tok::Eof, Span::new(start, start)));
    };

    let tok = match c {
        '"' => Tok::Str(read_string(input)?),
        '0'..='9' => {
            let digits = take_while::<_, _, Error>(1.., |c: char| c.is_ascii_digit())
                .parse_next(input)
                .expect("a digit was just confirmed to follow");
            let n = digits.parse::<i64>().map_err(|_| {
                Error::new(
                    Span::new(start, input.current_token_start()),
                    format!("integer `{digits}` is out of range"),
                )
            })?;
            Tok::Int(n)
        }
        c if c.is_ascii_alphabetic() || c == '_' => {
            let word =
                take_while::<_, _, Error>(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                    .parse_next(input)
                    .expect("a letter or underscore was just confirmed to follow");
            match word {
                "fn" => Tok::Fn,
                "pub" => Tok::Pub,
                "type" => Tok::Type,
                "enum" => Tok::Enum,
                "if" => Tok::If,
                "else" => Tok::Else,
                "input" => Tok::Input,
                "inputs" => Tok::Inputs,
                "lines" => Tok::Lines,
                // Reserved for arm composition now rather than discovered later: there is no
                // Bool `or`, and if one ever lands, arm-`or` binds loosest (draft.md, the
                // match-arms decision).
                "or" => Tok::Or,
                name => Tok::Ident(name.to_string()),
            }
        }
        // `-` is subtraction and negation now, so a digit after it is no longer a negative
        // literal: `a -1` would otherwise not be `a - 1`.
        '-' => {
            input.next_token();
            if input.peek_token() == Some('>') {
                input.next_token();
                Tok::Arrow
            } else {
                Tok::Minus
            }
        }
        '!' => {
            input.next_token();
            if input.peek_token() == Some('=') {
                input.next_token();
                Tok::Ne
            } else {
                Tok::Bang
            }
        }
        '=' | '<' | '>' => {
            input.next_token();
            if input.peek_token() == Some('=') {
                input.next_token();
                match c {
                    '=' => Tok::EqEq,
                    '<' => Tok::Le,
                    _ => Tok::Ge,
                }
            } else {
                match c {
                    '=' => Tok::Eq,
                    '<' => Tok::Lt,
                    _ => Tok::Gt,
                }
            }
        }
        '+' => single(input, Tok::Plus),
        '*' => single(input, Tok::Star),
        // `//` is not a token anymore: match arms compose with `or` now, so a second slash is
        // just a `/` where no expression can start, which is the parse error migration needs.
        '/' => single(input, Tok::Slash),
        '%' => single(input, Tok::Percent),
        '|' => single(input, Tok::Pipe),
        ',' => single(input, Tok::Comma),
        '.' => single(input, Tok::Dot),
        '(' => single(input, Tok::LParen),
        ')' => single(input, Tok::RParen),
        '[' => single(input, Tok::LBracket),
        ']' => single(input, Tok::RBracket),
        '{' => single(input, Tok::LBrace),
        '}' => single(input, Tok::RBrace),
        ':' => single(input, Tok::Colon),
        other => {
            return Err(Error::new(
                Span::new(start, start + 1),
                format!("unexpected character `{other}`"),
            ));
        }
    };
    Ok((tok, Span::new(start, input.current_token_start())))
}

fn single(input: &mut Input, tok: Tok) -> Tok {
    input.next_token();
    tok
}

/// Returns the unescaped contents of a string literal, having consumed through the closing
/// quote. `input` is positioned on the opening `"` on entry.
fn read_string(input: &mut Input) -> Result<String, Error> {
    let open = input.current_token_start();
    input.next_token();
    let mut text = String::new();
    loop {
        let pos = input.current_token_start();
        match input.peek_token() {
            None => return Err(Error::new(Span::new(open, pos), "unterminated string")),
            Some('"') => {
                input.next_token();
                return Ok(text);
            }
            // Escapes are JSON's set minus \u, which waits for a real value model.
            Some('\\') => {
                let backslash = pos;
                input.next_token();
                let Some(esc) = input.peek_token() else {
                    return Err(Error::new(
                        Span::new(backslash, backslash + 1),
                        "unterminated escape sequence",
                    ));
                };
                text.push(match esc {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => {
                        return Err(Error::new(
                            Span::new(backslash, backslash + 2),
                            format!("unknown escape sequence `\\{other}`"),
                        ));
                    }
                });
                input.next_token();
            }
            Some(c) => {
                text.push(c);
                input.next_token();
            }
        }
    }
}

/// Left and right binding power. Left below right makes the operator left-associative.
///
/// `|` sits below comparison so that `a | select(. >= 2)` splits at the pipe, which is the
/// ordering jq uses and the reason this table exists rather than a nest of functions.
fn infix_power(tok: &Tok) -> Option<(BinOp, u8, u8)> {
    let (op, left, right) = match tok {
        Tok::EqEq => (BinOp::Eq, 5, 6),
        Tok::Ne => (BinOp::Ne, 5, 6),
        Tok::Lt => (BinOp::Lt, 5, 6),
        Tok::Le => (BinOp::Le, 5, 6),
        Tok::Gt => (BinOp::Gt, 5, 6),
        Tok::Ge => (BinOp::Ge, 5, 6),
        Tok::Plus => (BinOp::Add, 7, 8),
        Tok::Minus => (BinOp::Sub, 7, 8),
        Tok::Star => (BinOp::Mul, 9, 10),
        Tok::Slash => (BinOp::Div, 9, 10),
        Tok::Percent => (BinOp::Rem, 9, 10),
        _ => return None,
    };
    Some((op, left, right))
}

const PIPE_LEFT: u8 = 1;
const PIPE_RIGHT: u8 = 2;

/// The conditional sits between `|` and comparison, so `a if c else b | f` groups as
/// `(a if c else b) | f` and `x | a if c else b` groups as `x | (a if c else b)`. Python puts
/// its ternary below `|` because there `|` is bitwise or; ours is the pipe, so the better
/// grouping comes for free.
const COND_POWER: u8 = 3;

pub fn parse(src: &str) -> Result<File, Error> {
    let mut p = Cursor {
        input: LocatingSlice::new(src),
        src,
        declined_cross_line: None,
    };

    // Declarations in any order and any mix, since no kind can refer to another's position:
    // aliases and enums are resolved before any signature is read.
    let mut defs = Vec::new();
    let mut aliases = Vec::new();
    let mut enums = Vec::new();
    loop {
        let (tok, _) = p.peek()?;
        match tok {
            Tok::Pub => {
                p.advance()?;
                match p.peek()?.0 {
                    Tok::Enum => enums.push(p.enum_decl(true)?),
                    _ => defs.push(p.def(true)?),
                }
            }
            Tok::Fn => defs.push(p.def(false)?),
            Tok::Type => aliases.push(p.alias()?),
            Tok::Enum => enums.push(p.enum_decl(false)?),
            _ => break,
        }
    }

    let body = p.expr(0)?;
    let (rest, rest_span) = p.peek()?;
    if rest != Tok::Eof {
        return Err(p.unexpected(rest_span, format!("expected end of program, found {rest}")));
    }
    Ok(File {
        aliases,
        enums,
        defs,
        body,
    })
}

/// A module is declarations only, with no trailing expression -- there is nothing here to run,
/// only names for a program to import. `pub` marks which ones a program actually receives.
pub fn parse_module(src: &str) -> Result<Module, Error> {
    let mut p = Cursor {
        input: LocatingSlice::new(src),
        src,
        declined_cross_line: None,
    };
    let mut defs = Vec::new();
    let mut enums = Vec::new();
    loop {
        let (tok, span) = p.peek()?;
        match tok {
            Tok::Pub => {
                p.advance()?;
                match p.peek()?.0 {
                    Tok::Enum => enums.push(p.enum_decl(true)?),
                    _ => defs.push(p.def(true)?),
                }
            }
            Tok::Fn => defs.push(p.def(false)?),
            Tok::Enum => enums.push(p.enum_decl(false)?),
            Tok::Eof => break,
            other => {
                return Err(Error::new(
                    span,
                    format!("expected `fn` or end of module, found {other}"),
                ));
            }
        }
    }
    Ok(Module { defs, enums })
}

struct Cursor<'i> {
    input: Input<'i>,
    /// The full source, kept alongside the advancing `input` so the same-line rule can look at
    /// the trivia between two tokens it already has spans for.
    src: &'i str,
    /// Where a call was declined because its argument started on a later line: the argument
    /// token's start, and the callee's name. Declining is usually right -- a definition's body
    /// and the program's body sit adjacent with nothing between them, and the line break is what
    /// keeps one from swallowing the other -- but when the parse then fails at exactly the
    /// declined token, the intended reading was probably the call, and the error can name the
    /// parens spelling that works across lines.
    declined_cross_line: Option<(usize, String)>,
}

impl<'i> Cursor<'i> {
    /// Reads the next token without consuming it. `Input` is `Copy` (a source `&str` plus an
    /// offset), so looking ahead is just tokenizing a throwaway copy of the cursor.
    fn peek(&self) -> Result<(Tok, Span), Error> {
        let mut probe = self.input;
        read_tok(&mut probe)
    }

    fn advance(&mut self) -> Result<(Tok, Span), Error> {
        read_tok(&mut self.input)
    }

    fn eat(&mut self, want: Tok) -> Result<Span, Error> {
        let (tok, span) = self.peek()?;
        if tok != want {
            return Err(self.unexpected(span, format!("expected {want}, found {tok}")));
        }
        self.advance()?;
        Ok(span)
    }

    /// An "unexpected token" error, extended with the parens spelling when the failing token is
    /// one a call declined to take as a cross-line argument (see `declined_cross_line`): the
    /// token had somewhere to go, and only the line break kept it from going there.
    fn unexpected(&self, span: Span, msg: String) -> Error {
        if let Some((at, callee)) = &self.declined_cross_line
            && *at == span.start
        {
            return Error::new(
                span,
                format!(
                    "{msg}; an argument must start on the same line as its function -- \
                     write `{callee}(...)` to call across lines"
                ),
            );
        }
        Error::new(span, msg)
    }

    /// Whether nothing but spaces and tabs sits between two byte offsets. The same-line rule for
    /// call arguments: an argument, bare or parenthesized, must start on the same line as its
    /// function, which is what lets a definition's body end next to the program's body without
    /// either reaching across the line break and swallowing the other.
    fn same_line(&self, from: usize, to: usize) -> bool {
        !self.src[from..to].contains('\n')
    }

    /// The same-line rule applied at one call site: true when the argument starting at
    /// `arg_span` is on `callee_span`'s line. A cross-line decline is recorded (see
    /// `declined_cross_line`) so that if the parse then fails at that token, the error can say
    /// why the call was not taken.
    fn takes_argument(&mut self, callee: &str, callee_span: Span, arg_span: Span) -> bool {
        if self.same_line(callee_span.end, arg_span.start) {
            return true;
        }
        self.declined_cross_line = Some((arg_span.start, callee.to_string()));
        false
    }

    fn eat_ident(&mut self, what: &str) -> Result<(String, Span), Error> {
        match self.advance()? {
            (Tok::Ident(n), span) => Ok((n, span)),
            (other, span) => Err(Error::new(span, format!("expected {what}, found {other}"))),
        }
    }

    /// `fn name(param: Type) -> Type = body` or `fn name() -> Type = body`.
    ///
    /// Both annotations are required by the grammar rather than by the checker, which is what
    /// makes the message point at the missing annotation instead of at an inference failure.
    fn def(&mut self, is_pub: bool) -> Result<Def, Error> {
        let start = self.eat(Tok::Fn)?;
        let (name, _) = self.eat_ident("a name")?;
        self.eat(Tok::LParen)?;

        let (next, _) = self.peek()?;
        let param = if next == Tok::RParen {
            None
        } else {
            let (param_name, param_span) = self.eat_ident("a name")?;
            let (colon, _) = self.peek()?;
            if colon != Tok::Colon {
                return Err(Error::new(
                    param_span,
                    format!("parameter `{param_name}` needs a type annotation"),
                ));
            }
            self.advance()?;
            let param_ty = self.type_expr()?;
            Some(Param {
                span: param_span.to(param_ty.span()),
                name: param_name,
                ty: param_ty,
            })
        };

        let close = self.eat(Tok::RParen)?;
        let (arrow, _) = self.peek()?;
        if arrow != Tok::Arrow {
            return Err(Error::new(
                close,
                format!("function `{name}` needs a return type"),
            ));
        }
        self.advance()?;
        let ret = self.type_expr()?;

        self.eat(Tok::Eq)?;
        let body = self.expr(0)?;
        Ok(Def {
            span: start.to(body.span()),
            name,
            param,
            ret,
            body,
            is_pub,
        })
    }

    /// `Str`, `Int`, `Bool`, `Vec<T>`, `Stream<T>`, or `Opt<T>`.
    fn type_expr(&mut self) -> Result<TypeExpr, Error> {
        let (tok, span) = self.advance()?;
        if tok == Tok::LBrace {
            return self.record_type(span);
        }
        let name = match tok {
            Tok::Ident(n) => n,
            other => return Err(Error::new(span, format!("expected a type, found {other}"))),
        };
        if !ty::takes_type_arg(&name) {
            return Ok(TypeExpr::Named { name, span });
        }
        self.eat(Tok::Lt)?;
        let elem = self.type_expr()?;
        let close = self.eat(Tok::Gt)?;
        let (elem, span) = (Box::new(elem), span.to(close));
        Ok(match name.as_str() {
            "Vec" => TypeExpr::Vec { elem, span },
            "Stream" => TypeExpr::Stream { elem, span },
            _ => TypeExpr::Opt { elem, span },
        })
    }

    /// A call's argument when it opens with `(` or `{`: a parenthesized expression, or a record
    /// literal standing bare because `{` cannot start any other expression. The closing token
    /// marks the argument's end unambiguously, which is what makes the parens form the
    /// disambiguator wherever the bare form's own stopping rules would read a program
    /// differently.
    fn argument(&mut self) -> Result<(Expr, Span), Error> {
        let (tok, span) = self.peek()?;
        if tok == Tok::LBrace {
            self.advance()?;
            let lit = self.record_lit(span)?;
            let lit_span = lit.span();
            return Ok((lit, lit_span));
        }
        self.eat(Tok::LParen)?;
        let inner = self.expr(0)?;
        let close = self.eat(Tok::RParen)?;
        Ok((inner, close))
    }

    /// `{name: expr, age: expr}`, the value form of the brace that `record_type` reads in type
    /// position.
    fn record_lit(&mut self, open: Span) -> Result<Expr, Error> {
        let mut fields = Vec::new();
        let (first, _) = self.peek()?;
        if first != Tok::RBrace {
            loop {
                let (name, name_span) = self.eat_ident("a field name")?;
                self.eat(Tok::Colon)?;
                fields.push((name, name_span, self.expr(0)?));
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        Ok(Expr::RecordLit {
            fields,
            span: open.to(close),
        })
    }

    /// `enum Shape { point, circle{r: Int}, celsius(Int) }`
    ///
    /// A variant is a name, optionally followed by its payload type. The payload rule is any
    /// single type, spelled the way a call spells its argument: a record type directly in
    /// braces, or any type in parens.
    fn enum_decl(&mut self, is_pub: bool) -> Result<EnumDecl, Error> {
        let start = self.eat(Tok::Enum)?;
        let (name, _) = self.eat_ident("an enum name")?;
        self.eat(Tok::LBrace)?;
        let mut variants = Vec::new();
        let (first, _) = self.peek()?;
        if first != Tok::RBrace {
            loop {
                let (vname, vspan) = self.eat_ident("a variant name")?;
                let payload = match self.peek()? {
                    (Tok::LBrace, open) => {
                        self.advance()?;
                        Some(self.record_type(open)?)
                    }
                    (Tok::LParen, _) => {
                        self.advance()?;
                        let ty = self.type_expr()?;
                        self.eat(Tok::RParen)?;
                        Some(ty)
                    }
                    _ => None,
                };
                variants.push(Variant {
                    name: vname,
                    span: vspan,
                    payload,
                });
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        Ok(EnumDecl {
            name,
            variants,
            span: start.to(close),
            is_pub,
        })
    }

    /// `type Db = {users: Vec<User>}`
    fn alias(&mut self) -> Result<Alias, Error> {
        let start = self.eat(Tok::Type)?;
        let (name, _) = self.eat_ident("a type name")?;
        self.eat(Tok::Eq)?;
        let ty = self.type_expr()?;
        let span = start.to(ty.span());
        Ok(Alias { name, ty, span })
    }

    fn record_type(&mut self, open: Span) -> Result<TypeExpr, Error> {
        let mut fields = Vec::new();
        let (first, _) = self.peek()?;
        if first != Tok::RBrace {
            loop {
                let (fname, _) = self.eat_ident("a field name")?;
                self.eat(Tok::Colon)?;
                fields.push((fname, self.type_expr()?));
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        Ok(TypeExpr::Record {
            fields,
            span: open.to(close),
        })
    }

    /// Whether the tokens ahead begin a match arm: `name ->`, `name{a, b} ->`, or `any() ->`.
    /// The one place this has to look carefully is a brace: a record *pattern* holds bare names
    /// (and `..`), so the first `:` proves the braces are a constructor's record literal
    /// argument instead. A lexical error while probing is not an arm; the ordinary path will
    /// surface it.
    fn arm_starts_here(&self) -> bool {
        let mut probe = self.input;
        let mut next = || read_tok(&mut probe).map(|(t, _)| t);
        let Ok(Tok::Ident(name)) = next() else {
            return false;
        };
        match next() {
            Ok(Tok::Arrow) => true,
            Ok(Tok::LParen) if name == "any" => {
                matches!(next(), Ok(Tok::RParen)) && matches!(next(), Ok(Tok::Arrow))
            }
            Ok(Tok::LBrace) => {
                loop {
                    match next() {
                        Ok(Tok::Ident(_) | Tok::Comma | Tok::Dot) => continue,
                        Ok(Tok::RBrace) => break,
                        _ => return false,
                    }
                }
                matches!(next(), Ok(Tok::Arrow))
            }
            _ => false,
        }
    }

    /// `left -> body or left -> body or ...`, first match wins; `or` is the one arm composer.
    /// A left side is a variant pattern (`circle{r}`), `any()`, or a Bool guard expression; the
    /// chain's final element may instead be a bare expression, the default. The chain reads `.`
    /// as its subject, so it usually sits to the right of a `|`; each body stops at `|` and
    /// `or`, which is what lets the chain be one pipe stage.
    ///
    /// `lead` is an expression `expr` had already read when the `->` or `or` after it revealed
    /// a chain; the first element continues from it instead of parsing fresh.
    fn match_expr(&mut self, lead: Option<Expr>) -> Result<Expr, Error> {
        let mut arms = Vec::new();
        let mut lead = lead;
        loop {
            let (arm, bare) = match lead.take() {
                Some(e) => self.guard_or_default_arm(e)?,
                None if self.arm_starts_here() => {
                    let pattern = self.pattern()?;
                    self.eat(Tok::Arrow)?;
                    let body = self.expr(COND_POWER)?;
                    let span = pattern.span().to(body.span());
                    (
                        MatchArm {
                            pattern,
                            body,
                            span,
                        },
                        false,
                    )
                }
                None => {
                    let e = self.operand(COND_POWER)?;
                    self.guard_or_default_arm(e)?
                }
            };
            let arm_span = arm.span;
            arms.push(arm);
            let (sep, _) = self.peek()?;
            if sep != Tok::Or {
                break;
            }
            if bare {
                return Err(Error::new(
                    arm_span,
                    "a bare expression is the chain's default and must come last".to_string(),
                ));
            }
            self.advance()?;
        }
        let span = arms[0].span.to(arms[arms.len() - 1].span);
        Ok(Expr::Match { arms, span })
    }

    /// The element `left` opens: `left -> body` makes `left` a guard, and a bare `left` is the
    /// default, desugared to a `Default` arm whose body is `left` itself. Returns the arm and
    /// whether it was bare, which only the chain's last element may be.
    fn guard_or_default_arm(&mut self, left: Expr) -> Result<(MatchArm, bool), Error> {
        if self.peek()?.0 == Tok::Arrow {
            self.advance()?;
            let body = self.expr(COND_POWER)?;
            let span = left.span().to(body.span());
            return Ok((
                MatchArm {
                    pattern: Pattern::Guard(left),
                    body,
                    span,
                },
                false,
            ));
        }
        let span = left.span();
        Ok((
            MatchArm {
                pattern: Pattern::Default { span },
                body: left,
                span,
            },
            true,
        ))
    }

    fn pattern(&mut self) -> Result<Pattern, Error> {
        let (name, span) = self.eat_ident("a pattern")?;
        let (next, brace_span) = self.peek()?;
        if name == "any" && next == Tok::LParen {
            self.advance()?;
            let close = self.eat(Tok::RParen)?;
            return Ok(Pattern::Default {
                span: span.to(close),
            });
        }
        if next != Tok::LBrace {
            return Ok(Pattern::Variant {
                name,
                span,
                fields: None,
            });
        }
        self.advance()?;
        let mut names = Vec::new();
        let mut rest = false;
        let (first, _) = self.peek()?;
        if first != Tok::RBrace {
            loop {
                // `..` is two Dot tokens, and it ends the list: naming a field after "and the
                // rest" would make the marker meaningless.
                if self.peek()?.0 == Tok::Dot {
                    self.advance()?;
                    self.eat(Tok::Dot)?;
                    rest = true;
                    break;
                }
                names.push(self.eat_ident("a field name")?);
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        let fields = FieldsPattern {
            names,
            rest,
            span: brace_span.to(close),
        };
        Ok(Pattern::Variant {
            name,
            span: span.to(close),
            fields: Some(fields),
        })
    }

    fn expr(&mut self, min_power: u8) -> Result<Expr, Error> {
        // Arm chains begin only where an expression begins fresh (a pipe stage, a delimited
        // position). An arm body or a conditional branch enters at COND_POWER and leaves any
        // `->` or `or` it meets for the chain that owns it, instead of opening a nested chain
        // that would swallow the rest of the outer one.
        let fresh = min_power <= PIPE_RIGHT;
        let mut lhs = if fresh && self.arm_starts_here() {
            self.match_expr(None)?
        } else {
            let e = self.operand(min_power)?;
            let (tok, _) = self.peek()?;
            if fresh && (tok == Tok::Arrow || tok == Tok::Or) {
                self.match_expr(Some(e))?
            } else {
                e
            }
        };

        // Right-associative, so `a if c else b if d else e` chains rightward without parens.
        let (tok, _) = self.peek()?;
        if tok == Tok::If && COND_POWER >= min_power {
            self.advance()?;
            let cond = self.operand(COND_POWER + 1)?;
            self.eat(Tok::Else)?;
            let otherwise = self.expr(COND_POWER)?;
            let span = lhs.span().to(otherwise.span());
            lhs = Expr::Cond {
                then: Box::new(lhs),
                cond: Box::new(cond),
                otherwise: Box::new(otherwise),
                span,
            };
        }

        loop {
            let (tok, _) = self.peek()?;
            if tok != Tok::Pipe || PIPE_LEFT < min_power {
                break;
            }
            self.advance()?;
            let rhs = self.expr(PIPE_RIGHT)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Pipe {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        Ok(lhs)
    }

    fn operand(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;

        loop {
            let (tok, _) = self.peek()?;
            let Some((op, left, right)) = infix_power(&tok) else {
                break;
            };
            if left < min_power {
                break;
            }
            self.advance()?;
            let rhs = self.operand(right)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        Ok(lhs)
    }

    /// Negation binds tighter than any infix operator and looser than any postfix one, so
    /// `-a.b` negates the field and `-a * b` negates only `a`.
    fn unary(&mut self) -> Result<Expr, Error> {
        let (tok, span) = self.peek()?;
        if tok == Tok::Minus {
            self.advance()?;
            let base = self.unary()?;
            let full = span.to(base.span());
            return Ok(Expr::Neg {
                base: Box::new(base),
                span: full,
            });
        }
        self.postfix()
    }

    /// `[]` and `.name` bind tighter than any infix operator, so `a.b[] | c` projects `a.b`.
    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.atom()?;
        loop {
            let (tok, _) = self.peek()?;
            match tok {
                Tok::LBracket => {
                    self.advance()?;
                    let (next, _) = self.peek()?;
                    if next == Tok::RBracket {
                        let close = self.advance()?.1;
                        let span = e.span().to(close);
                        e = Expr::Project {
                            base: Box::new(e),
                            span,
                        };
                    } else {
                        let index = self.expr(0)?;
                        let close = self.eat(Tok::RBracket)?;
                        let span = e.span().to(close);
                        e = Expr::Index {
                            base: Box::new(e),
                            index: Box::new(index),
                            span,
                        };
                    }
                }
                Tok::Bang => {
                    let bang = self.advance()?.1;
                    let span = e.span().to(bang);
                    e = Expr::Unwrap {
                        base: Box::new(e),
                        span,
                    };
                }
                Tok::Dot => {
                    self.advance()?;
                    let (ft, fspan) = self.advance()?;
                    let Tok::Ident(name) = ft else {
                        return Err(Error::new(
                            fspan,
                            format!("expected a field name, found {ft}"),
                        ));
                    };
                    let span = e.span().to(fspan);
                    e = Expr::Field {
                        base: Box::new(e),
                        name,
                        span,
                    };
                }
                _ => return Ok(e),
            }
        }
    }

    /// What a name means once read: a qualified variant (`Shape.circle`), a call when an
    /// argument starts after it on the same line, or a plain variable reference.
    ///
    /// A name followed by an argument is a call; bare application is the default spelling and
    /// `f(x)` is `f` applied to the grouped atom `(x)`. The bare form asks two things of the
    /// call that the delimited forms do not: the callee must be lowercase (a capitalised bare
    /// call could never typecheck), and the argument's first token excludes `-` (already
    /// subtraction: `f -1` stays `f - 1`), `.` (projection binds tighter than bare application
    /// everywhere, so `p.b` is a field access even though `.b` alone could be an argument --
    /// issue #19), and `[` (indexing owns it for the same reason: `v[0]` indexes, so a Vec
    /// literal argument needs parens).
    fn ident_expr(&mut self, name: String, span: Span) -> Result<Expr, Error> {
        let (next, next_span) = self.peek()?;
        // `Shape.circle`: the casing rule makes uppercase-then-dot unambiguous, since a
        // capitalised name can never be a value for `.` to project a field out of.
        if next == Tok::Dot && name.chars().next().is_some_and(char::is_uppercase) {
            self.advance()?;
            let (variant, variant_span) = self.eat_ident("a variant name")?;
            let (after, after_span) = self.peek()?;
            let (payload, end) = if (after == Tok::LParen || after == Tok::LBrace)
                && self.takes_argument(&format!("{name}.{variant}"), variant_span, after_span)
            {
                let (arg, close) = self.argument()?;
                (Some(Box::new(arg)), close)
            } else {
                (None, variant_span)
            };
            return Ok(Expr::Variant {
                enum_name: name,
                enum_span: span,
                variant,
                variant_span,
                payload,
                span: span.to(end),
            });
        }
        let argument_starts = match next {
            Tok::LParen | Tok::LBrace => true,
            Tok::Str(_) | Tok::Int(_) | Tok::Input | Tok::Inputs | Tok::Lines | Tok::Ident(_) => {
                bare_callee(&name)
            }
            _ => false,
        };
        if !argument_starts || !self.takes_argument(&name, span, next_span) {
            return Ok(Expr::Var { name, span });
        }
        // `name()` is a nullary call, the one place a call's argument is optional: the empty
        // parens are checked for before falling into the ordinary single-expression parse, so
        // every other argument form (parenthesized, record, or bare) still names exactly one.
        let (arg, close) = if next == Tok::LParen {
            self.advance()?;
            let (after, _) = self.peek()?;
            if after == Tok::RParen {
                let close = self.advance()?.1;
                (None, close)
            } else {
                let inner = self.expr(0)?;
                let close = self.eat(Tok::RParen)?;
                (Some(inner), close)
            }
        } else if next == Tok::LBrace {
            let (lit, close) = self.argument()?;
            (Some(lit), close)
        } else {
            // A bare argument is a postfix chain, not an operand: an infix operator after it
            // belongs to the enclosing expression, so `f x + y` is `f(x) + y`. Chaining is
            // right-recursive through `atom` itself, making `f g x` read `f(g(x))` -- with no
            // first-class functions, the only reading that could ever typecheck.
            let arg = self.postfix()?;
            let end = arg.span();
            (Some(arg), end)
        };
        Ok(Expr::Call {
            func: name,
            func_span: span,
            arg: arg.map(Box::new),
            span: span.to(close),
        })
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let (tok, span) = self.advance()?;
        match tok {
            Tok::Str(text) => Ok(Expr::Str { text, span }),
            Tok::Int(value) => Ok(Expr::Int { value, span }),
            Tok::Input => Ok(Expr::Input { span }),
            Tok::Inputs => Ok(Expr::Inputs { span }),
            Tok::Lines => Ok(Expr::Lines { span }),

            // `.name` is field access on the subject, so the leading dot yields `.` and the
            // postfix loop above picks the field up.
            Tok::Dot => {
                let (next, _) = self.peek()?;
                if let Tok::Ident(name) = next {
                    let (_, fspan) = self.advance()?;
                    return Ok(Expr::Field {
                        base: Box::new(Expr::Subject { span }),
                        name,
                        span: span.to(fspan),
                    });
                }
                Ok(Expr::Subject { span })
            }

            Tok::LBrace => self.record_lit(span),

            Tok::LBracket => {
                // `,` is a separator here, not an operator. It has no meaning outside a literal
                // while everything stays in the value layer.
                let mut items = Vec::new();
                let (first, _) = self.peek()?;
                if first != Tok::RBracket {
                    loop {
                        items.push(self.expr(0)?);
                        let (sep, _) = self.peek()?;
                        if sep != Tok::Comma {
                            break;
                        }
                        self.advance()?;
                    }
                }
                let close = self.eat(Tok::RBracket)?;
                Ok(Expr::VecLit {
                    items,
                    span: span.to(close),
                })
            }

            Tok::Ident(name) => self.ident_expr(name, span),

            Tok::LParen => {
                let inner = self.expr(0)?;
                self.eat(Tok::RParen)?;
                Ok(inner)
            }

            other => Err(Error::new(
                span,
                format!("expected an expression, found {other}"),
            )),
        }
    }
}

/// Whether `name` can take a bare (undelimited) argument. Only a lowercase name can: a
/// capitalised name is a type under the casing rule, so a capitalised bare call could never
/// typecheck. Constructors still apply with the delimited forms (`Circle{r: 1}`,
/// `Circle(...)`), which this does not gate.
fn bare_callee(name: &str) -> bool {
    !name.chars().next().is_some_and(char::is_uppercase)
}
