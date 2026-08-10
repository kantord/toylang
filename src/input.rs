use mlua::Lua;
use serde_json::Value;

use crate::ty::Type;

/// Check the input against the type the program declared, before anything runs.
///
/// This is a parse, not a coercion. `{"age": "36"}` where `Int` was declared is an error, not a
/// conversion: the moment input is coerced, the type stops describing the value and the runtime
/// and the type system have stopped making the same guarantee.
pub fn validate(value: &Value, ty: &Type, path: &str) -> Result<(), String> {
    let found = match (ty, value) {
        (Type::Str, Value::String(_))
        | (Type::Bool, Value::Bool(_)) => return Ok(()),

        (Type::Int, Value::Number(n)) => {
            return if n.is_i64() {
                Ok(())
            } else {
                Err(format!("{path}: expected Int, found the non-integer number {n}"))
            };
        }

        (Type::Vec(elem), Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                validate(item, elem, &format!("{path}[{i}]"))?;
            }
            return Ok(());
        }

        (Type::Record(fields), Value::Object(map)) => {
            for (name, field_ty) in fields {
                let Some(v) = map.get(name) else {
                    return Err(format!("{path}: missing field `{name}`"));
                };
                validate(v, field_ty, &format!("{path}.{name}"))?;
            }
            // Undeclared fields are ignored rather than rejected, so a program can read two
            // fields out of a log line without describing the whole line. Open: whether that
            // is the right call once the type is also used to lay the value out.
            return Ok(());
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
