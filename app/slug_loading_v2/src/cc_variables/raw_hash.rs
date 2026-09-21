//! Java raw-payload hashing does not apply Starlark list/dict hash barriers.
//! Hash bits are Slug-native; equal raw payloads must nevertheless hash equally.
use std::hash::Hasher;

use starlark::values::structs::StructRef;

use super::*;
use crate::provider::StarlarkToolchainInfo;
use crate::provider::starlark_user_provider_fields;

pub(super) fn hash(value: Value<'_>, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
    visit(value, hasher, &mut Vec::new())
}
fn visit<'v>(
    value: Value<'v>,
    hasher: &mut StarlarkHasher,
    stack: &mut Vec<Value<'v>>,
) -> starlark::Result<()> {
    if stack.iter().any(|v| v.identity() == value.identity()) {
        return Err(starlark::Error::new_other(anyhow::anyhow!(
            "cyclic C++ variable payload is unsupported"
        )));
    }
    stack.push(value);
    let result = (|| {
        if let Some(list) = ListRef::from_value(value) {
            "list".hash(hasher);
            for v in list.iter() {
                visit(v, hasher, stack)?;
            }
        } else if let Some(tuple) = TupleRef::from_value(value) {
            "tuple".hash(hasher);
            for v in tuple.iter() {
                visit(v, hasher, stack)?;
            }
        } else if let Some(dict) = DictRef::from_value(value) {
            "dict".hash(hasher);
            let mut unordered = 0u64;
            for (key, value) in dict.iter() {
                let mut item = StarlarkHasher::new();
                visit(key, &mut item, stack)?;
                visit(value, &mut item, stack)?;
                unordered = unordered.wrapping_add(item.finish());
            }
            unordered.hash(hasher);
        } else if let Some(structure) = StructRef::from_value(value) {
            let mut fields = structure
                .iter()
                .map(|(k, v)| (k.as_str(), v))
                .collect::<Vec<_>>();
            fields.sort_by(|(a, _), (b, _)| a.cmp(b));
            "struct".hash(hasher);
            for (name, value) in fields {
                name.hash(hasher);
                visit(value, hasher, stack)?;
            }
        } else if let Some((id, mut fields)) = starlark_user_provider_fields(value) {
            fields.sort_by(|(a, _), (b, _)| a.cmp(b));
            id.hash(hasher);
            for (name, value) in fields {
                name.hash(hasher);
                visit(value, hasher, stack)?;
            }
        } else if let Some(mut fields) = StarlarkToolchainInfo::fields_from_value(value) {
            fields.sort_by(|(a, _), (b, _)| a.cmp(b));
            "ToolchainInfo".hash(hasher);
            for (name, value) in fields {
                name.hash(hasher);
                visit(value, hasher, stack)?;
            }
        } else if let Some(variables) = cc_variables_value(value) {
            // Raw Java object hashing bypasses Single's Starlark hash barrier.
            // Keep one cycle stack through nested Variables instead of invoking
            // write_hash again with a fresh stack.
            variables.kind.hash(hasher);
            parent_occurrence(variables.parent).hash(hasher);
            for (name, value) in variables.locals {
                name.hash(hasher);
                visit(value, hasher, stack)?;
            }
        } else {
            value.write_hash(hasher)?;
        }
        Ok(())
    })();
    stack.pop();
    result
}
