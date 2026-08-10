use crate::ast::{BinOp, Expr};
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

/// A program is one expression. Definitions and the `def* expr` file form arrive at step 3.
pub fn parse(tokens: &[Token]) -> Result<Expr, Error> {
    let mut p = Parser { tokens, pos: 0 };
    let expr = p.expr(0)?;
    let rest = p.peek();
    if rest.tok != Tok::Eof {
        return Err(Error::new(rest.span, format!("expected end of program, found {}", rest.tok)));
    }
    Ok(expr)
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
            other => Err(Error::new(t.span, format!("expected an expression, found {other}"))),
        }
    }
}
