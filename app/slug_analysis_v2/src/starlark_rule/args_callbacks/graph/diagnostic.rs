//! Borrowed provider projection and bounded error-only class diagnostics.
use slug_build_api_v2::ProviderIdentity;

use super::*;

pub(in super::super) fn provider_fields<'v>(
    value: Value<'v>,
) -> Option<(ProviderIdentity, Vec<(CompactString, Value<'v>)>)> {
    if let Some(fields) = slug_loading_v2::provider::StarlarkToolchainInfo::fields_from_value(value)
    {
        return Some((ProviderIdentity::builtin("ToolchainInfo"), fields));
    }
    slug_loading_v2::provider::starlark_user_provider_fields(value)
        .map(|(id, fields)| (ProviderIdentity::user(id), fields))
}
/// Diagnostic prefix only: stop before Debug enters any object's fields.
/// This distinguishes concrete wrapper classes without printing captures/code.
pub(super) fn bounded_value_class(value: Value<'_>) -> String {
    struct Prefix(String);
    impl std::fmt::Write for Prefix {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            for ch in text.chars() {
                if matches!(ch, '{' | '"' | '\'') || self.0.len() + ch.len_utf8() > 160 {
                    return Err(std::fmt::Error);
                }
                self.0.push(ch);
            }
            Ok(())
        }
    }
    let mut prefix = Prefix(String::new());
    let _ = std::fmt::write(&mut prefix, format_args!("{value:?}"));
    prefix.0
}

pub(super) fn host_error(message: String) -> starlark::Error {
    starlark::Error::new_other(std::io::Error::other(message))
}
