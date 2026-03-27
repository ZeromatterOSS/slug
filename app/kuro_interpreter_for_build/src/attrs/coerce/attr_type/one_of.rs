/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use gazebo::prelude::SliceExt;
use kuro_node::attrs::attr_type::one_of::OneOfAttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;
use crate::attrs::coerce::error::CoercionError;

impl AttrTypeCoerce for OneOfAttrType {
    fn coerce_item(
        &self,
        configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> kuro_error::Result<CoercedAttr> {
        let mut errs = Vec::new();

        // Bazel compatibility: when the value looks like a source file path
        // (bare string with / and no label markers), try source coercion before
        // dep coercion. In Bazel, source files are implicit targets, but in Buck2
        // they're separate — so source coercion must take precedence for file paths
        // in one_of(dep, source) contexts like attr.label_list(allow_files=True).
        let prefer_source_first = if let Some(s) = value.unpack_str() {
            s.contains('/') && !s.starts_with("//") && !s.starts_with('@') && !s.starts_with(':')
        } else {
            false
        };

        if prefer_source_first && self.xs.len() >= 2 {
            // Try alternatives in reverse order (source types tend to be last)
            for (i, x) in self.xs.iter().enumerate().rev() {
                match x.coerce_item(configurable, ctx, value) {
                    Ok(v) => return Ok(CoercedAttr::OneOf(Box::new(v), i as u32)),
                    Err(e) => errs.push(e),
                }
            }
        } else {
            // Normal order: bias towards the start of the list
            for (i, x) in self.xs.iter().enumerate() {
                match x.coerce_item(configurable, ctx, value) {
                    Ok(v) => return Ok(CoercedAttr::OneOf(Box::new(v), i as u32)),
                    Err(e) => {
                        // TODO(nga): anyhow error creation is expensive.
                        errs.push(e)
                    }
                }
            }
        }
        Err(CoercionError::one_of_many(errs).into())
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Union(self.xs.map(|x| x.starlark_type()))
    }
}
