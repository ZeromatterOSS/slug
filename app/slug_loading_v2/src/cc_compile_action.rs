//! Constructor-only native compiler bridge. Pending recipes are module-rooted,
//! never raw Values in a Send+Sync action sink or a published graph value.
use std::fmt;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::CppCompileConstruction;
use slug_configuration_v2::SlugConfiguration;
use starlark::any::ProvidesStaticType;
use starlark::environment::Module;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;

#[cfg(test)]
mod tests;

pub struct CcCompileRequest<'v> {
    pub configuration: &'v SlugConfiguration,
    pub compilation_context: Value<'v>,
    pub toolchain: Value<'v>,
    pub features: Value<'v>,
    pub variables: Value<'v>,
    pub source: Value<'v>,
    pub output: Value<'v>,
    pub dotd: Value<'v>,
    pub additional_inputs: Option<Value<'v>>,
    pub additional_inputs_set: Option<Value<'v>>,
    pub cache_key_inputs: Option<Value<'v>>,
    pub build_info_headers: Option<Value<'v>>,
    pub action_name: Option<&'v str>,
    pub should_scan_includes: Option<bool>,
    pub use_pic: bool,
    pub needs_include_validation: bool,
    pub toolchain_type: &'v str,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PendingCppCompile {
    pub construction: CppCompileConstruction,
    pub normal_action_count: usize,
    pub exec_group: Option<CompactString>,
    pub additional_files: Box<[AnalysisArtifact]>,
    pub mandatory_direct: Box<[AnalysisArtifact]>,
}

/// Borrowed synchronous return only: immediately moved into Module.extra_value.
pub struct PreparedCcCompile<'v> {
    pub snapshot: PendingCppCompile,
    /// Additional set, selected tool files, non-code, declared headers, cache-only.
    pub input_depsets: [Value<'v>; 5],
}
starlark::starlark_simple_value!(PendingCppCompile);
impl fmt::Display for PendingCppCompile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<pending compiler action>")
    }
}
#[starlark_value(type = "pending_cc_compile")]
impl<'v> StarlarkValue<'v> for PendingCppCompile {}

pub(crate) fn record<'v>(
    module: &'v Module,
    pending: PreparedCcCompile<'v>,
    variables: Value<'v>,
    context: Value<'v>,
) {
    // A traced persistent stack avoids quadratic copies while recording. The
    // only traversal is the final ordered drain; no host-held moving pointers.
    let previous = crate::evaluator_roots::pending_cc_actions(module);
    let row = module.heap().alloc(AllocTuple([
        previous,
        module.heap().alloc_simple(pending.snapshot),
        variables,
        context,
        module.heap().alloc(AllocTuple(pending.input_depsets)),
    ]));
    crate::evaluator_roots::set_pending_cc_actions(module, row);
}

pub fn pending<'v>(module: &'v Module) -> Vec<(PreparedCcCompile<'v>, Value<'v>, Value<'v>)> {
    let mut head = crate::evaluator_roots::pending_cc_actions(module);
    let mut rows = Vec::new();
    while !head.is_none() {
        let row = TupleRef::from_value(head)
            .expect("native pending row")
            .content();
        rows.push((
            PreparedCcCompile {
                snapshot: PendingCppCompile::from_value(row[1])
                    .expect("native pending snapshot")
                    .clone(),
                input_depsets: TupleRef::from_value(row[4])
                    .expect("native role depsets")
                    .content()
                    .try_into()
                    .expect("five roles"),
            },
            row[2],
            row[3],
        ));
        head = row[0];
    }
    rows.reverse();
    rows
}

pub(crate) fn optional_string<'v>(
    value: Option<Value<'v>>,
    name: &str,
) -> anyhow::Result<Option<&'v str>> {
    value
        .filter(|value| !value.is_none())
        .map(|value| {
            value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("CppCompile {name} requires string or None"))
        })
        .transpose()
}
pub(crate) fn optional_bool(value: Option<Value<'_>>, name: &str) -> anyhow::Result<Option<bool>> {
    value
        .filter(|value| !value.is_none())
        .map(|value| {
            value
                .unpack_bool()
                .ok_or_else(|| anyhow::anyhow!("CppCompile {name} requires bool or None"))
        })
        .transpose()
}
pub(crate) fn none(value: Option<Value<'_>>, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.is_none_or(|v| v.is_none()),
        "CppCompile {name} is unsupported"
    );
    Ok(())
}
pub(crate) fn empty_sequence(value: Option<Value<'_>>, name: &str) -> anyhow::Result<()> {
    if let Some(value) = value {
        let empty = ListRef::from_value(value)
            .map(|v| v.is_empty())
            .or_else(|| TupleRef::from_value(value).map(|v| v.content().is_empty()))
            .or_else(|| (value.get_type() == "range").then(|| value.length().ok() == Some(0)));
        anyhow::ensure!(
            empty == Some(true),
            "CppCompile {name} requires an empty sequence"
        );
    }
    Ok(())
}
