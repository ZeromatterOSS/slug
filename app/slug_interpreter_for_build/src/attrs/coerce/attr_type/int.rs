/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_node::attrs::attr_type::int::IntAttrType;
use slug_node::attrs::coerced_attr::CoercedAttr;
use slug_node::attrs::coercion_context::AttrCoercionContext;
use slug_node::attrs::configurable::AttrIsConfigurable;
use starlark::typing::Ty;
use starlark::values::UnpackValue;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;

impl AttrTypeCoerce for IntAttrType {
    fn coerce_item(
        &self,
        _configurable: AttrIsConfigurable,
        _ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> slug_error::Result<CoercedAttr> {
        let v = i64::unpack_value_err(value)?;
        // Validate against allowed values if specified (Bazel attr.int(values=[...]))
        if let Some(allowed) = &self.allowed_values {
            if !allowed.contains(&v) {
                let allowed_str: Vec<String> = allowed.iter().map(|i| i.to_string()).collect();
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Integer value {} is not allowed. Must be one of: [{}]",
                    v,
                    allowed_str.join(", ")
                ));
            }
        }
        Ok(CoercedAttr::Int(v))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Basic(Ty::int())
    }
}
