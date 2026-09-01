//! Evaluator-local configured fragment facades.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_configuration_v2::CppFragmentProjection;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark_map::small_set::SmallSet;

use crate::BzlModuleIdentity;
use crate::builtin_restriction::check_default_allowlist;
use crate::provider::alloc_starlark_label;
use crate::subrule_invocation::AnalysisCallToken;

const ACTIVE_FRAGMENT_NAMES_EXCEPT_CPP_AND_COVERAGE: &[&str] = &[
    "android",
    "apple",
    "bazel_android",
    "bazel_py",
    "j2objc",
    "java",
    "objc",
    "platform",
    "proto",
    "py",
];

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CppFragmentValue {
    projection: CppFragmentProjection,
    callers: Arc<[(CompactString, BzlModuleIdentity)]>,
}

impl CppFragmentValue {
    pub fn new(
        projection: CppFragmentProjection,
        callers: Arc<[(CompactString, BzlModuleIdentity)]>,
    ) -> Self {
        Self {
            projection,
            callers,
        }
    }

    fn check(&self, eval: &Evaluator<'_, '_, '_>) -> anyhow::Result<()> {
        check_default_allowlist(eval, &self.callers)
    }

    fn optional<'v>(value: Option<&str>, heap: Heap<'v>) -> Value<'v> {
        value.map_or_else(Value::new_none, |value| heap.alloc_str(value).to_value())
    }
}

impl fmt::Display for CppFragmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<cpp configuration fragment>")
    }
}

starlark::starlark_simple_value!(CppFragmentValue);

#[starlark_module]
fn cpp_fragment_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn custom_malloc<'v>(this: &CppFragmentValue, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        Ok(this
            .projection
            .custom_malloc()?
            .map_or_else(Value::new_none, |label| alloc_starlark_label(heap, label)))
    }

    fn compilation_mode(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<String> {
        this.check(eval)?;
        Ok(this.projection.compilation_mode()?.to_owned())
    }

    fn propeller_optimize_absolute_cc_profile<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.projection.propeller_optimize_absolute_cc_profile()?,
            eval.heap(),
        ))
    }

    fn propeller_optimize_absolute_ld_profile<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.projection.propeller_optimize_absolute_ld_profile()?,
            eval.heap(),
        ))
    }

    fn fdo_path<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.projection.fdo_path()?,
            eval.heap(),
        ))
    }

    fn cs_fdo_path<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.projection.cs_fdo_path()?,
            eval.heap(),
        ))
    }

    fn proto_profile(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.projection.proto_profile()?)
    }
}

#[starlark_value(type = "cpp")]
impl<'v> StarlarkValue<'v> for CppFragmentValue {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cpp_fragment_methods)
    }
}

/// Bazel's public coverage fragment contains no caller restriction. The
/// optional canonical label is the complete configured projection.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CoverageFragmentValue {
    output_generator: Option<CanonicalLabel>,
}

impl CoverageFragmentValue {
    pub fn new(output_generator: Option<CanonicalLabel>) -> Self {
        Self { output_generator }
    }
}

impl fmt::Display for CoverageFragmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<coverage configuration fragment>")
    }
}

starlark::starlark_simple_value!(CoverageFragmentValue);

#[starlark_module]
fn coverage_fragment_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn output_generator<'v>(
        this: &CoverageFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(this
            .output_generator
            .as_ref()
            .map_or_else(Value::new_none, |label| {
                alloc_starlark_label(heap, label.clone())
            }))
    }
}

#[starlark_value(type = "coverage")]
impl<'v> StarlarkValue<'v> for CoverageFragmentValue {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(coverage_fragment_methods)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RuleFragmentCollection {
    #[allocative(skip)]
    token: AnalysisCallToken,
    declarations: Arc<SmallSet<CompactString>>,
    #[allocative(skip)]
    cpp: FrozenValue,
    #[allocative(skip)]
    coverage: FrozenValue,
}

impl RuleFragmentCollection {
    pub fn new(
        token: AnalysisCallToken,
        declarations: Arc<SmallSet<CompactString>>,
        cpp: FrozenValue,
        coverage: FrozenValue,
    ) -> Self {
        Self {
            token,
            declarations,
            cpp,
            coverage,
        }
    }
}

impl fmt::Display for RuleFragmentCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.fragments>")
    }
}

starlark::starlark_simple_value!(RuleFragmentCollection);

#[starlark_module]
fn rule_fragment_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn cpp<'v>(this: &RuleFragmentCollection) -> anyhow::Result<Value<'v>> {
        this.token
            .require_active("cpp", "rule fragment collection")?;
        if !this.declarations.contains("cpp") {
            anyhow::bail!("rule has to declare 'cpp' as a required fragment in order to access it");
        }
        Ok(this.cpp.to_value())
    }

    #[starlark(attribute)]
    fn coverage<'v>(this: &RuleFragmentCollection) -> anyhow::Result<Value<'v>> {
        this.token
            .require_active("coverage", "rule fragment collection")?;
        if !this.declarations.contains("coverage") {
            anyhow::bail!(
                "rule has to declare 'coverage' as a required fragment in order to access it"
            );
        }
        Ok(this.coverage.to_value())
    }
}

#[starlark_value(type = "fragments")]
impl<'v> StarlarkValue<'v> for RuleFragmentCollection {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(rule_fragment_methods)
    }

    fn dir_attr(&self) -> Vec<String> {
        ACTIVE_FRAGMENT_NAMES_EXCEPT_CPP_AND_COVERAGE
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct SubruleFragmentCollection {
    #[allocative(skip)]
    token: AnalysisCallToken,
    declarations: Arc<SmallSet<CompactString>>,
    #[allocative(skip)]
    cpp: FrozenValue,
    #[allocative(skip)]
    coverage: FrozenValue,
}

impl SubruleFragmentCollection {
    pub fn new(
        token: AnalysisCallToken,
        declarations: Arc<SmallSet<CompactString>>,
        cpp: FrozenValue,
        coverage: FrozenValue,
    ) -> Self {
        Self {
            token,
            declarations,
            cpp,
            coverage,
        }
    }
}

impl fmt::Display for SubruleFragmentCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<subrule ctx.fragments>")
    }
}

starlark::starlark_simple_value!(SubruleFragmentCollection);

#[starlark_value(type = "subrule_fragments")]
impl<'v> StarlarkValue<'v> for SubruleFragmentCollection {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        self.token
            .require_active(attribute, "subrule fragment collection")
            .ok()?;
        match attribute {
            "cpp" if self.declarations.contains("cpp") => Some(self.cpp.to_value()),
            "coverage" if self.declarations.contains("coverage") => Some(self.coverage.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        self.declarations.iter().map(ToString::to_string).collect()
    }
}
