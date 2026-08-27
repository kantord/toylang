use winnow::error::ParserError;
use winnow::prelude::*;
use winnow::stream::{LocatingSlice, Location, Stream};
use winnow::token::take_while;

use crate::ast::{
    Alias, BinOp, Def, EnumDecl, Expr, FieldsPattern, File, MatchArm, Module, Param, Pattern,
    Span, TypeExpr, Variant,
};
use crate::error::Error;

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
    Plus,
    Minus,
    Star,
    Slash,
    SlashSlash,
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
            Tok::Plus => "`+`",
            Tok::Minus => "`-`",
            Tok::Star => "`*`",
            Tok::Slash => "`/`",
            Tok::SlashSlash => "`//`",
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
            let word = take_while::<_, _, Error>(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
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
        '/' => {
            input.next_token();
            if input.peek_token() == Some('/') {
                input.next_token();
                Tok::SlashSlash
            } else {
                Tok::Slash
            }
        }
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
    let mut p = Cursor { input: LocatingSlice::new(src), bare_ok: true };

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
        return Err(Error::new(rest_span, format!("expected end of program, found {rest}")));
    }
    Ok(File { aliases, enums, defs, body })
}

/// A module is declarations only, with no trailing expression -- there is nothing here to run,
/// only names for a program to import. `pub` marks which ones a program actually receives.
pub fn parse_module(src: &str) -> Result<Module, Error> {
    let mut p = Cursor { input: LocatingSlice::new(src), bare_ok: true };
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
                return Err(Error::new(span, format!("expected `fn` or end of module, found {other}")));
            }
        }
    }
    Ok(Module { defs, enums })
}

struct Cursor<'i> {
    input: Input<'i>,
    /// Off while parsing a function body's own undelimited chain (see `def` and `root`). A
    /// definition's body and whatever follows it -- another `fn`, or the file's own body -- sit
    /// adjacent with no token between them, the one place in the grammar two `expr` calls meet
    /// without a delimiter to anchor on. A trailing bare call there could reach across that
    /// boundary and swallow unrelated content, so bare calls are suspended for the outermost
    /// chain of a definition's body and restored by `delimited` the moment real bracketing
    /// (`(...)`, `[...]`, `{...}`) is entered, since a closing token bounds those regardless of
    /// what sits outside.
    bare_ok: bool,
}

impl<'i> Cursor<'i> {
    /// Reads the next token without consuming it. `Input` is `Copy` (a source `&str` plus an
    /// offset), so looking ahead is just tokenizing a throwaway copy of the cursor.
    fn peek(&self) -> Result<(Tok, Span), Error> {
        let mut probe = self.input;
        read_tok(&mut probe)
    }

    /// One token past `peek`.
    fn peek2(&self) -> Result<(Tok, Span), Error> {
        let mut probe = self.input;
        read_tok(&mut probe)?;
        read_tok(&mut probe)
    }

    fn advance(&mut self) -> Result<(Tok, Span), Error> {
        read_tok(&mut self.input)
    }

    fn eat(&mut self, want: Tok) -> Result<Span, Error> {
        let (tok, span) = self.peek()?;
        if tok != want {
            return Err(Error::new(span, format!("expected {want}, found {tok}")));
        }
        self.advance()?;
        Ok(span)
    }

    fn eat_ident(&mut self, what: &str) -> Result<(String, Span), Error> {
        match self.advance()? {
            (Tok::Ident(n), span) => Ok((n, span)),
            (other, span) => Err(Error::new(span, format!("expected {what}, found {other}"))),
        }
    }

    /// Runs `f` with bare calls turned back on, for content bounded by a real delimiter. See
    /// `bare_ok`.
    fn delimited<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        let saved = self.bare_ok;
        self.bare_ok = true;
        let out = f(self);
        self.bare_ok = saved;
        out
    }

    /// `fn name(param: Type) -> Type = body`
    ///
    /// Both annotations are required by the grammar rather than by the checker, which is what
    /// makes the message point at the missing annotation instead of at an inference failure.
    fn def(&mut self, is_pub: bool) -> Result<Def, Error> {
        let start = self.eat(Tok::Fn)?;
        let (name, _) = self.eat_ident("a name")?;
        self.eat(Tok::LParen)?;

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
        let param = Param { span: param_span.to(param_ty.span()), name: param_name, ty: param_ty };

        let close = self.eat(Tok::RParen)?;
        let (arrow, _) = self.peek()?;
        if arrow != Tok::Arrow {
            return Err(Error::new(close, format!("function `{name}` needs a return type")));
        }
        self.advance()?;
        let ret = self.type_expr()?;

        self.eat(Tok::Eq)?;
        self.bare_ok = false;
        let body = self.expr(0);
        self.bare_ok = true;
        let body = body?;
        Ok(Def { span: start.to(body.span()), name, param, ret, body, is_pub })
    }

    /// `Str`, `Int`, `Bool`, or `Vec<T>`.
    fn type_expr(&mut self) -> Result<TypeExpr, Error> {
        let (tok, span) = self.advance()?;
        if tok == Tok::LBrace {
            return self.record_type(span);
        }
        let name = match tok {
            Tok::Ident(n) => n,
            other => return Err(Error::new(span, format!("expected a type, found {other}"))),
        };
        if name != "Vec" {
            return Ok(TypeExpr::Named { name, span });
        }
        self.eat(Tok::Lt)?;
        let elem = self.type_expr()?;
        let close = self.eat(Tok::Gt)?;
        Ok(TypeExpr::Vec { elem: Box::new(elem), span: span.to(close) })
    }

    /// A call's argument, wrapped in parens or spelled as a bare record literal. This is the
    /// "nested" call form: legal anywhere an atom is legal, including as an operand, because the
    /// closing `)` or `}` marks its end unambiguously wherever it appears.
    ///
    /// The parens may be omitted when the argument is a record literal. That is unambiguous
    /// because `{` cannot start any other expression and cannot follow one, so `f {` was a syntax
    /// error before this and nothing is taken away by giving it a meaning.
    fn argument(&mut self) -> Result<(Expr, Span), Error> {
        let (tok, span) = self.peek()?;
        if tok == Tok::LBrace {
            self.advance()?;
            let lit = self.record_lit(span)?;
            let lit_span = lit.span();
            return Ok((lit, lit_span));
        }
        self.eat(Tok::LParen)?;
        let inner = self.delimited(|p| p.expr(0))?;
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
                fields.push((name, name_span, self.delimited(|p| p.expr(0))?));
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        Ok(Expr::RecordLit { fields, span: open.to(close) })
    }

    /// `enum Shape { point, circle{r: Int} }`
    ///
    /// A variant is a name, optionally followed by a record type spelling its payload. The
    /// payload rule is any single type, but the record form is the only spelling that exists so
    /// far, the same way arguments already travel as records.
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
                    _ => None,
                };
                variants.push(Variant { name: vname, span: vspan, payload });
                let (sep, _) = self.peek()?;
                if sep != Tok::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        let close = self.eat(Tok::RBrace)?;
        Ok(EnumDecl { name, variants, span: start.to(close), is_pub })
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
        Ok(TypeExpr::Record { fields, span: open.to(close) })
    }

    /// `f x`, the parenless "root" call form. Legal only where `expr` is entered fresh (a pipe
    /// stage, a function body, inside `(...)`/`[...]`/`{...}`), never as an operand: `operand`
    /// and everything it calls (`unary`, `postfix`, `atom`) never look for this, so it cannot
    /// surface partway through a larger expression. `x` stops at the first infix operator, `|`,
    /// or `if`, which is what makes `f x | y` mean `(f x) | y` and `f x + y` a parse error
    /// (the trailing `+ y` is left for the caller to reject) rather than a silent `(f x) + y`.
    fn root(&mut self, min_power: u8) -> Result<Expr, Error> {
        if self.bare_ok {
            let (tok, span) = self.peek()?;
            if let Tok::Ident(name) = tok
                && bare_callee(&name)
            {
                let (tok2, _) = self.peek2()?;
                if starts_bare_argument(&tok2) {
                    self.advance()?;
                    let arg = self.bare_argument()?;
                    let full = span.to(arg.span());
                    return Ok(Expr::Call { func: name, func_span: span, arg: Box::new(arg), span: full });
                }
            }
        }
        self.operand(min_power)
    }

    /// The argument of a bare call: another bare call (so `f g x` is `f(g(x))`, right-recursive)
    /// or an ordinary postfix chain. Not `expr`: an infix operator, `|`, or `if` here belongs to
    /// whatever encloses the whole application, not to this argument.
    fn bare_argument(&mut self) -> Result<Expr, Error> {
        let (tok, span) = self.peek()?;
        if let Tok::Ident(name) = tok
            && bare_callee(&name)
        {
            let (tok2, _) = self.peek2()?;
            if starts_bare_argument(&tok2) {
                self.advance()?;
                let arg = self.bare_argument()?;
                let full = span.to(arg.span());
                return Ok(Expr::Call { func: name, func_span: span, arg: Box::new(arg), span: full });
            }
        }
        self.postfix()
    }

    /// Whether the tokens ahead begin a match arm: `name ->`, `name{a, b} ->`, or `any() ->`.
    /// The one place this has to look carefully is a brace: a record *pattern* holds bare names
    /// (and `..`), so the first `:` proves the braces are a constructor's record literal
    /// argument instead. A lexical error while probing is not an arm; the ordinary path will
    /// surface it.
    fn arm_starts_here(&self) -> bool {
        let mut probe = self.input;
        let mut next = || read_tok(&mut probe).map(|(t, _)| t);
        let Ok(Tok::Ident(name)) = next() else { return false };
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

    /// `pattern -> body // pattern -> body // ...`, first match wins. The chain reads `.` as
    /// its subject, so it usually sits to the right of a `|`; each body stops at `|` or `//`,
    /// which is what lets the chain be one pipe stage.
    fn match_expr(&mut self) -> Result<Expr, Error> {
        let mut arms = Vec::new();
        loop {
            let pattern = self.pattern()?;
            self.eat(Tok::Arrow)?;
            let body = self.expr(COND_POWER)?;
            let span = pattern.span().to(body.span());
            arms.push(MatchArm { pattern, body, span });
            let (sep, _) = self.peek()?;
            if sep != Tok::SlashSlash {
                break;
            }
            self.advance()?;
        }
        let span = arms[0].span.to(arms[arms.len() - 1].span);
        Ok(Expr::Match { arms, span })
    }

    fn pattern(&mut self) -> Result<Pattern, Error> {
        let (name, span) = self.eat_ident("a pattern")?;
        let (next, brace_span) = self.peek()?;
        if name == "any" && next == Tok::LParen {
            self.advance()?;
            let close = self.eat(Tok::RParen)?;
            return Ok(Pattern::Default { span: span.to(close) });
        }
        if next != Tok::LBrace {
            return Ok(Pattern::Variant { name, span, fields: None });
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
        let fields = FieldsPattern { names, rest, span: brace_span.to(close) };
        Ok(Pattern::Variant { name, span: span.to(close), fields: Some(fields) })
    }

    fn expr(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs =
            if self.arm_starts_here() { self.match_expr()? } else { self.root(min_power)? };

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
            lhs = Expr::Pipe { lhs: Box::new(lhs), rhs: Box::new(rhs), span };
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
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
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
            return Ok(Expr::Neg { base: Box::new(base), span: full });
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
                        e = Expr::Project { base: Box::new(e), span };
                    } else {
                        let index = self.delimited(|p| p.expr(0))?;
                        let close = self.eat(Tok::RBracket)?;
                        let span = e.span().to(close);
                        e = Expr::Index { base: Box::new(e), index: Box::new(index), span };
                    }
                }
                Tok::Bang => {
                    let bang = self.advance()?.1;
                    let span = e.span().to(bang);
                    e = Expr::Unwrap { base: Box::new(e), span };
                }
                Tok::Dot => {
                    self.advance()?;
                    let (ft, fspan) = self.advance()?;
                    let Tok::Ident(name) = ft else {
                        return Err(Error::new(fspan, format!("expected a field name, found {ft}")));
                    };
                    let span = e.span().to(fspan);
                    e = Expr::Field { base: Box::new(e), name, span };
                }
                _ => return Ok(e),
            }
        }
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
                        items.push(self.delimited(|p| p.expr(0))?);
                        let (sep, _) = self.peek()?;
                        if sep != Tok::Comma {
                            break;
                        }
                        self.advance()?;
                    }
                }
                let close = self.eat(Tok::RBracket)?;
                Ok(Expr::VecLit { items, span: span.to(close) })
            }

            Tok::Ident(name) => {
                let (next, _) = self.peek()?;
                // `Shape.circle`: the casing rule makes uppercase-then-dot unambiguous, since a
                // capitalised name can never be a value for `.` to project a field out of.
                if next == Tok::Dot && name.chars().next().is_some_and(char::is_uppercase) {
                    self.advance()?;
                    let (variant, variant_span) = self.eat_ident("a variant name")?;
                    let (after, _) = self.peek()?;
                    let (payload, end) = if after == Tok::LParen || after == Tok::LBrace {
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
                if next != Tok::LParen && next != Tok::LBrace {
                    return Ok(Expr::Var { name, span });
                }
                let (arg, close) = self.argument()?;
                Ok(Expr::Call { func: name, func_span: span, arg: Box::new(arg), span: span.to(close) })
            }

            Tok::LParen => {
                let inner = self.delimited(|p| p.expr(0))?;
                self.eat(Tok::RParen)?;
                Ok(inner)
            }

            other => Err(Error::new(span, format!("expected an expression, found {other}"))),
        }
    }
}

/// Whether `name tok` reads as `name` applied bare to an argument starting with `tok`. `(` and
/// `{` are excluded because those are the "nested" call form (`argument`), which works
/// unconditionally and does not need root placement. `-` is excluded because it is already
/// subtraction: `f -x` stays `f - x`, the same resolution Haskell gives the identical clash,
/// rather than adding a rule to prefer negation.
/// Whether `name` can be a bare call's function. Only a lowercase name can: functions are
/// values under the casing rule, and a capitalised name followed by `.` is the qualified
/// variant spelling (`Shape.circle`), which would otherwise be swallowed as `Shape (.circle)`
/// since `.` also starts a bare argument.
fn bare_callee(name: &str) -> bool {
    !name.chars().next().is_some_and(char::is_uppercase)
}

fn starts_bare_argument(tok: &Tok) -> bool {
    matches!(
        tok,
        Tok::Str(_)
            | Tok::Int(_)
            | Tok::Input
            | Tok::Inputs
            | Tok::Lines
            | Tok::Dot
            | Tok::LBracket
            | Tok::Ident(_)
    )
}
