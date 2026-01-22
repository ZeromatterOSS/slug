/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use kuro_core::configuration::transition::id::TransitionId;
use kuro_node::attrs::attr_type::transition_dep::CoercedTransitionDep;
use kuro_node::attrs::attr_type::transition_dep::TransitionDepAttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use starlark::typing::Ty;
use starlark::values::UnpackValue;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;

impl AttrTypeCoerce for TransitionDepAttrType {
    fn coerce_item(
        &self,
        _configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> kuro_error::Result<CoercedAttr> {
        let (dep, transition) = if self.transition.is_some() {
            (ctx.coerce_providers_label(value.unpack_str_err()?)?, None)
        } else {
            let (dep, transition) = UnpackValue::unpack_value_err(value)?;
            (
                ctx.coerce_providers_label(dep)?,
                Some(Arc::new(TransitionId::Target(
                    ctx.coerce_providers_label(transition)?,
                ))),
            )
        };

        Ok(CoercedAttr::TransitionDep(Box::new(CoercedTransitionDep {
            dep,
            transition,
        })))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Basic(Ty::unions(vec![
            Ty::tuple2(Ty::string(), Ty::string()),
            Ty::string(),
        ]))
    }
}
