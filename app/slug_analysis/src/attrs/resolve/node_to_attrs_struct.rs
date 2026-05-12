/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_node::attrs::inspect_options::AttrInspectOptions;
use slug_node::nodes::configured::ConfiguredTargetNodeRef;
use starlark::values::ValueOfUnchecked;
use starlark::values::structs::AllocStruct;
use starlark::values::structs::StructRef;

use crate::attrs::resolve::configured_attr::ConfiguredAttrExt;
use crate::attrs::resolve::ctx::AttrResolutionContext;

/// Prepare `ctx.attrs` for rule impl.
pub(crate) fn node_to_attrs_struct<'v>(
    node: ConfiguredTargetNodeRef,
    ctx: &mut dyn AttrResolutionContext<'v>,
) -> slug_error::Result<ValueOfUnchecked<'v, StructRef<'static>>> {
    let attrs_iter = node.attrs(AttrInspectOptions::All);
    let mut resolved_attrs = Vec::with_capacity(attrs_iter.size_hint().0);
    for a in attrs_iter {
        let resolved = a.value.resolve_single_for_ctx_attr(
            node.label().pkg(),
            a.attr.coercer(),
            node.label().cfg_pair(),
            ctx,
        )?;
        resolved_attrs.push((a.name, resolved));
    }
    Ok(ctx
        .heap()
        .alloc_typed_unchecked(AllocStruct(resolved_attrs))
        .cast())
}
