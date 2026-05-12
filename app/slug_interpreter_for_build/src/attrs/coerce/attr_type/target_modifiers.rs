/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_error::internal_error;
use slug_interpreter::types::opaque_metadata::OpaqueMetadata;
use slug_node::attrs::attr_type::target_modifiers::TargetModifiersAttrType;
use slug_node::attrs::coerced_attr::CoercedAttr;
use slug_node::attrs::coercion_context::AttrCoercionContext;
use slug_node::attrs::configurable::AttrIsConfigurable;
use slug_node::attrs::values::TargetModifiersValue;
use starlark::values::Value;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;

impl AttrTypeCoerce for TargetModifiersAttrType {
    fn coerce_item(
        &self,
        configurable: AttrIsConfigurable,
        _ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> slug_error::Result<CoercedAttr> {
        if configurable == AttrIsConfigurable::Yes {
            return Err(internal_error!("modifiers attribute is not configurable"));
        }
        let value = value
            .to_json_value()
            .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Tier0))
            .with_buck_error_context(|| {
                format!(
                    "Target modifiers attribute is not convertible to JSON: {}",
                    value.to_repr(),
                )
            })?;

        Ok(CoercedAttr::TargetModifiers(TargetModifiersValue::new(
            value,
        )))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Basic(OpaqueMetadata::starlark_type_repr())
    }
}
