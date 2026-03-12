/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_error::BuckErrorContext;
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr::CoercedValue;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use starlark::docs::DocString;
use starlark::docs::DocStringKind;
use starlark::values::Value;

use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;
use crate::attrs::coerce::error::CoercionError;

pub(crate) mod attrs_global;
pub mod coerce;
pub(crate) mod starlark_attribute;
pub use attrs_global::resolve_configuration_field_to_label;
pub use starlark_attribute::StarlarkAttribute;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum AttrCoerceError {
    #[error("Parameter `{0}` had no value provided, but it is mandatory")]
    MissingMandatoryParameter(String),
}

pub trait AttributeCoerceExt {
    fn coerce<'v>(
        &self,
        param_name: &str,
        configurable: AttrIsConfigurable,
        coercer_ctx: &dyn AttrCoercionContext,
        value: Value<'v>,
    ) -> kuro_error::Result<CoercedValue>;

    fn docstring(&self) -> Option<DocString>;

    fn starlark_type(&self) -> TyMaybeSelect;
}

impl AttributeCoerceExt for Attribute {
    /// Attempt to coerce a value. If the value provided is `None`, and a default value is available,
    /// that default value is returned.
    fn coerce<'v>(
        &self,
        param_name: &str,
        configurable: AttrIsConfigurable,
        coercer_ctx: &dyn AttrCoercionContext,
        value: Value<'v>,
    ) -> kuro_error::Result<CoercedValue> {
        if self.is_default_only() {
            if value.is_none() {
                return Ok(CoercedValue::Default);
            } else {
                return Err(CoercionError::DefaultOnly(value.to_string()).into());
            }
        }

        match self.default() {
            default if !value.is_none() => {
                let coerced = self
                    .coercer()
                    .coerce_with_default(configurable, coercer_ctx, value, default.map(|x| &**x))
                    .with_buck_error_context(|| {
                        format!("Error coercing attribute `{param_name}` of type `{self}`")
                    })?;

                // Enforce allow_empty constraint for list attributes
                if !self.allow_empty() {
                    if let kuro_node::attrs::coerced_attr::CoercedAttr::List(ref items) = coerced {
                        if items.is_empty() {
                            return Err(kuro_error::kuro_error!(
                                kuro_error::ErrorTag::Input,
                                "attribute `{}` cannot be empty (allow_empty is False)",
                                param_name,
                            ));
                        }
                    }
                }

                Ok(CoercedValue::Custom(coerced))
            }
            Some(_) => Ok(CoercedValue::Default),
            None => Err(AttrCoerceError::MissingMandatoryParameter(param_name.to_owned()).into()),
        }
    }

    fn docstring(&self) -> Option<DocString> {
        DocString::from_docstring(DocStringKind::Starlark, self.doc())
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        self.coercer().starlark_type()
    }
}
