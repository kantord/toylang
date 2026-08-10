use crate::ast::{BinOp, Def, Expr, File, Param, Span, TypeExpr};
use crate::error::Error;
use crate::lex::{Tok, Token};

/// Left and right binding power. Left below right makes the operator left-associative.
///
/// One entry today. The table exists because `|` and `,` sit below comparison at step 4, and
/// precedence is the thing hand-written parsers get rewritten over when it arrives late.
fn infix_power(tok: &Tok) -> Option<(BinOp, u8, u8)> {
    match tok {
        Tok::Plus => Some((BinOp::Add, 1, 2)),
        _ => None,
    }
}

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

    fn ident(&mut self) -> Result<(String, Span), Error> {
        let t = self.advance();
        match &t.tok {
            Tok::Ident(name) => Ok((name.clone(), t.span)),
            other => Err(Error::new(t.span, format!("expected a name, found {other}"))),
        }
    }

    /// `fn name(param: Type) -> Type = body`
    ///
    /// Both annotations are required by the grammar rather than by the checker, which is what
    /// makes the message point at the missing annotation instead of at an inference failure.
    fn def(&mut self) -> Result<Def, Error> {
        let start = self.eat(Tok::Fn)?.span;
        let (name, _) = self.ident()?;
        self.eat(Tok::LParen)?;

        let (param_name, param_span) = self.ident()?;
        if self.peek().tok != Tok::Colon {
            return Err(Error::new(
                param_span,
                format!("parameter `{param_name}` needs a type annotation"),
            ));
        }
        self.advance();
        let param_ty = self.type_expr()?;
        let param = Param { span: param_span.to(param_ty.span), name: param_name, ty: param_ty };

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

    fn type_expr(&mut self) -> Result<TypeExpr, Error> {
        let t = self.advance();
        match &t.tok {
            Tok::Ident(name) => Ok(TypeExpr { name: name.clone(), span: t.span }),
            other => Err(Error::new(t.span, format!("expected a type, found {other}"))),
        }
    }

    fn expr(&mut self, min_power: u8) -> Result<Expr, Error> {
        let mut lhs = self.atom()?;

        while let Some((op, left, right)) = infix_power(&self.peek().tok) {
            if left < min_power {
                break;
            }
            self.advance();
            let rhs = self.expr(right)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }

        Ok(lhs)
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let t = self.advance();
        match &t.tok {
            Tok::Str(text) => Ok(Expr::Str { text: text.clone(), span: t.span }),
            Tok::Int(value) => Ok(Expr::Int { value: *value, span: t.span }),
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
