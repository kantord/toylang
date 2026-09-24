//! The JSON reader: a value read off stdin, off a Str (`parse`) or off each line of stdin
//! (`inputs`), laid out as the slots the compiled code reads.
//!
//! What to look for is a type descriptor the compiler emits as one string (the private protocol
//! with src/emit_llvm.rs), so only the shape the program declared is accepted:
//!
//! ```text
//! s            Str
//! i            Int
//! f            Float
//! b            Bool
//! [T           Vec of T
//! {n,name:T,...}   record with n fields, in the type's declared order
//! e{n,Name,variant,variant:T,...}   enum with n variants, in declaration order (a variant's
//!              position is its tag); a variant with no `:T` is a unit variant. Name is what a
//!              mismatch says it expected, and what `@` names.
//! @Name        the nearest enclosing enum called Name, whose descriptor is still open around
//!              this one. A recursive enum's payload names itself back rather than spelling
//!              itself out again, which would have no end (kantord/toylang#94).
//! ```
//!
//! serde_json does the tokenizing (whitespace, string escapes and surrogate pairs, the number
//! grammar). This file supplies what it cannot: the descriptor, the slot layout, and the message
//! `toylang: input: <what> at <path>` naming where in the value the mismatch is. The reader
//! walks the input as it is tokenized rather than building a `serde_json::Value` first, so the
//! first mismatch in document order is the one reported and no second copy of the value exists.

use std::cell::RefCell;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Visitor};

use super::{TlVec, column_mut, slice_of, slice_of_mut, tl_rec_new, tl_vec_new, vec_of_slots};
use super::{leak_str, tl_rec_set};

/// A type the input declares, read off a descriptor.
#[derive(Debug)]
enum Ty {
    Str,
    Int,
    Float,
    Bool,
    Vec(Box<Ty>),
    Record(Vec<(String, Ty)>),
    /// An index into `Schema::enums`, which is what lets a recursive enum contain itself.
    Enum(usize),
}

#[derive(Debug)]
struct EnumDef {
    name: String,
    /// In declaration order; `None` marks a unit variant.
    variants: Vec<(String, Option<Ty>)>,
}

#[derive(Debug)]
pub struct Schema {
    root: Ty,
    enums: Vec<EnumDef>,
}

/// The descriptor text and a cursor over it. Every method answers `None` for a descriptor that
/// does not follow the grammar, which the compiler never emits; the caller turns that into
/// `bad type descriptor` rather than reading past the end.
struct Cursor<'a> {
    text: &'a [u8],
    at: usize,
    enums: Vec<EnumDef>,
    /// The enums whose descriptors are open around the type being read, outermost first. `@Name`
    /// resolves by walking this from the inside, so a name is always the nearest enclosing enum
    /// of that name, which is what makes two instantiations of one generic enum, nested inside
    /// each other, resolve to the right one.
    open: Vec<usize>,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn expect(&mut self, c: u8) -> Option<()> {
        (self.peek()? == c).then(|| self.at += 1)
    }

    /// The text up to (not including) the first of `stops`.
    fn until(&mut self, stops: &[u8]) -> Option<&'a str> {
        let rest = self.text.get(self.at..)?;
        let n = rest.iter().position(|b| stops.contains(b))?;
        self.at += n;
        std::str::from_utf8(&rest[..n]).ok()
    }

    /// A decimal count followed by the `,` after it.
    fn count(&mut self) -> Option<usize> {
        let n = self.until(b",")?.parse().ok()?;
        self.expect(b',')?;
        Some(n)
    }

    fn ty(&mut self) -> Option<Ty> {
        let c = self.peek()?;
        self.at += 1;
        Some(match c {
            b's' => Ty::Str,
            b'i' => Ty::Int,
            b'f' => Ty::Float,
            b'b' => Ty::Bool,
            b'[' => Ty::Vec(Box::new(self.ty()?)),
            b'@' => {
                let name = self.until(b",}")?;
                let id = self
                    .open
                    .iter()
                    .rev()
                    .find(|&&id| self.enums[id].name == name);
                Ty::Enum(*id?)
            }
            b'{' => {
                let n = self.count()?;
                let mut fields = Vec::new();
                for _ in 0..n {
                    let name = self.until(b":")?.to_owned();
                    self.expect(b':')?;
                    fields.push((name, self.ty()?));
                    let _ = self.expect(b',');
                }
                self.expect(b'}')?;
                Ty::Record(fields)
            }
            b'e' => {
                self.expect(b'{')?;
                let n = self.count()?;
                let name = self.until(b",}")?.to_owned();
                let id = self.enums.len();
                self.enums.push(EnumDef {
                    name,
                    variants: Vec::new(),
                });
                self.open.push(id);
                let mut variants = Vec::new();
                for _ in 0..n {
                    self.expect(b',')?;
                    let name = self.until(b":,}")?.to_owned();
                    let payload = match self.expect(b':') {
                        Some(()) => Some(self.ty()?),
                        None => None,
                    };
                    variants.push((name, payload));
                }
                self.open.pop();
                self.expect(b'}')?;
                self.enums[id].variants = variants;
                Ty::Enum(id)
            }
            _ => return None,
        })
    }
}

/// A refusal: what was wrong and where, the two halves of `toylang: input: <what> at <path>`.
#[derive(Debug, PartialEq)]
pub struct Failure {
    pub what: String,
    pub path: String,
}

/// One step into the value, for the path a message names.
#[derive(Clone, Copy)]
enum Seg<'a> {
    Field(&'a str),
    Index(usize),
}

/// Reads documents against one schema. `root` names the source in messages: `input` for stdin,
/// `parse` for a `parse(s)` call, `inputs` for a line of a stream.
pub struct Reader<'a> {
    schema: &'a Schema,
    root: &'a str,
    /// Where the value being read sits. A segment is popped when its value has been read, not
    /// when it fails, so after an error this is the path of the failure.
    path: RefCell<Vec<Seg<'a>>>,
    /// Set by the first mismatch. serde has no way to carry a structured error out of a visitor,
    /// so the visitor stores it here and returns a placeholder `Error::custom`.
    failure: RefCell<Option<Failure>>,
}

impl<'a> Reader<'a> {
    pub fn new(schema: &'a Schema, root: &'a str) -> Self {
        Reader {
            schema,
            root,
            path: RefCell::new(Vec::new()),
            failure: RefCell::new(None),
        }
    }

    fn path_here(&self) -> String {
        let mut path = self.root.to_owned();
        for seg in self.path.borrow().iter() {
            match seg {
                Seg::Field(name) => path.extend([".", name]),
                Seg::Index(i) => path.push_str(&format!("[{i}]")),
            }
        }
        path
    }

    /// Records a mismatch at the current path.
    fn refuse<E: Error>(&self, what: impl Into<String>) -> E {
        self.failure.borrow_mut().get_or_insert(Failure {
            what: what.into(),
            path: self.path_here(),
        });
        E::custom("input does not match the declared type")
    }

    /// One JSON value, and nothing but whitespace after it. `text` is one document: all of stdin,
    /// the string handed to `parse`, or one line of a stream.
    pub fn document(&self, text: &[u8]) -> Result<i64, Failure> {
        // serde_json checks UTF-8 only in the strings it reads, so raw bytes inside a value the
        // program did not declare would otherwise pass. A Str is Unicode scalar values.
        if std::str::from_utf8(text).is_err() {
            return Err(Failure {
                what: "input is not valid UTF-8".into(),
                path: self.root.into(),
            });
        }
        self.path.borrow_mut().clear();
        *self.failure.borrow_mut() = None;
        let mut de = serde_json::Deserializer::from_slice(text);
        let seed = Value {
            reader: self,
            ty: &self.schema.root,
            seg: None,
        };
        let slot = match seed.deserialize(&mut de) {
            Ok(slot) => slot,
            Err(e) => return Err(self.failure_of(&e)),
        };
        match de.end() {
            Ok(()) => Ok(slot),
            Err(_) => Err(Failure {
                what: "trailing content after the value".into(),
                path: self.root.into(),
            }),
        }
    }

    /// The refusal a serde error stands for: the mismatch a visitor recorded, or else a syntax
    /// error serde_json found itself, which names the line and column it read to.
    fn failure_of(&self, e: &serde_json::Error) -> Failure {
        if let Some(recorded) = self.failure.borrow_mut().take() {
            return recorded;
        }
        let what = if e.is_eof() {
            "unexpected end of input".to_owned()
        } else {
            let text = e.to_string();
            let msg = text.rsplit_once(" at line ").map_or(&*text, |(msg, _)| msg);
            // What serde_json calls half of a surrogate pair, in the words the C parser used and
            // tests/corpus/unpaired_surrogate_input.yaml is about.
            let msg = match msg {
                "unexpected end of hex escape" | "lone leading surrogate in hex escape" => {
                    "unpaired surrogate"
                }
                msg => msg,
            };
            format!("{msg} (line {} column {})", e.line(), e.column())
        };
        Failure {
            what,
            path: self.path_here(),
        }
    }
}

/// The value at `ty`: a seed that reads one JSON value, and the visitor that turns it into a slot.
#[derive(Clone, Copy)]
struct Value<'r, 'a> {
    reader: &'r Reader<'a>,
    ty: &'a Ty,
    /// How the path reaches this value, pushed for as long as it is being read.
    seg: Option<Seg<'a>>,
}

impl<'r, 'a> Value<'r, 'a> {
    fn child(&self, ty: &'a Ty, seg: Seg<'a>) -> Value<'r, 'a> {
        Value {
            reader: self.reader,
            ty,
            seg: Some(seg),
        }
    }

    fn enum_def(&self, id: usize) -> &'a EnumDef {
        &self.reader.schema.enums[id]
    }

    /// The refusal for a JSON value of the wrong kind: what the declared type is.
    fn mismatch<E: Error>(&self) -> E {
        let what = match self.ty {
            Ty::Str => "expected a string".to_owned(),
            Ty::Int => "expected an integer".to_owned(),
            Ty::Float => "expected a number".to_owned(),
            Ty::Bool => "expected a boolean".to_owned(),
            Ty::Vec(_) => "expected an array".to_owned(),
            Ty::Record(_) => "expected an object".to_owned(),
            Ty::Enum(id) => format!("expected {}", self.enum_def(*id).name),
        };
        self.reader.refuse(what)
    }

    /// An Int is 32 bits wide, the rule `input::validate` in the compiler enforces for every other
    /// backend; the C parser this replaced took anything up to 31 characters.
    fn int<E: Error>(&self, v: i128) -> Result<i64, E> {
        match i32::try_from(v) {
            Ok(v) => Ok(i64::from(v)),
            Err(_) => Err(self.reader.refuse("integer is out of range")),
        }
    }

    /// The two-slot box the compiler builds for a constructed enum: slot 0 the variant's
    /// declaration index, slot 1 its payload (zero for a unit variant).
    fn enum_box(tag: usize, payload: i64) -> i64 {
        let b = tl_rec_new(2);
        unsafe {
            tl_rec_set(b, 0, tag as i64);
            tl_rec_set(b, 1, payload);
        }
        b as i64
    }
}

impl<'de, 'r, 'a> DeserializeSeed<'de> for Value<'r, 'a> {
    type Value = i64;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<i64, D::Error> {
        if let Some(seg) = self.seg {
            self.reader.path.borrow_mut().push(seg);
        }
        let slot = d.deserialize_any(self)?;
        if self.seg.is_some() {
            self.reader.path.borrow_mut().pop();
        }
        Ok(slot)
    }
}

impl<'de, 'r, 'a> Visitor<'de> for Value<'r, 'a> {
    type Value = i64;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a value of the declared type")
    }

    fn visit_bool<E: Error>(self, v: bool) -> Result<i64, E> {
        match self.ty {
            Ty::Bool => Ok(i64::from(v)),
            _ => Err(self.mismatch()),
        }
    }

    fn visit_i64<E: Error>(self, v: i64) -> Result<i64, E> {
        match self.ty {
            Ty::Int => self.int(v.into()),
            Ty::Float => Ok((v as f64).to_bits() as i64),
            _ => Err(self.mismatch()),
        }
    }

    fn visit_u64<E: Error>(self, v: u64) -> Result<i64, E> {
        match self.ty {
            Ty::Int => self.int(v.into()),
            Ty::Float => Ok((v as f64).to_bits() as i64),
            _ => Err(self.mismatch()),
        }
    }

    /// A float where Int was declared is an error, not a truncation. Every JSON number is a legal
    /// Float, integer or not (ADR 0007), and comes back as the double's bit pattern in the slot.
    fn visit_f64<E: Error>(self, v: f64) -> Result<i64, E> {
        match self.ty {
            Ty::Float => Ok(v.to_bits() as i64),
            Ty::Int => Err(self
                .reader
                .refuse("expected an integer, found a non-integer number")),
            _ => Err(self.mismatch()),
        }
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<i64, E> {
        match self.ty {
            Ty::Str => Ok(leak_str(v.as_bytes().to_vec()) as i64),
            Ty::Enum(id) => {
                let def = self.enum_def(*id);
                let unit = def.variants.iter().position(|(n, p)| p.is_none() && n == v);
                match unit {
                    Some(tag) => Ok(Self::enum_box(tag, 0)),
                    None => Err(self
                        .reader
                        .refuse(format!("`{v}` is not a unit variant of {}", def.name))),
                }
            }
            _ => Err(self.mismatch()),
        }
    }

    fn visit_unit<E: Error>(self) -> Result<i64, E> {
        Err(self.mismatch())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<i64, A::Error> {
        let Ty::Vec(elem) = self.ty else {
            return Err(self.mismatch());
        };
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1 << 16));
        while let Some(item) = seq.next_element_seed(self.child(elem, Seg::Index(items.len())))? {
            items.push(item);
        }
        let ncols = match &**elem {
            Ty::Record(fields) => Some(fields.len()),
            _ => None,
        };
        Ok(vec_of_items(&items, ncols))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<i64, A::Error> {
        match self.ty {
            Ty::Record(fields) => {
                let mut slots = vec![0i64; fields.len()];
                let mut seen = vec![false; fields.len()];
                while let Some(key) = map.next_key_seed(FieldKey(fields))? {
                    let Some(f) = key else {
                        // A field the program did not declare is ignored, so a program can read
                        // two fields out of a log line without describing the whole line.
                        map.next_value_seed(Skip)?;
                        continue;
                    };
                    let (name, ty) = &fields[f];
                    slots[f] = map.next_value_seed(self.child(ty, Seg::Field(name)))?;
                    seen[f] = true;
                }
                if let Some(f) = seen.iter().position(|&s| !s) {
                    let name = &fields[f].0;
                    return Err(self.reader.refuse(format!("missing field `{name}`")));
                }
                let rec = tl_rec_new(slots.len() as i64);
                unsafe { slice_of_mut(rec, slots.len() as i64) }.copy_from_slice(&slots);
                Ok(rec as i64)
            }
            // One enum value spans two JSON shapes (ADR 0009): a bare string for a unit variant,
            // a single-key object for a payload one.
            Ty::Enum(id) => {
                let def = self.enum_def(*id);
                let Some((tag, payload_ty, name)) = map.next_key_seed(PayloadKey(&self, def))?
                else {
                    return Err(self.mismatch());
                };
                let payload = map.next_value_seed(self.child(payload_ty, Seg::Field(name)))?;
                // One key is the whole shape, so the wrapper closes right here.
                if map.next_key_seed(FieldKey(&[]))?.is_some() {
                    return Err(self.reader.refuse("expected `}`"));
                }
                Ok(Self::enum_box(tag, payload))
            }
            _ => Err(self.mismatch()),
        }
    }
}

/// A value the program did not declare, read and dropped. Not serde's `IgnoredAny`: that skips a
/// string's contents without checking a `\ud800` escape is half a pair, and a number without
/// checking it fits a double, where every value the host reads is refused for either. A document
/// is refused for what it says anywhere in it, declared or not.
struct Skip;

impl<'de> DeserializeSeed<'de> for Skip {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(Skip)
    }
}

impl<'de> Visitor<'de> for Skip {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(Skip)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map.next_key_seed(FieldKey(&[]))?.is_some() {
            map.next_value_seed(Skip)?;
        }
        Ok(())
    }
}

/// The key of a record's field: its index in the descriptor, or `None` for a field it does not
/// declare.
struct FieldKey<'a>(&'a [(String, Ty)]);

impl<'de> DeserializeSeed<'de> for FieldKey<'_> {
    type Value = Option<usize>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Option<usize>, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for FieldKey<'_> {
    type Value = Option<usize>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a field name")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Option<usize>, E> {
        Ok(self.0.iter().position(|(name, _)| name == v))
    }
}

/// The key of an enum's single-key object: the payload variant it names, as its tag, payload type
/// and name. A key that names a unit variant or nothing at all is refused.
struct PayloadKey<'r, 'a>(&'r Value<'r, 'a>, &'a EnumDef);

impl<'de, 'a> DeserializeSeed<'de> for PayloadKey<'_, 'a> {
    type Value = (usize, &'a Ty, &'a str);

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de, 'a> Visitor<'de> for PayloadKey<'_, 'a> {
    type Value = (usize, &'a Ty, &'a str);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a variant name")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let payload_variant = self
            .1
            .variants
            .iter()
            .enumerate()
            .find_map(|(tag, (n, p))| {
                let payload = p.as_ref().filter(|_| n == v)?;
                Some((tag, payload, n.as_str()))
            });
        payload_variant.ok_or_else(|| {
            self.0
                .reader
                .refuse(format!("`{v}` is not a payload variant of {}", self.1.name))
        })
    }
}

/// A Vec of the values read: one column of the items themselves, or, for a Vec of records, one
/// column per field (`ncols`), gathered out of the record blobs the items are. Filling the columns
/// directly would avoid materialising each record.
fn vec_of_items(items: &[i64], ncols: Option<usize>) -> i64 {
    let Some(ncols) = ncols else {
        return vec_of_slots(items) as i64;
    };
    let v = tl_vec_new(items.len() as i64, ncols as i64);
    for c in 0..ncols {
        let column = unsafe { column_mut(v, c as i64) };
        for (dst, &rec) in column.iter_mut().zip(items) {
            *dst = unsafe { slice_of(rec as *const i64, ncols as i64) }[c];
        }
    }
    v as i64
}

impl Schema {
    /// `root` names the source in the refusal for a descriptor that does not follow the grammar.
    pub fn parse(descriptor: &[u8], root: &str) -> Result<Schema, Failure> {
        let bad = || Failure {
            what: "bad type descriptor".into(),
            path: root.into(),
        };
        let mut cursor = Cursor {
            text: descriptor,
            at: 0,
            enums: Vec::new(),
            open: Vec::new(),
        };
        let root = cursor.ty().ok_or_else(bad)?;
        Ok(Schema {
            root,
            enums: cursor.enums,
        })
    }

    /// The Vec of the values (`document`'s slots) a stream of this type makes.
    pub fn vec_of(&self, items: &[i64]) -> *mut TlVec {
        let ncols = match &self.root {
            Ty::Record(fields) => Some(fields.len()),
            _ => None,
        };
        vec_of_items(items, ncols) as *mut TlVec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TlStr, bytes, column, tl_rec_get, tl_vec_len};

    fn read_at(root: &str, desc: &str, text: &[u8]) -> Result<i64, Failure> {
        let schema = Schema::parse(desc.as_bytes(), root)?;
        Reader::new(&schema, root).document(text)
    }

    fn read(desc: &str, text: &str) -> Result<i64, Failure> {
        read_at("input", desc, text.as_bytes())
    }

    /// What was refused and where, for an input that must be.
    fn refused(desc: &str, text: &str) -> (String, String) {
        match read(desc, text) {
            Ok(_) => panic!("{text} should have been refused as {desc}"),
            Err(f) => (f.what, f.path),
        }
    }

    fn refusal(what: &str, path: &str) -> (String, String) {
        (what.to_owned(), path.to_owned())
    }

    fn slot(desc: &str, text: &str) -> i64 {
        read(desc, text).unwrap_or_else(|f| panic!("{text}: {} at {}", f.what, f.path))
    }

    unsafe fn text_of(slot: i64) -> String {
        String::from_utf8(unsafe { bytes(slot as *const TlStr) }.to_vec()).unwrap()
    }

    unsafe fn ints_of(vec: i64) -> Vec<i64> {
        unsafe { column(vec as *const TlVec, 0) }.to_vec()
    }

    #[test]
    fn scalars_read_as_their_slots() {
        unsafe {
            assert_eq!(slot("i", " 42\n"), 42);
            assert_eq!(slot("i", "-7"), -7);
            assert_eq!(slot("b", "true"), 1);
            assert_eq!(slot("b", "false"), 0);
            assert_eq!(text_of(slot("s", "\"h\u{e9}llo\"")), "h\u{e9}llo");
            assert_eq!(f64::from_bits(slot("f", "2.5") as u64), 2.5);
            assert_eq!(
                f64::from_bits(slot("f", "3") as u64),
                3.0,
                "an integer is a legal Float"
            );
        }
        assert_eq!(
            refused("i", "\"3\""),
            refusal("expected an integer", "input"),
            "a string is not coerced"
        );
        assert_eq!(refused("b", "0"), refusal("expected a boolean", "input"));
        assert_eq!(refused("s", "7"), refusal("expected a string", "input"));
        assert_eq!(refused("f", "\"1\""), refusal("expected a number", "input"));
        assert_eq!(refused("b", "null"), refusal("expected a boolean", "input"));
    }

    #[test]
    fn an_int_is_32_bits_and_never_a_float() {
        assert_eq!(slot("i", "2147483647"), i64::from(i32::MAX));
        assert_eq!(slot("i", "-2147483648"), i64::from(i32::MIN));
        assert!(
            read("i", "007").is_err() && read("i", "+7").is_err(),
            "not JSON, though C read both"
        );
        let out_of_range = refusal("integer is out of range", "input");
        assert_eq!(refused("i", "2147483648"), out_of_range);
        assert_eq!(refused("i", "-2147483649"), out_of_range);
        assert_eq!(refused("i", "9223372036854775807"), out_of_range);
        assert_eq!(refused("i", "18446744073709551615"), out_of_range);
        let non_integer = refusal("expected an integer, found a non-integer number", "input");
        assert_eq!(refused("i", "1.5"), non_integer);
        assert_eq!(
            refused("i", "3.0"),
            non_integer,
            "not truncated, even when whole"
        );
        assert_eq!(refused("i", "1e2"), non_integer);
        // The Int, not the Float, decides: past what a u64 holds serde_json reads a double.
        assert_eq!(refused("i", "123456789012345678901234567890"), non_integer);
    }

    #[test]
    fn floats_follow_the_json_number_grammar_not_strtod() {
        for ok in ["0", "-0", "1", "1E5", "1e+5", "0.5", "-1.5e-3", "  7  "] {
            assert!(read("f", ok).is_ok(), "{ok}");
        }
        for bad in [
            "+1", "1.", ".5", "-.5", "01", "1e", "--1", "0x10", "inf", "-inf", "NaN", "Infinity",
            "-", "1_000",
        ] {
            assert!(read("f", bad).is_err(), "{bad} is not a JSON number");
        }
        let negative_zero = f64::from_bits(slot("f", "-0") as u64);
        assert!(negative_zero == 0.0 && negative_zero.is_sign_negative());
        assert_eq!(
            refused("f", "1e999").0,
            "number out of range (line 1 column 5)",
            "a double that overflows is refused, not read as infinity"
        );
    }

    /// serde_json's default float path returns a different double from the correctly rounded one
    /// for long digit strings: 14.8% of a sample weighted toward them, 0 with `float_roundtrip`.
    /// If the feature is dropped from Cargo.toml this fails on the first two.
    #[test]
    fn long_digit_floats_parse_correctly_rounded() {
        let mut texts = vec![
            "8525388853933633.0".to_owned(),
            "91186252760.18955".to_owned(),
        ];
        // A fixed pseudo-random sample of 1 to 25 significant digits with a fraction and an
        // optional exponent, the shape the measurement used.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20_000 {
            let digits: String = (0..1 + next() % 25)
                .map(|_| char::from(b'0' + (next() % 10) as u8))
                .collect();
            let digits = digits.trim_start_matches('0');
            let digits = if digits.is_empty() { "1" } else { digits };
            let split = (next() as usize) % (digits.len() + 1);
            let (int, frac) = digits.split_at(split);
            let int = if int.is_empty() { "0" } else { int };
            let frac = if frac.is_empty() { "0" } else { frac };
            let exp = match next() % 3 {
                0 => format!("e{}", next() % 40),
                1 => format!("e-{}", next() % 40),
                _ => String::new(),
            };
            texts.push(format!("{int}.{frac}{exp}"));
        }
        for text in &texts {
            let want = text.parse::<f64>().unwrap().to_bits();
            assert_eq!(slot("f", text) as u64, want, "{text}");
        }
    }

    #[test]
    fn strings_decode_escapes_and_refuse_what_json_refuses() {
        unsafe {
            assert_eq!(
                text_of(slot("s", r#""a\"b\\c\/d\n\t\r\b\f""#)),
                "a\"b\\c/d\n\t\r\u{8}\u{c}"
            );
            assert_eq!(text_of(slot("s", r#""é€A""#)), "\u{e9}\u{20ac}A");
            assert_eq!(
                text_of(slot("s", r#""😀""#)),
                "\u{1f600}",
                "a surrogate pair"
            );
            assert_eq!(text_of(slot("s", r#""\u0000""#)), "\0");
            assert_eq!(
                text_of(slot("s", "\"\u{1f600}\"")),
                "\u{1f600}",
                "raw UTF-8"
            );
        }
        for lone in [
            r#""\ud800""#,
            r#""\udc00""#,
            r#""\ud800\u0041""#,
            r#""\ud800x""#,
        ] {
            let what = refused("s", lone).0;
            assert!(what.starts_with("unpaired surrogate"), "{lone}: {what}");
        }
        for bad in [
            r#""\ud800""#,
            r#""\udc00""#,
            r#""\ud800A""#,
            r#""\ud800x""#,
            r#""\u12""#,
            r#""\u12g4""#,
            r#""\x""#,
            "\"tab\there\"",
            "\"line\nbreak\"",
            "\"open",
        ] {
            assert!(read("s", bad).is_err(), "{bad:?}");
        }
        assert!(
            read_at("input", "s", b"\"\xff\"").is_err(),
            "bytes that are not UTF-8 never become a Str"
        );
        let f = read_at("input", "{1,a:i}", b"{\"z\": \"\xff\", \"a\": 1}").unwrap_err();
        assert_eq!(
            f.what, "input is not valid UTF-8",
            "not even in a field nobody reads"
        );
        assert!(
            read_at("input", "s", b"\"\xed\xa0\x80\"").is_err(),
            "a raw surrogate"
        );
    }

    const USERS: &str = "{1,users:[{2,name:s,age:i}}";

    #[test]
    fn records_take_fields_in_any_order_and_name_the_path_of_a_mismatch() {
        unsafe {
            let user = slot("{2,name:s,age:i}", r#"{"age": 36, "name": "ada"}"#) as *const i64;
            assert_eq!(text_of(tl_rec_get(user, 0)), "ada");
            assert_eq!(tl_rec_get(user, 1), 36);

            let db = slot(
                USERS,
                r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}]}"#,
            );
            let users = tl_rec_get(db as *const i64, 0) as *const TlVec;
            assert_eq!(
                (tl_vec_len(users), (*users).ncols),
                (2, 2),
                "a column per field"
            );
            assert_eq!(column(users, 1), [36, 9]);
            assert_eq!(text_of(column(users, 0)[1]), "bo");
        }
        assert_eq!(
            refused(
                USERS,
                r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": "9"}]}"#
            ),
            refusal("expected an integer", "input.users[1].age")
        );
        assert_eq!(
            refused(USERS, r#"{"users": [{"name": "ada"}]}"#),
            refusal("missing field `age`", "input.users[0]")
        );
        assert_eq!(
            refused(USERS, "{}"),
            refusal("missing field `users`", "input")
        );
        assert_eq!(refused(USERS, "[]"), refusal("expected an object", "input"));
        assert_eq!(
            refused(USERS, r#"{"users": {}}"#),
            refusal("expected an array", "input.users")
        );
    }

    #[test]
    fn duplicate_missing_and_extra_record_fields() {
        unsafe {
            let rec = slot("{1,a:i}", r#"{"a": 1, "a": 2}"#) as *const i64;
            assert_eq!(tl_rec_get(rec, 0), 2, "the last of a duplicated field wins");
            let rec = slot("{1,a:i}", r#"{"z": [1, {"x": "}"}], "a": 5, "y": null}"#) as *const i64;
            assert_eq!(
                tl_rec_get(rec, 0),
                5,
                "undeclared fields are skipped, whatever they hold"
            );
        }
        assert_eq!(
            refused("{1,a:i}", r#"{"a": 1, "a": "x"}"#),
            refusal("expected an integer", "input.a"),
            "every occurrence is read, not just the last"
        );
        // The value of a skipped field is still JSON.
        assert!(read("{1,a:i}", r#"{"z": [1, 2, "a": 1}"#).is_err());
        assert!(read("{1,a:i}", r#"{"a": 1, "z": nul}"#).is_err());
        // Nor is what it says let through for being in a field nobody reads.
        assert!(read("{1,a:i}", r#"{"a": 1, "z": ["\ud800"]}"#).is_err());
        assert!(read("{1,a:i}", r#"{"a": 1, "z": {"k": 1e999}}"#).is_err());
        assert!(read("{1,a:i}", r#"{"a": 1, "\udc00": 1}"#).is_err());
        assert!(read("{0,}", "{}").is_ok(), "a record with no fields");
        assert!(read("{0,}", r#"{"a": 1}"#).is_ok());
    }

    /// `Vec<Shape>` where `Shape = Circle | Rect{w, h} | Empty`, spelled the way the compiler does.
    const SHAPE: &str = "e{3,Shape,Empty,Circle:i,Rect:{2,w:i,h:i}}";

    #[test]
    fn enums_are_a_bare_string_or_a_single_key_object() {
        unsafe {
            let unit = slot(SHAPE, "\"Empty\"") as *const i64;
            assert_eq!(tl_rec_get(unit, 0), 0);
            let circle = slot(SHAPE, r#"{"Circle": 5}"#) as *const i64;
            assert_eq!((tl_rec_get(circle, 0), tl_rec_get(circle, 1)), (1, 5));
            let rect = slot(SHAPE, r#"{"Rect": {"h": 2, "w": 3}}"#) as *const i64;
            assert_eq!(tl_rec_get(rect, 0), 2);
            let payload = tl_rec_get(rect, 1) as *const i64;
            assert_eq!((tl_rec_get(payload, 0), tl_rec_get(payload, 1)), (3, 2));
        }
        assert_eq!(
            refused(SHAPE, "\"Circle\""),
            refusal("`Circle` is not a unit variant of Shape", "input")
        );
        assert_eq!(
            refused(SHAPE, "\"Nope\""),
            refusal("`Nope` is not a unit variant of Shape", "input")
        );
        assert_eq!(
            refused(SHAPE, r#"{"Empty": 1}"#),
            refusal("`Empty` is not a payload variant of Shape", "input")
        );
        assert_eq!(
            refused(SHAPE, r#"{"Circle": "x"}"#),
            refusal("expected an integer", "input.Circle")
        );
        assert_eq!(
            refused(SHAPE, r#"{"Circle": 1, "Empty": 0}"#),
            refusal("expected `}`", "input"),
            "one key is the whole shape"
        );
        assert_eq!(refused(SHAPE, "{}"), refusal("expected Shape", "input"));
        assert_eq!(refused(SHAPE, "3"), refusal("expected Shape", "input"));
        assert_eq!(refused(SHAPE, "[]"), refusal("expected Shape", "input"));
    }

    #[test]
    fn a_recursive_enum_names_itself_back() {
        const TREE: &str = "e{2,Tree,Leaf,Node:[@Tree}";
        unsafe {
            let tree = slot(
                TREE,
                r#"{"Node": [{"Node": []}, "Leaf", {"Node": ["Leaf"]}]}"#,
            );
            let node = tree as *const i64;
            assert_eq!(tl_rec_get(node, 0), 1);
            let children = tl_rec_get(node, 1) as *const TlVec;
            assert_eq!(tl_vec_len(children), 3);
        }
        assert_eq!(
            refused(TREE, r#"{"Node": [{"Node": [7]}]}"#),
            refusal("expected Tree", "input.Node[0].Node[0]")
        );
        // `@Name` is the nearest enclosing enum of that name: the inner Tree here shadows the outer.
        let nested = "e{1,Tree,Wrap:e{2,Tree,Leaf,Again:@Tree}}";
        assert!(read(nested, r#"{"Wrap": {"Again": {"Again": "Leaf"}}}"#).is_ok());
        assert_eq!(
            refused(nested, r#"{"Wrap": {"Again": {"Wrap": 1}}}"#),
            refusal(
                "`Wrap` is not a payload variant of Tree",
                "input.Wrap.Again"
            )
        );
    }

    #[test]
    fn vecs_of_scalars_of_records_and_of_vecs() {
        unsafe {
            assert_eq!(ints_of(slot("[i", "[1, 2, 3]")), [1, 2, 3]);
            assert_eq!(tl_vec_len(slot("[i", " [ ] ") as *const TlVec), 0);
            let rows = slot("[[i", "[[1], [], [2, 3]]") as *const TlVec;
            let inner: Vec<Vec<i64>> = column(rows, 0).iter().map(|&v| ints_of(v)).collect();
            assert_eq!(inner, [vec![1], vec![], vec![2, 3]]);
            // A record with one field is still a record: one column, but gathered out of blobs.
            let ones = slot("[{1,a:i}", r#"[{"a": 4}, {"a": 5}]"#) as *const TlVec;
            assert_eq!((tl_vec_len(ones), (*ones).ncols), (2, 1));
            assert_eq!(column(ones, 0), [4, 5]);
            let none = slot("[{2,a:i,b:i}", "[]") as *const TlVec;
            assert_eq!((tl_vec_len(none), (*none).ncols), (0, 2));
        }
        assert_eq!(
            refused("[i", "[1, 2"),
            refusal("unexpected end of input", "input")
        );
        assert_eq!(
            refused("[i", "[1, x]"),
            refusal("expected value (line 1 column 5)", "input[1]")
        );
        assert_eq!(refused("[i", "[1, 2,]").1, "input");
        assert_eq!(refused("[i", "[1 2]").1, "input");
        assert_eq!(
            refused("[i", "[1, \"a\"]"),
            refusal("expected an integer", "input[1]")
        );
    }

    #[test]
    fn a_document_is_one_value_and_whitespace() {
        assert_eq!(
            refused("i", ""),
            refusal("unexpected end of input", "input")
        );
        assert_eq!(
            refused("i", " \n\t\r "),
            refusal("unexpected end of input", "input")
        );
        assert_eq!(
            refused("i", "1 2"),
            refusal("trailing content after the value", "input")
        );
        assert_eq!(
            refused("i", "1x"),
            refusal("trailing content after the value", "input")
        );
        assert_eq!(
            refused("[i", "[1]]"),
            refusal("trailing content after the value", "input")
        );
        assert_eq!(slot("i", "\r\n 1 \r\n\r\n"), 1);
        // 0x0c (form feed) and a non-breaking space are not JSON whitespace.
        assert!(read("i", "\u{c}1").is_err() && read("i", "\u{a0}1").is_err());
        // The source's name is what a message opens its path with.
        let f = read_at("parse", "[i", b"[1, true]").unwrap_err();
        assert_eq!(
            (f.what.as_str(), f.path.as_str()),
            ("expected an integer", "parse[1]")
        );
        let f = read_at("inputs", "i", b"1 2").unwrap_err();
        assert_eq!(f.path, "inputs");
    }

    #[test]
    fn a_descriptor_that_breaks_the_grammar_is_refused_not_read_past() {
        for bad in [
            "",
            "x",
            "[",
            "{",
            "{2,a:i}",
            "{x,a:i}",
            "e{1,E}",
            "@Nope",
            "e{1,E,A:@Other}",
        ] {
            let f = Schema::parse(bad.as_bytes(), "input").unwrap_err();
            assert_eq!(
                (f.what.as_str(), f.path.as_str()),
                ("bad type descriptor", "input"),
                "{bad}"
            );
        }
    }

    #[test]
    fn deep_and_wide_inputs_do_not_overflow_the_stack() {
        let deep = format!("{}{}", "[".repeat(300), "]".repeat(300));
        let desc = format!("{}i", "[".repeat(300));
        assert!(
            read(&desc, &deep).is_err(),
            "serde_json's recursion limit refuses it"
        );
        let wide = format!("[{}0]", "0,".repeat(200_000));
        assert_eq!(unsafe { ints_of(slot("[i", &wide)) }.len(), 200_001);
    }
}
