/// A raw native cache-field value. This deliberately does not parse or
/// normalize option defaults or values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheFieldValue<'a> {
    Null,
    Empty,
    Scalar(&'a str),
}
use crate::native::value::*;
pub(super) fn native_cache_text(v: &NativeValue) -> Result<String, ()> {
    Ok(match v {
        NativeValue::Bool(x) => x.to_string(),
        NativeValue::Int(x) => x.to_string(),
        NativeValue::Text(x) | NativeValue::Dotted(x) => x.to_string(),
        NativeValue::Tri(x) => match x {
            TriState::Auto => "AUTO",
            TriState::Yes => "YES",
            TriState::No => "NO",
        }
        .into(),
        NativeValue::Enum(x) => x.member.to_string(),
        NativeValue::Duration(x) => dur(*x),
        NativeValue::Entry(k, v) => format!("{k}={v}"),
        NativeValue::Env(EnvValue::Set(n, v)) => format!("Set[name={n}, value={v}]"),
        NativeValue::Env(EnvValue::Inherit(n)) => format!("Inherit[name={n}]"),
        NativeValue::Env(EnvValue::Unset(n)) => format!("Unset[name={n}]"),
        NativeValue::Shard(ShardValue::Explicit) => "EXPLICIT".into(),
        NativeValue::Shard(ShardValue::Disabled) => "DISABLED".into(),
        NativeValue::Shard(ShardValue::Forced(x)) => format!("forced={x}"),
        NativeValue::Runs(x) => format!("(?:(?>.*)) Options: [{}]", x.positive_runs()),
        NativeValue::List(x) => format!(
            "[{}]",
            x.iter()
                .map(|v| native_cache_text(v).unwrap())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NativeValue::OrderedMap(x) => format!(
            "{{{}}}",
            x.iter()
                .map(|(k, v)| format!(
                    "{}={}",
                    native_cache_text(k).unwrap(),
                    native_cache_text(v).unwrap()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}
pub(super) fn format_native_cache_field(n: &str, v: Option<&NativeValue>) -> String {
    match v {
        None => format_cache_field(n, CacheFieldValue::Null),
        Some(NativeValue::List(x)) if x.is_empty() => format_cache_field(n, CacheFieldValue::Empty),
        Some(x) => format_cache_field(n, CacheFieldValue::Scalar(&native_cache_text(x).unwrap())),
    }
}
fn dur(x: Duration) -> String {
    let mut s = "PT".to_owned();
    let (h, m, q) = (x.seconds / 3600, (x.seconds % 3600) / 60, x.seconds % 60);
    if h > 0 {
        s.push_str(&format!("{h}H"))
    }
    if m > 0 {
        s.push_str(&format!("{m}M"))
    }
    if q > 0 || x.nanos > 0 {
        if x.nanos == 0 {
            s.push_str(&format!("{q}S"))
        } else {
            let mut f = format!("{:09}", x.nanos);
            while f.ends_with('0') {
                f.pop();
            }
            s.push_str(&format!("{q}.{f}S"))
        }
    }
    if s == "PT" { "PT0S".into() } else { s }
}

/// Formats one `OptionsBase::mapToCacheKey`-style field.
///
/// Scalars are quoted, with only backslashes and double quotes escaped. Every
/// field has the trailing comma and space required by the native cache grammar.
pub fn format_cache_field(name: &str, value: CacheFieldValue<'_>) -> String {
    let mut output = String::with_capacity(name.len() + 10);
    output.push_str(name);
    output.push('=');
    match value {
        CacheFieldValue::Null => output.push_str("NULL"),
        CacheFieldValue::Empty => output.push_str("EMPTY"),
        CacheFieldValue::Scalar(value) => {
            output.push('"');
            for character in value.chars() {
                if matches!(character, '\\' | '"') {
                    output.push('\\');
                }
                output.push(character);
            }
            output.push('"');
        }
    }
    output.push_str(", ");
    output
}
