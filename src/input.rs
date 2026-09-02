use mlua::Lua;
use serde_json::Value;

use crate::ty::{self, Enums, Type};

/// Check the input against the type the program declared, before anything runs.
///
/// This is a parse, not a coercion. `{"age": "36"}` where `Int` was declared is an error, not a
/// conversion: the moment input is coerced, the type stops describing the value and the runtime
/// and the type system have stopped making the same guarantee.
pub fn validate(enums: &Enums, value: &Value, ty: &Type, path: &str) -> Result<(), String> {
    let found = match (ty, value) {
        (Type::Str, Value::String(_)) | (Type::Bool, Value::Bool(_)) => return Ok(()),

        (Type::Int, Value::Number(n)) => {
            // Input is the other place an Int enters, and the 32-bit rule has to hold at both.
            // Accepting an i64 here left five backends carrying a value the type cannot hold
            // while Go refused to decode it, which is a disagreement rather than a wrong answer.
            let Some(n) = n.as_i64() else {
                return Err(format!(
                    "{path}: expected Int, found the non-integer number {n}"
                ));
            };
            return if i32::try_from(n).is_ok() {
                Ok(())
            } else {
                Err(format!(
                    "{path}: expected Int, found {n}, which does not fit in 32 bits"
                ))
            };
        }

        (Type::Float, Value::Number(_)) => {
            // A Float is the double a JSON number already is (ADR 0007), so every JSON number
            // -- integer or not -- reads as one, with no exactness to lose the way Int64 does.
            return Ok(());
        }

        (Type::Vec(elem), Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                validate(enums, item, elem, &format!("{path}[{i}]"))?;
            }
            return Ok(());
        }

        (Type::Record(fields), Value::Object(map)) => {
            for (name, field_ty) in fields {
                let Some(v) = map.get(name) else {
                    return Err(format!("{path}: missing field `{name}`"));
                };
                validate(enums, v, field_ty, &format!("{path}.{name}"))?;
            }
            // Undeclared fields are ignored rather than rejected, so a program can read two
            // fields out of a log line without describing the whole line. Open: whether that
            // is the right call once the type is also used to lay the value out.
            return Ok(());
        }

        // One enum value spans two JSON shapes (ADR 0009): a bare string naming a unit
        // variant, or a single-key object whose key names a payload variant. Everything else
        // is refused with the enum's name, since "found a string" alone would not say which
        // closed set the string missed.
        (Type::Enum { name, .. }, v) => {
            // Read from the registry, not off `ty`: a recursive enum's nested occurrence of
            // itself carries a placeholder in place of its variants (kantord/toylang#94).
            let variants = ty::variants(enums, ty);
            return match v {
                Value::String(s) => match variants.iter().find(|(n, _)| n == s) {
                    Some((_, None)) => Ok(()),
                    Some((_, Some(_))) => Err(format!(
                        "{path}: `{s}` is a payload variant of {name}, written {{\"{s}\": ...}}"
                    )),
                    None => Err(format!("{path}: `{s}` is not a variant of {name}")),
                },
                Value::Object(map) if map.len() == 1 => {
                    let (key, inner) = map.iter().next().expect("exactly one entry");
                    match variants.iter().find(|(n, _)| n == key) {
                        Some((_, Some(payload))) => {
                            validate(enums, inner, payload, &format!("{path}.{key}"))
                        }
                        Some((_, None)) => Err(format!(
                            "{path}: `{key}` is a unit variant of {name}, written \"{key}\""
                        )),
                        None => Err(format!("{path}: `{key}` is not a variant of {name}")),
                    }
                }
                v => Err(format!("{path}: expected {name}, found {}", describe(v))),
            };
        }

        (_, v) => describe(v),
    };
    Err(format!("{path}: expected {ty}, found {found}"))
}

fn describe(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

pub fn to_lua(lua: &Lua, value: &Value) -> mlua::Result<mlua::Value> {
    Ok(match value {
        Value::Null => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => mlua::Value::Integer(i),
            None => mlua::Value::Number(n.as_f64().expect("a number is i64 or f64")),
        },
        Value::String(s) => mlua::Value::String(lua.create_string(s)?),
        Value::Array(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                t.set(i + 1, to_lua(lua, item)?)?;
            }
            mlua::Value::Table(t)
        }
        Value::Object(map) => {
            let t = lua.create_table()?;
            for (k, v) in map {
                t.set(k.as_str(), to_lua(lua, v)?)?;
            }
            mlua::Value::Table(t)
        }
    })
}
