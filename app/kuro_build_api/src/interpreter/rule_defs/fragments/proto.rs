/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Protocol Buffers configuration fragment.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

// ============================================================================
// ProtoFragment - Protocol Buffers configuration fragment
// ============================================================================

/// Proto configuration fragment stub.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ProtoFragment;

impl Display for ProtoFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proto fragment")
    }
}

starlark_simple_value!(ProtoFragment);

#[starlark_value(type = "proto_fragment")]
impl<'v> StarlarkValue<'v> for ProtoFragment {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "experimental_protoc_opts"
                | "cc_proto_library_source_suffixes"
                | "cc_proto_library_header_suffixes"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "experimental_protoc_opts" => Some(heap.alloc(AllocList::EMPTY)),
            "cc_proto_library_source_suffixes" => Some(heap.alloc(vec![".pb.cc"])),
            "cc_proto_library_header_suffixes" => Some(heap.alloc(vec![".pb.h"])),
            _ => None,
        }
    }
}
