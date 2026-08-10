use crate::ast::{BinOp, Def, Expr, File, Param, TypeExpr};
use crate::error::Error;
use crate::lex::{Tok, Token};

/// Left and right binding power. Left below right makes the operator left-associative.
///
/// `|` sits below comparison so that `a | select(. >= 2)` splits at the pipe, which is the
/// ordering jq uses and the reason this table exists rather than a nest of functions.
fn infix_power(tok: &Tok) -> Option<(BinOp, u8, u8)> {
    let (op, left, right) = match tok {
        Tok::EqEq => (BinOp::Eq, 3, 4),
        Tok::Ne => (BinOp::Ne, 3, 4),
        Tok::Lt => (BinOp::Lt, 3, 4),
        Tok::Le => (BinOp::Le, 3, 4),
        Tok::Gt => (BinOp::Gt, 3, 4),
        Tok::Ge => (BinOp::Ge, 3, 4),
        Tok::Plus => (BinOp::Add, 5, 6),
        _ => return None,
    };
    Some((op, left, right))
}

const PIPE_LEFT: u8 = 1;
const PIPE_RIGHT: u8 = 2;

pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    let mut p = Parser { tokens, pos: 0 };

    let mut defs = Vec::new();
    while p.peek().tok == Tok::Fn {
        defs.push(p.def()?);
    }

    let body = p.expr(0)?;
    let rest = p.peek();
    if rest.tok != Tok::Eof {
        return Err(Error::new(rest.span, format!("expected end of program, found {}", rest.tok)));
    }
    Ok(File { defs, body })
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

    fn expr(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.operand(min_power)?;

        while self.peek().tok == Tok::Pipe && PIPE_LEFT >= min_power {
            self.advance();
            let rhs = self.expr(PIPE_RIGHT)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Pipe { lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }

        Ok(lhs)
    }

    fn operand(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.postfix()?;

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

    /// `[]` binds tighter than any infix operator, so `a[] | b` projects `a` and pipes that.
    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.atom()?;
        while self.peek().tok == Tok::LBracket {
            self.advance();
            let close = self.eat(Tok::RBracket)?.span;
            let span = e.span().to(close);
            e = Expr::Project { base: Box::new(e), span };
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let t = self.advance();
        match &t.tok {
            Tok::Str(text) => Ok(Expr::Str { text: text.clone(), span: t.span }),
            Tok::Int(value) => Ok(Expr::Int { value: *value, span: t.span }),
            Tok::Dot => Ok(Expr::Subject { span: t.span }),

            Tok::Select => {
                self.eat(Tok::LParen)?;
                let pred = self.expr(0)?;
                let close = self.eat(Tok::RParen)?.span;
                Ok(Expr::Select { pred: Box::new(pred), span: t.span.to(close) })
            }

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
                if self.peek().tok != Tok::LParen {
                    return Ok(Expr::Var { name: name.clone(), span: t.span });
                }
                self.advance();
                let arg = self.expr(0)?;
                let close = self.eat(Tok::RParen)?.span;
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
