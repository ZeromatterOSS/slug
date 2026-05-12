/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_node::attrs::attr_type::bool::BoolAttrType;
use slug_node::attrs::attr_type::bool::BoolLiteral;
use slug_node::attrs::coerced_attr::CoercedAttr;
use slug_node::attrs::coercion_context::AttrCoercionContext;
use slug_node::attrs::configurable::AttrIsConfigurable;
use starlark::typing::Ty;
use starlark::values::UnpackValue;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;

impl AttrTypeCoerce for BoolAttrType {
    fn coerce_item(
        &self,
        _configurable: AttrIsConfigurable,
        _ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> slug_error::Result<CoercedAttr> {
        // Bazel allows integers for bool attributes (0 = False, nonzero = True)
        let b = if let Some(b) = value.unpack_bool() {
            b
        } else if let Ok(Some(i)) = i64::unpack_value(value) {
            i != 0
        } else {
            UnpackValue::unpack_value_err(value)?
        };
        Ok(CoercedAttr::Bool(BoolLiteral(b)))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Basic(Ty::bool())
    }
}
