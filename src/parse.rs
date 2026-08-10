use crate::ast::Expr;
use crate::error::Error;
use crate::lex::{Tok, Token};

/// A program is one expression. Definitions and the `def* expr` file form arrive at step 3.
pub fn parse(tokens: &[Token]) -> Result<Expr, Error> {
    let first = &tokens[0];
    let expr = match &first.tok {
        Tok::Str(text) => Expr::Str { text: text.clone(), span: first.span },
        Tok::Eof => return Err(Error::new(first.span, "empty program")),
    };

    let next = &tokens[1];
    if next.tok != Tok::Eof {
        return Err(Error::new(next.span, "expected end of program"));
    }
    Ok(expr)
}
