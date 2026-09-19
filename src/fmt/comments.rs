//! Comment placement. Comments are the one input that lives beside the tree rather than in it:
//! `parse` records every `#` line with its span (`ast::Comment`), and `Comments` hands them out
//! in source order as the file template walks the line-owning items -- declarations, `let`
//! bindings, a `let` block's value, the program body. A comment on its own line goes above the
//! next such item; a comment trailing code stays at the end of the line that item lands on; an
//! own-line comment inside an expression rises to the top of the item holding it, since a
//! re-rendered expression has no line for it to stay on. The one piece of author spacing kept is
//! the blank line after a comment, which is what tells a file banner from a doc comment. The
//! text itself is normalised by `comment_text`.

use super::multi_line::pad;
use crate::ast::Comment;

/// The file's comments, handed out in source order as the printer walks the line-owning items.
/// The placement rules are in the module doc.
pub(super) struct Comments<'a> {
    list: &'a [Comment],
    next: usize,
}

impl<'a> Comments<'a> {
    pub(super) fn new(list: &'a [Comment]) -> Self {
        Comments { list, next: 0 }
    }

    /// Every comment not yet taken that starts before `limit`.
    pub(super) fn take_before(&mut self, limit: usize) -> Vec<&'a Comment> {
        let mut out = Vec::new();
        while let Some(c) = self.list.get(self.next)
            && c.span.start < limit
        {
            out.push(c);
            self.next += 1;
        }
        out
    }

    /// The comment on the line the last printed item ended on, if there is one.
    pub(super) fn take_trailing(&mut self) -> Option<&'a Comment> {
        let c = self.list.get(self.next)?;
        if c.own_line {
            return None;
        }
        self.next += 1;
        Some(c)
    }

    pub(super) fn take_rest(&mut self) -> Vec<&'a Comment> {
        self.take_before(usize::MAX)
    }
}

/// Own-line comments, one per line at `indent`, keeping the blank line after any that had one.
pub(super) fn comment_lines(comments: &[&Comment], indent: usize) -> String {
    let mut out = String::new();
    for c in comments {
        out.push_str(&pad(indent));
        out.push_str(&comment_text(c));
        out.push('\n');
        if c.blank_after {
            out.push('\n');
        }
    }
    out
}

/// `rendered` with `comment`, if any, at the end of its last line.
pub(super) fn with_trailing(mut rendered: String, comment: Option<&Comment>) -> String {
    if let Some(c) = comment {
        rendered.push(' ');
        rendered.push_str(&comment_text(c));
    }
    rendered
}

/// `# text`: one space after the `#` (added when the author wrote none), the text's own
/// further indentation kept, since an indented line inside a comment is usually deliberate
/// (a list, a code sample). The parser already drops trailing whitespace. A bare `#` stays
/// bare.
fn comment_text(c: &Comment) -> String {
    if c.text.is_empty() || c.text.starts_with(' ') {
        format!("#{}", c.text)
    } else {
        format!("# {}", c.text)
    }
}
