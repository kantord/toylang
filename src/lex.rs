use crate::ast::Span;
use crate::error::Error;

#[derive(Debug, PartialEq)]
pub enum Tok {
    Str(String),
    Int(i64),
    Ident(String),
    Fn,
    Plus,
    LParen,
    RParen,
    Colon,
    Arrow,
    Eq,
    Eof,
}

impl std::fmt::Display for Tok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tok::Str(_) => write!(f, "a string"),
            Tok::Int(n) => write!(f, "`{n}`"),
            Tok::Ident(name) => write!(f, "`{name}`"),
            Tok::Fn => write!(f, "`fn`"),
            Tok::Plus => write!(f, "`+`"),
            Tok::LParen => write!(f, "`(`"),
            Tok::RParen => write!(f, "`)`"),
            Tok::Colon => write!(f, "`:`"),
            Tok::Arrow => write!(f, "`->`"),
            Tok::Eq => write!(f, "`=`"),
            Tok::Eof => write!(f, "end of program"),
        }
    }
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
        let start = i;
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let tok = match bytes[i] {
            b'"' => {
                let (text, end) = string(src, i)?;
                i = end;
                Tok::Str(text)
            }
            b'0'..=b'9' => {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let digits = &src[start..i];
                let n = digits.parse::<i64>().map_err(|_| {
                    Error::new(Span::new(start, i), format!("integer `{digits}` is out of range"))
                })?;
                Tok::Int(n)
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                match &src[start..i] {
                    "fn" => Tok::Fn,
                    name => Tok::Ident(name.to_string()),
                }
            }
            b'-' => {
                // `-` only ever begins `->`; there is no subtraction and no unary minus yet.
                if bytes.get(i + 1) != Some(&b'>') {
                    return Err(Error::new(Span::new(start, i + 1), "expected `->`"));
                }
                i += 2;
                Tok::Arrow
            }
            b'+' => {
                i += 1;
                Tok::Plus
            }
            b'(' => {
                i += 1;
                Tok::LParen
            }
            b')' => {
                i += 1;
                Tok::RParen
            }
            b':' => {
                i += 1;
                Tok::Colon
            }
            b'=' => {
                i += 1;
                Tok::Eq
            }
            _ => {
                return Err(Error::new(
                    Span::new(start, start + 1),
                    format!("unexpected character `{}`", src[start..].chars().next().unwrap()),
                ));
            }
        };
        out.push(Token { tok, span: Span::new(start, i) });
    }

    out.push(Token { tok: Tok::Eof, span: Span::new(src.len(), src.len()) });
    Ok(out)
}

/// Returns the unescaped contents and the index just past the closing quote.
fn string(src: &str, start: usize) -> Result<(String, usize), Error> {
    let bytes = src.as_bytes();
    let mut i = start + 1;
    let mut text = String::new();
    loop {
        match bytes.get(i) {
            None => return Err(Error::new(Span::new(start, i), "unterminated string")),
            Some(b'"') => return Ok((text, i + 1)),
            // Escapes are JSON's set minus \u, which waits for a real value model.
            Some(b'\\') => {
                let esc = bytes
                    .get(i + 1)
                    .ok_or_else(|| Error::new(Span::new(i, i + 1), "unterminated escape sequence"))?;
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
                // Step by whole characters so multi-byte input is not split mid-character.
                let c = src[i..].chars().next().unwrap();
                text.push(c);
                i += c.len_utf8();
            }
        }
    }
}
