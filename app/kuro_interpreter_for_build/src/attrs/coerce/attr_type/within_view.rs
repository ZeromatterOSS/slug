/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_error::internal_error;
use kuro_node::attrs::attr_type::visibility::VisibilityAttrType;
use kuro_node::attrs::attr_type::within_view::WithinViewAttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;
use crate::attrs::coerce::attr_type::visibility::parse_visibility_with_view;

impl AttrTypeCoerce for WithinViewAttrType {
    fn coerce_item(
        &self,
        configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> kuro_error::Result<CoercedAttr> {
        if configurable == AttrIsConfigurable::Yes {
            return Err(internal_error!("Within view attribute is not configurable"));
        }
        Ok(CoercedAttr::WithinView(
            parse_visibility_with_view(ctx, value)?.build_within_view(),
        ))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        // Starlark type of the attribute is the same.
        VisibilityAttrType.starlark_type()
    }
}
