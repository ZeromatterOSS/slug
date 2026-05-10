/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use display_container::fmt_keyed_container;
use dupe::Dupe;
use indexmap::map::RawEntryApiV1;
use kuro_core::provider::id::ProviderId;
use serde::Serializer;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::coerce::coerce;
use starlark::collections::Hashed;
use starlark::collections::StarlarkHasher;
use starlark::eval::Evaluator;
use starlark::eval::ParametersParser;
use starlark::typing::Ty;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FrozenRef;
use starlark::values::Heap;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::FrozenDictRef;
use starlark::values::list::FrozenListRef;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::provider::ProviderLike;
use crate::interpreter::rule_defs::provider::callable::UserProviderCallableData;

static USER_PROVIDER_CREATE_COUNT: AtomicUsize = AtomicUsize::new(0);
static RULES_CC_USER_PROVIDER_CREATE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum UserProviderError {
    #[error("Value for parameter `{0}` mismatches type `{1}`: `{2}`")]
    MismatchedType(String, Ty, String),
    #[error("Required parameter `{0}` is missing")]
    MissingParameter(String),
}

/// The result of calling the output of `provider()`. This is just a simple data structure of
/// either immediately available values or, later, `FutureValue` types that are resolved
/// asynchronously
#[derive(Debug, Clone, Coerce, Trace, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct UserProviderGen<'v, V: ValueLike<'v>> {
    pub(crate) callable: FrozenRef<'static, UserProviderCallableData>,
    attributes: Box<[V]>,
    present: Box<[bool]>,
    _marker: PhantomData<&'v ()>,
}

starlark_complex_value!(pub UserProvider<'v>);

impl<'v, V: ValueLike<'v>> UserProviderGen<'v, V> {
    fn iter_items(&self) -> impl Iterator<Item = (&str, V)> {
        assert_eq!(self.callable.fields.len(), self.attributes.len());
        assert_eq!(self.callable.fields.len(), self.present.len());
        self.callable
            .fields
            .keys()
            .map(|s| s.as_str())
            .zip(self.attributes.iter().copied())
            .zip(self.present.iter().copied())
            .filter_map(|((name, value), present)| present.then_some((name, value)))
    }
}

fn write_user_provider_field_hash(
    value: Value<'_>,
    hasher: &mut StarlarkHasher,
) -> starlark::Result<()> {
    if let Some(list) = FrozenListRef::from_value(value) {
        "list".hash(hasher);
        list.len().hash(hasher);
        for value in list.iter() {
            write_user_provider_field_hash(value.to_value(), hasher)?;
        }
        return Ok(());
    }

    if let Some(frozen_value) = value.unpack_frozen() {
        if let Some(dict) = FrozenDictRef::from_frozen_value(frozen_value) {
            "dict".hash(hasher);
            let entries: Vec<_> = dict.iter().collect();
            entries.len().hash(hasher);
            let mut entry_hashes = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let mut entry_hasher = StarlarkHasher::new();
                write_user_provider_field_hash(key.to_value(), &mut entry_hasher)?;
                write_user_provider_field_hash(value.to_value(), &mut entry_hasher)?;
                entry_hashes.push(entry_hasher.finish());
            }
            entry_hashes.sort_unstable();
            for hash in entry_hashes {
                hash.hash(hasher);
            }
            return Ok(());
        }
    }

    value.write_hash(hasher)
}

impl<'v, V: ValueLike<'v>> Display for UserProviderGen<'v, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_keyed_container(
            f,
            &format!("{}(", self.callable.provider_id.name),
            ")",
            "=",
            self.iter_items(),
        )
    }
}

#[starlark_value(type = "struct")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for UserProviderGen<'v, V>
where
    Self: ProvidesStaticType<'v>,
{
    fn dir_attr(&self) -> Vec<String> {
        assert_eq!(self.callable.fields.len(), self.attributes.len());
        assert_eq!(self.callable.fields.len(), self.present.len());
        self.callable
            .fields
            .keys()
            .zip(self.present.iter())
            .filter_map(|(name, present)| present.then(|| name.clone()))
            .collect()
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.get_attr_hashed(Hashed::new(attribute), heap)
    }

    fn get_attr_hashed(&self, attribute: Hashed<&str>, _heap: Heap<'v>) -> Option<Value<'v>> {
        let index = self
            .callable
            .fields
            .raw_entry_v1()
            .index_from_hash(attribute.hash().promote(), |k| k == attribute.key())?;
        self.present[index].then(|| self.attributes[index].to_value())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let this: &UserProvider = coerce(self);
        let other: &UserProvider = match UserProvider::from_value(other) {
            Some(other) => other,
            None => return Ok(false),
        };
        if this.callable.provider_id != other.callable.provider_id {
            return Ok(false);
        }
        if this.attributes.len() != other.attributes.len() {
            // If provider ids are equal, then providers point to the same provider callable,
            // and lengths should be equal. So this code is unreachable.
            return Ok(false);
        }
        if this.present != other.present {
            return Ok(false);
        }
        for ((v1, v2), present) in this
            .attributes
            .iter()
            .zip(other.attributes.iter())
            .zip(this.present.iter())
        {
            if *present {
                if !v1.equals(v2.to_value())? {
                    return Ok(false);
                }
            }
        }
        for (k1, k2) in this
            .callable
            .fields
            .keys()
            .zip(other.callable.fields.keys())
        {
            if k1 != k2 {
                // If provider ids are equal, then providers point to the same provider callable,
                // and keys should be equal. So this code is unreachable.
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.callable.provider_id.hash(hasher);
        for (k, v) in self.iter_items() {
            k.hash(hasher);
            write_user_provider_field_hash(v.to_value(), hasher)?;
        }
        Ok(())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

impl<'v, V: ValueLike<'v>> serde::Serialize for UserProviderGen<'v, V> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.collect_map(self.iter_items())
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for UserProviderGen<'v, V> {
    fn id(&self) -> &Arc<ProviderId> {
        &self.callable.provider_id
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        self.iter_items().map(|(k, v)| (k, v.to_value())).collect()
    }
}

fn provider_path_is_rules_cc_cc_private(provider_id: &ProviderId) -> bool {
    let Some(path) = &provider_id.path else {
        return false;
    };
    path.cell().as_str().starts_with("rules_cc+") && path.path().as_str().contains("cc/private")
}

fn user_provider_create_checkpoint(
    checkpoint: &'static str,
    callable: FrozenRef<'static, UserProviderCallableData>,
    eval: &Evaluator<'_, '_, '_>,
    value_count: usize,
) {
    if !kuro_util::memory_checkpoint::enabled()
        || !provider_path_is_rules_cc_cc_private(&callable.provider_id)
    {
        return;
    }

    let create_count = USER_PROVIDER_CREATE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    let rules_cc_create_count =
        RULES_CC_USER_PROVIDER_CREATE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    if rules_cc_create_count > 32 && !rules_cc_create_count.is_power_of_two() {
        return;
    }

    let location = eval.call_stack_top_location();
    let (call_file_len, call_line, call_column) = location
        .as_ref()
        .map(|location| {
            let resolved = location.resolve_span();
            (
                location.filename().len(),
                resolved.begin.line + 1,
                resolved.begin.column + 1,
            )
        })
        .unwrap_or((0, 0, 0));

    kuro_util::memory_checkpoint::checkpoint(
        checkpoint,
        [
            ("create_count", create_count),
            ("rules_cc_create_count", rules_cc_create_count),
            ("schema_fields", callable.fields.len()),
            ("values", value_count),
            ("stack_depth", eval.call_stack_count()),
            ("call_file_len", call_file_len),
            ("call_line", call_line),
            ("call_column", call_column),
        ],
    );
    tracing::warn!(
        target: "kuro_memory",
        checkpoint,
        provider_name = callable.provider_id.name.as_str(),
        provider_path = ?callable.provider_id.path,
        call_file = location
            .as_ref()
            .map(|location| location.filename())
            .unwrap_or("<unknown>"),
        call_line,
        rules_cc_create_count,
        schema_fields = callable.fields.len(),
        values = value_count,
        "rules_cc user provider construction provider={} count={} fields={} values={}",
        callable.provider_id.name,
        rules_cc_create_count,
        callable.fields.len(),
        value_count
    );
}

/// Creates instances of mutable `UserProvider`s; called from a `NativeFunction`
pub(crate) fn user_provider_creator<'v>(
    callable: FrozenRef<'static, UserProviderCallableData>,
    eval: &Evaluator<'v, '_, '_>,
    param_parser: &mut ParametersParser<'v, '_>,
) -> kuro_error::Result<Value<'v>> {
    let heap = eval.heap();
    let values = callable
        .fields
        .iter()
        .map(|(name, field)| match param_parser.next_opt()? {
            Some(value) => {
                if !field.ty.matches(value) {
                    return Err(UserProviderError::MismatchedType(
                        name.to_owned(),
                        field.ty.as_ty().dupe(),
                        value.to_repr(),
                    )
                    .into());
                }
                Ok((true, value))
            }
            None => match field.default {
                Some(default) => Ok((true, default.to_value())),
                None if field.required => {
                    Err(UserProviderError::MissingParameter(name.to_owned()).into())
                }
                None => Ok((false, Value::new_none())),
            },
        })
        .collect::<kuro_error::Result<Vec<_>>>()?;
    let value_count = values.len();
    let (present, attributes): (Vec<bool>, Vec<Value<'v>>) = values.into_iter().unzip();
    user_provider_create_checkpoint("user_provider_create", callable, eval, value_count);
    Ok(heap.alloc(UserProvider {
        callable,
        attributes: attributes.into_boxed_slice(),
        present: present.into_boxed_slice(),
        _marker: PhantomData,
    }))
}

/// Creates instances of schemaless `UserProvider`s; called for providers created without `fields=`.
/// Schemaless providers accept arbitrary keyword arguments.
///
/// For schemaless providers, we dynamically add the kwargs as fields to the callable,
/// then create a regular UserProvider. This allows it to work with the existing freeze machinery.
pub(crate) fn user_provider_creator_schemaless<'v>(
    callable: FrozenRef<'static, UserProviderCallableData>,
    args: &starlark::eval::Arguments<'v, '_>,
    eval: &Evaluator<'v, '_, '_>,
) -> kuro_error::Result<Value<'v>> {
    let heap = eval.heap();

    // Get all named arguments as a map
    let names_map = args.names_map()?;

    // Convert to a vector of (name, value) pairs, sorted by key for determinism
    let mut pairs: Vec<_> = names_map
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), *v))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    // Build a SchemalessUserProvider with the sorted fields
    let (field_names, values): (Vec<String>, Vec<Value<'v>>) = pairs.into_iter().unzip();
    user_provider_create_checkpoint(
        "user_provider_create_schemaless",
        callable,
        eval,
        values.len(),
    );

    Ok(heap.alloc(SchemalessUserProvider {
        callable,
        field_names: field_names.into_boxed_slice(),
        values: values.into_boxed_slice(),
        _marker: PhantomData,
    }))
}

// ============================================================================
// SchemalessUserProvider - For providers created without fields= parameter
// ============================================================================

/// A provider instance created from a schemaless provider definition.
/// Uses separate arrays for field names and values, similar to UserProviderGen.
///
/// This is a complex value that supports freeze/thaw operations.
#[derive(Debug, Clone, Coerce, Trace, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct SchemalessUserProviderGen<'v, V: ValueLike<'v>> {
    pub(crate) callable: FrozenRef<'static, UserProviderCallableData>,
    /// Field names (sorted for determinism)
    field_names: Box<[String]>,
    /// Field values (in same order as field_names)
    values: Box<[V]>,
    _marker: PhantomData<&'v ()>,
}

starlark_complex_value!(pub SchemalessUserProvider<'v>);

impl<'v, V: ValueLike<'v>> SchemalessUserProviderGen<'v, V> {
    fn iter_items(&self) -> impl Iterator<Item = (&str, V)> {
        assert_eq!(self.field_names.len(), self.values.len());
        self.field_names
            .iter()
            .map(|s| s.as_str())
            .zip(self.values.iter().copied())
    }
}

impl<'v, V: ValueLike<'v>> Display for SchemalessUserProviderGen<'v, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_keyed_container(
            f,
            &format!("{}(", self.callable.provider_id.name),
            ")",
            "=",
            self.iter_items(),
        )
    }
}

#[starlark_value(type = "struct")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for SchemalessUserProviderGen<'v, V>
where
    Self: ProvidesStaticType<'v>,
{
    fn dir_attr(&self) -> Vec<String> {
        self.field_names.iter().cloned().collect()
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        self.field_names
            .iter()
            .position(|n| n == attribute)
            .map(|idx| self.values[idx].to_value())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

impl<'v, V: ValueLike<'v>> serde::Serialize for SchemalessUserProviderGen<'v, V> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.collect_map(self.iter_items())
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for SchemalessUserProviderGen<'v, V> {
    fn id(&self) -> &Arc<ProviderId> {
        &self.callable.provider_id
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        self.iter_items().map(|(k, v)| (k, v.to_value())).collect()
    }
}
