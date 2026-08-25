use crate::ast::Span;
use crate::error::Error;

#[derive(Debug, PartialEq)]
pub enum Tok {
    Str(String),
    Int(i64),
    Ident(String),
    Fn,
    Type,
    Select,
    Map,
    If,
    Else,
    Input,
    Lines,
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
            Tok::Type => "`type`",
            Tok::Select => "`select`",
            Tok::Map => "`map`",
            Tok::If => "`if`",
            Tok::Else => "`else`",
            Tok::Input => "`input`",
            Tok::Lines => "`lines`",
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
        if bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
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
                    "type" => Tok::Type,
                    "select" => Tok::Select,
                    "map" => Tok::Map,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "input" => Tok::Input,
                    "lines" => Tok::Lines,
                    name => Tok::Ident(name.to_string()),
                }
            }
            b'-' => {
                // `-` is subtraction and negation now, so a digit after it is no longer a
                // negative literal: `a -1` would otherwise not be `a - 1`.
                if bytes.get(i + 1) == Some(&b'>') {
                    i += 2;
                    Tok::Arrow
                } else {
                    i += 1;
                    Tok::Minus
                }
            }
            b'!' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    i += 2;
                    Tok::Ne
                } else {
                    i += 1;
                    Tok::Bang
                }
            }
            b'=' | b'<' | b'>' => {
                let c = bytes[i];
                i += 1;
                if bytes.get(i) == Some(&b'=') {
                    i += 1;
                    match c {
                        b'=' => Tok::EqEq,
                        b'<' => Tok::Le,
                        _ => Tok::Ge,
                    }
                } else {
                    match c {
                        b'=' => Tok::Eq,
                        b'<' => Tok::Lt,
                        _ => Tok::Gt,
                    }
                }
            }
            _ => {
                let tok = match bytes[i] {
                    b'+' => Tok::Plus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'%' => Tok::Percent,
                    b'|' => Tok::Pipe,
                    b',' => Tok::Comma,
                    b'.' => Tok::Dot,
                    b'(' => Tok::LParen,
                    b')' => Tok::RParen,
                    b'[' => Tok::LBracket,
                    b']' => Tok::RBracket,
                    b'{' => Tok::LBrace,
                    b'}' => Tok::RBrace,
                    b':' => Tok::Colon,
                    _ => {
                        return Err(Error::new(
                            Span::new(start, start + 1),
                            format!(
                                "unexpected character `{}`",
                                src[start..].chars().next().unwrap()
                            ),
                        ));
                    }
                };
                i += 1;
                tok
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
