use crate::ast::{Alias, BinOp, Def, Expr, File, Param, TypeExpr};
use crate::error::Error;
use crate::lex::{Tok, Token};

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

pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    let mut p = Parser { tokens, pos: 0 };

    // Declarations in any order and any mix, since neither kind can refer to the other's
    // position: aliases are resolved before any signature is read.
    let mut defs = Vec::new();
    let mut aliases = Vec::new();
    loop {
        match p.peek().tok {
            Tok::Fn => defs.push(p.def()?),
            Tok::Type => aliases.push(p.alias()?),
            _ => break,
        }
    }

    let body = p.expr(0)?;
    let rest = p.peek();
    if rest.tok != Tok::Eof {
        return Err(Error::new(rest.span, format!("expected end of program, found {}", rest.tok)));
    }
    Ok(File { aliases, defs, body })
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &'a Token {
        // The lexer always emits Eof, so the index cannot run past the end.
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &'a Token {
        let t = &self.tokens[self.pos];
        if t.tok != Tok::Eof {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, want: Tok) -> Result<&'a Token, Error> {
        let t = self.peek();
        if t.tok != want {
            return Err(Error::new(t.span, format!("expected {want}, found {}", t.tok)));
        }
        Ok(self.advance())
    }

    /// `fn name(param: Type) -> Type = body`
    ///
    /// Both annotations are required by the grammar rather than by the checker, which is what
    /// makes the message point at the missing annotation instead of at an inference failure.
    fn def(&mut self) -> Result<Def, Error> {
        let start = self.eat(Tok::Fn)?.span;
        let name = match &self.advance().tok {
            Tok::Ident(n) => n.clone(),
            other => {
                let t = &self.tokens[self.pos - 1];
                return Err(Error::new(t.span, format!("expected a name, found {other}")));
            }
        };
        self.eat(Tok::LParen)?;

        let pt = self.advance();
        let (param_name, param_span) = match &pt.tok {
            Tok::Ident(n) => (n.clone(), pt.span),
            other => return Err(Error::new(pt.span, format!("expected a name, found {other}"))),
        };
        if self.peek().tok != Tok::Colon {
            return Err(Error::new(
                param_span,
                format!("parameter `{param_name}` needs a type annotation"),
            ));
        }
        self.advance();
        let param_ty = self.type_expr()?;
        let param = Param { span: param_span.to(param_ty.span()), name: param_name, ty: param_ty };

        let close = self.eat(Tok::RParen)?.span;
        if self.peek().tok != Tok::Arrow {
            return Err(Error::new(close, format!("function `{name}` needs a return type")));
        }
        self.advance();
        let ret = self.type_expr()?;

        self.eat(Tok::Eq)?;
        let body = self.expr(0)?;
        Ok(Def { span: start.to(body.span()), name, param, ret, body })
    }

    /// `Str`, `Int`, `Bool`, or `Vec<T>`.
    fn type_expr(&mut self) -> Result<TypeExpr, Error> {
        let t = self.advance();
        if t.tok == Tok::LBrace {
            return self.record_type(t.span);
        }
        let name = match &t.tok {
            Tok::Ident(n) => n.clone(),
            other => return Err(Error::new(t.span, format!("expected a type, found {other}"))),
        };
        if name != "Vec" {
            return Ok(TypeExpr::Named { name, span: t.span });
        }
        self.eat(Tok::Lt)?;
        let elem = self.type_expr()?;
        let close = self.eat(Tok::Gt)?.span;
        Ok(TypeExpr::Vec { elem: Box::new(elem), span: t.span.to(close) })
    }

    /// `{name: Str, age: Int}`
    /// A call's argument, and the argument of `map` and `select`, which are keyword forms rather
    /// than calls and would otherwise not share the rule.
    ///
    /// The parens may be omitted when the argument is a record literal. That is unambiguous
    /// because `{` cannot start any other expression and cannot follow one, so `f {` was a syntax
    /// error before this and nothing is taken away by giving it a meaning.
    fn argument(&mut self) -> Result<(Expr, crate::ast::Span), Error> {
        if self.peek().tok == Tok::LBrace {
            let open = self.advance().span;
            let lit = self.record_lit(open)?;
            let span = lit.span();
            return Ok((lit, span));
        }
        self.eat(Tok::LParen)?;
        let inner = self.expr(0)?;
        let close = self.eat(Tok::RParen)?.span;
        Ok((inner, close))
    }

    /// `{name: expr, age: expr}`, the value form of the brace that `record_type` reads in type
    /// position.
    fn record_lit(&mut self, open: crate::ast::Span) -> Result<Expr, Error> {
        let mut fields = Vec::new();
        if self.peek().tok != Tok::RBrace {
            loop {
                let ct = self.advance();
                let name = match &ct.tok {
                    Tok::Ident(n) => n.clone(),
                    other => {
                        return Err(Error::new(
                            ct.span,
                            format!("expected a field name, found {other}"),
                        ));
                    }
                };
                self.eat(Tok::Colon)?;
                fields.push((name, ct.span, self.expr(0)?));
                if self.peek().tok != Tok::Comma {
                    break;
                }
                self.advance();
            }
        }
        let close = self.eat(Tok::RBrace)?.span;
        Ok(Expr::RecordLit { fields, span: open.to(close) })
    }

    /// `type Db = {users: Vec<User>}`
    fn alias(&mut self) -> Result<Alias, Error> {
        let start = self.eat(Tok::Type)?.span;
        let t = self.advance();
        let name = match &t.tok {
            Tok::Ident(n) => n.clone(),
            other => {
                return Err(Error::new(t.span, format!("expected a type name, found {other}")));
            }
        };
        self.eat(Tok::Eq)?;
        let ty = self.type_expr()?;
        let span = start.to(ty.span());
        Ok(Alias { name, ty, span })
    }

    fn record_type(&mut self, open: crate::ast::Span) -> Result<TypeExpr, Error> {
        let mut fields = Vec::new();
        if self.peek().tok != Tok::RBrace {
            loop {
                let ft = self.advance();
                let fname = match &ft.tok {
                    Tok::Ident(n) => n.clone(),
                    other => {
                        return Err(Error::new(
                            ft.span,
                            format!("expected a field name, found {other}"),
                        ));
                    }
                };
                self.eat(Tok::Colon)?;
                fields.push((fname, self.type_expr()?));
                if self.peek().tok != Tok::Comma {
                    break;
                }
                self.advance();
            }
        }
        let close = self.eat(Tok::RBrace)?.span;
        Ok(TypeExpr::Record { fields, span: open.to(close) })
    }

    fn expr(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.operand(min_power)?;

        // Right-associative, so `a if c else b if d else e` chains rightward without parens.
        if self.peek().tok == Tok::If && COND_POWER >= min_power {
            self.advance();
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

        while self.peek().tok == Tok::Pipe && PIPE_LEFT >= min_power {
            self.advance();
            let rhs = self.expr(PIPE_RIGHT)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Pipe { lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }

        Ok(lhs)
    }

    fn operand(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;

        while let Some((op, left, right)) = infix_power(&self.peek().tok) {
            if left < min_power {
                break;
            }
            self.advance();
            let rhs = self.operand(right)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }

        Ok(lhs)
    }

    /// Negation binds tighter than any infix operator and looser than any postfix one, so
    /// `-a.b` negates the field and `-a * b` negates only `a`.
    fn unary(&mut self) -> Result<Expr, Error> {
        if self.peek().tok == Tok::Minus {
            let minus = self.advance().span;
            let base = self.unary()?;
            let span = minus.to(base.span());
            return Ok(Expr::Neg { base: Box::new(base), span });
        }
        self.postfix()
    }

    /// `[]` and `.name` bind tighter than any infix operator, so `a.b[] | c` projects `a.b`.
    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.atom()?;
        loop {
            match self.peek().tok {
                Tok::LBracket => {
                    self.advance();
                    if self.peek().tok == Tok::RBracket {
                        let close = self.advance().span;
                        let span = e.span().to(close);
                        e = Expr::Project { base: Box::new(e), span };
                    } else {
                        let index = self.expr(0)?;
                        let close = self.eat(Tok::RBracket)?.span;
                        let span = e.span().to(close);
                        e = Expr::Index {
                            base: Box::new(e),
                            index: Box::new(index),
                            span,
                        };
                    }
                }
                Tok::Bang => {
                    let bang = self.advance().span;
                    let span = e.span().to(bang);
                    e = Expr::Unwrap { base: Box::new(e), span };
                }
                Tok::Dot => {
                    self.advance();
                    let ft = self.advance();
                    let Tok::Ident(name) = &ft.tok else {
                        return Err(Error::new(
                            ft.span,
                            format!("expected a field name, found {}", ft.tok),
                        ));
                    };
                    let span = e.span().to(ft.span);
                    e = Expr::Field { base: Box::new(e), name: name.clone(), span };
                }
                _ => return Ok(e),
            }
        }
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let t = self.advance();
        match &t.tok {
            Tok::Str(text) => Ok(Expr::Str { text: text.clone(), span: t.span }),
            Tok::Int(value) => Ok(Expr::Int { value: *value, span: t.span }),
            Tok::Input => Ok(Expr::Input { span: t.span }),

            // `.name` is field access on the subject, so the leading dot yields `.` and the
            // postfix loop above picks the field up.
            Tok::Dot => {
                if let Tok::Ident(name) = &self.peek().tok {
                    let name = name.clone();
                    let ft = self.advance();
                    return Ok(Expr::Field {
                        base: Box::new(Expr::Subject { span: t.span }),
                        name,
                        span: t.span.to(ft.span),
                    });
                }
                Ok(Expr::Subject { span: t.span })
            }

            Tok::Select => {
                let (pred, close) = self.argument()?;
                Ok(Expr::Select { pred: Box::new(pred), span: t.span.to(close) })
            }

            Tok::Map => {
                let (body, close) = self.argument()?;
                Ok(Expr::Map { body: Box::new(body), span: t.span.to(close) })
            }

            Tok::LBrace => self.record_lit(t.span),

            Tok::LBracket => {
                // `,` is a separator here, not an operator. It has no meaning outside a literal
                // while everything stays in the value layer.
                let mut items = Vec::new();
                if self.peek().tok != Tok::RBracket {
                    loop {
                        items.push(self.expr(0)?);
                        if self.peek().tok != Tok::Comma {
                            break;
                        }
                        self.advance();
                    }
                }
                let close = self.eat(Tok::RBracket)?.span;
                Ok(Expr::VecLit { items, span: t.span.to(close) })
            }

            Tok::Ident(name) => {
                if self.peek().tok != Tok::LParen && self.peek().tok != Tok::LBrace {
                    return Ok(Expr::Var { name: name.clone(), span: t.span });
                }
                let (arg, close) = self.argument()?;
                Ok(Expr::Call {
                    func: name.clone(),
                    func_span: t.span,
                    arg: Box::new(arg),
                    span: t.span.to(close),
                })
            }

            Tok::LParen => {
                let inner = self.expr(0)?;
                self.eat(Tok::RParen)?;
                Ok(inner)
            }

            other => Err(Error::new(t.span, format!("expected an expression, found {other}"))),
        }
    }
}
