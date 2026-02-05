/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::visibility::VisibilityAttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use kuro_node::visibility::VisibilityPattern;
use kuro_node::visibility::VisibilityWithinViewBuilder;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::attr_type::list::coerce_list;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;
use crate::interpreter::selector::StarlarkSelector;

#[derive(Debug, kuro_error::Error)]
enum VisibilityAttrTypeCoerceError {
    #[error("Visibility attribute is not configurable (internal error)")]
    #[kuro(tag = Tier0)]
    AttrTypeNotConfigurable,
    #[error("Visibility must be a list of string, got `{0}`")]
    #[kuro(tag = Input)]
    WrongType(String),
    #[error("Visibility attribute is not configurable (i.e. cannot use `select()`): `{0}`")]
    #[kuro(tag = Input)]
    NotConfigurable(String),
}

impl AttrTypeCoerce for VisibilityAttrType {
    fn coerce_item(
        &self,
        configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> kuro_error::Result<CoercedAttr> {
        if configurable == AttrIsConfigurable::Yes {
            return Err(VisibilityAttrTypeCoerceError::AttrTypeNotConfigurable.into());
        }
        Ok(CoercedAttr::Visibility(
            parse_visibility_with_view(ctx, value)?.build_visibility(),
        ))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        AttrType::list(AttrType::string()).starlark_type()
    }
}

/// Bazel-style visibility constants
const BAZEL_VISIBILITY_PUBLIC: &str = "//visibility:public";
const BAZEL_VISIBILITY_PRIVATE: &str = "//visibility:private";

pub(crate) fn parse_visibility_with_view(
    ctx: &dyn AttrCoercionContext,
    attr: Value,
) -> kuro_error::Result<VisibilityWithinViewBuilder> {
    let list = match coerce_list(attr) {
        Ok(list) => list,
        Err(e) => {
            if StarlarkSelector::from_value(attr).is_some() {
                return Err(VisibilityAttrTypeCoerceError::NotConfigurable(attr.to_repr()).into());
            }
            return Err(e);
        }
    };

    let mut builder = VisibilityWithinViewBuilder::with_capacity(list.len());
    for item in list {
        let Some(item) = item.unpack_str() else {
            if StarlarkSelector::from_value(*item).is_some() {
                return Err(VisibilityAttrTypeCoerceError::NotConfigurable(attr.to_repr()).into());
            }
            return Err(VisibilityAttrTypeCoerceError::WrongType(attr.to_repr()).into());
        };

        // Support both Kuro-style ("PUBLIC") and Bazel-style ("//visibility:public")
        if item == VisibilityPattern::PUBLIC || item == BAZEL_VISIBILITY_PUBLIC {
            // TODO(cjhopman): We should probably enforce that this is the only entry.
            builder.add_public();
        } else if item == BAZEL_VISIBILITY_PRIVATE {
            // //visibility:private means no visibility - don't add anything
            // The default is already private (empty list), so we just skip this entry
            continue;
        } else {
            // Handle Bazel's special package patterns:
            // - //pkg:__pkg__ means only that exact package can see this target
            // - //pkg:__subpackages__ means that package and all subpackages can see this target
            //
            // These are handled by the target pattern parser, but we need to convert them
            // to the format expected by Kuro's pattern matcher.
            let normalized_item = if item.ends_with(":__pkg__") {
                // //pkg:__pkg__ -> //pkg: (matches all targets in exact package)
                item.trim_end_matches("__pkg__").to_owned()
            } else if item.ends_with(":__subpackages__") {
                // //pkg:__subpackages__ -> //pkg/... (recursive match)
                let base = item.trim_end_matches(":__subpackages__");
                format!("{}/...", base)
            } else {
                item.to_owned()
            };

            builder.add(VisibilityPattern(
                ctx.coerce_target_pattern(&normalized_item)?,
            ));
        }
    }
    Ok(builder)
}
