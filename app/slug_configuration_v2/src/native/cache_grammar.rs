/// A raw native cache-field value. This deliberately does not parse or
/// normalize option defaults or values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheFieldValue<'a> {
    Null,
    Empty,
    Scalar(&'a str),
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
