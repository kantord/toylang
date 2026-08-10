use crate::ast::Span;
use crate::error::Error;

#[derive(Debug, PartialEq)]
pub enum Tok {
    Str(String),
    Eof,
}

#[derive(Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] != b'"' {
            return Err(Error::new(
                Span::new(i, i + 1),
                format!("unexpected character `{}`", src[i..].chars().next().unwrap()),
            ));
        }

        let start = i;
        i += 1;
        let mut text = String::new();
        loop {
            match bytes.get(i) {
                None => return Err(Error::new(Span::new(start, i), "unterminated string")),
                Some(b'"') => {
                    i += 1;
                    break;
                }
                // Escapes are JSON's set minus \u, which waits for a real value model.
                Some(b'\\') => {
                    let esc = bytes.get(i + 1).ok_or_else(|| {
                        Error::new(Span::new(i, i + 1), "unterminated escape sequence")
                    })?;
                    text.push(match esc {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        _ => {
                            return Err(Error::new(
                                Span::new(i, i + 2),
                                format!("unknown escape sequence `\\{}`", *esc as char),
                            ));
                        }
                    });
                    i += 2;
                }
                Some(_) => {
                    // Step off the byte index onto a char boundary so multi-byte input
                    // does not split mid-character.
                    let c = src[i..].chars().next().unwrap();
                    text.push(c);
                    i += c.len_utf8();
                }
            }
        }
        out.push(Token { tok: Tok::Str(text), span: Span::new(start, i) });
    }

    out.push(Token { tok: Tok::Eof, span: Span::new(src.len(), src.len()) });
    Ok(out)
}
