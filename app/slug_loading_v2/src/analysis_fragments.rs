//! Evaluator-local configured fragment facades.

use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::RetainedCppFragment;
use slug_configuration_v2::AppleFragmentProjection;
use slug_configuration_v2::CppFragmentProjection;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocImmutableList;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;
use starlark_map::small_set::SmallSet;

use crate::builtin_restriction::check_default_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;
use crate::provider::alloc_starlark_label;
use crate::subrule_invocation::AnalysisCallToken;

const ACTIVE_FRAGMENT_NAMES_EXCEPT_APPLE_CPP_AND_COVERAGE: &[&str] = &[
    "android",
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
    retained: RetainedCppFragment,
}

impl CppFragmentValue {
    pub fn new(projection: CppFragmentProjection) -> Self {
        Self {
            retained: RetainedCppFragment::new(projection),
        }
    }

    fn check(&self, eval: &Evaluator<'_, '_, '_>) -> anyhow::Result<()> {
        check_default_allowlist(eval, &source_identities_for_evaluator(eval)?)
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

pub fn cpp_fragment_value(value: Value<'_>) -> Option<RetainedCppFragment> {
    CppFragmentValue::from_value(value).map(|value| value.retained.clone())
}
pub fn alloc_frozen_cpp_fragment(heap: &FrozenHeap, retained: RetainedCppFragment) -> FrozenValue {
    heap.alloc(CppFragmentValue { retained })
}
impl StarlarkStructuralValue for CppFragmentValue {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.retained.hash(hasher);
        Ok(())
    }
}

#[starlark_module]
fn cpp_fragment_methods(builder: &mut MethodsBuilder) {
    fn minimum_os_version<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        // Public CppConfiguration method, unlike the private profile getters.
        Ok(CppFragmentValue::optional(
            this.retained.projection().minimum_os_version()?,
            eval.heap(),
        ))
    }

    fn should_generate_dotd_files(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.retained.projection().should_generate_dotd_files()?)
    }

    fn save_temps(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.retained.projection().save_temps()?)
    }

    fn process_headers_in_dependencies(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this
            .retained
            .projection()
            .process_headers_in_dependencies()?)
    }

    fn fission_active_for_current_compilation_mode(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this
            .retained
            .projection()
            .fission_active_for_current_compilation_mode()?)
    }

    fn should_strip_binaries(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.retained.projection().should_strip_binaries()?)
    }
    fn force_pic(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval).map_err(|error| {
            anyhow::anyhow!("{error} (feature 'force_pic' in CppConfiguration)")
        })?;
        Ok(this.retained.projection().force_pic()?)
    }

    fn use_llvm_coverage_map_format(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.retained.projection().use_llvm_coverage_map_format()?)
    }

    fn fdo_instrument<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval).map_err(|error| {
            anyhow::anyhow!("{error} (feature 'fdo_instrument' in CppConfiguration)")
        })?;
        Ok(CppFragmentValue::optional(
            this.retained.projection().fdo_instrument()?,
            eval.heap(),
        ))
    }

    fn cs_fdo_instrument<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.retained.projection().cs_fdo_instrument()?,
            eval.heap(),
        ))
    }

    #[starlark(attribute)]
    fn custom_malloc<'v>(this: &CppFragmentValue, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        Ok(this
            .retained
            .projection()
            .custom_malloc()?
            .map_or_else(Value::new_none, |label| alloc_starlark_label(heap, label)))
    }

    fn compilation_mode(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<String> {
        this.check(eval)?;
        Ok(this.retained.projection().compilation_mode()?.to_owned())
    }

    fn propeller_optimize_absolute_cc_profile<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.retained
                .projection()
                .propeller_optimize_absolute_cc_profile()?,
            eval.heap(),
        ))
    }

    fn propeller_optimize_absolute_ld_profile<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.retained
                .projection()
                .propeller_optimize_absolute_ld_profile()?,
            eval.heap(),
        ))
    }

    fn fdo_path<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.retained.projection().fdo_path()?,
            eval.heap(),
        ))
    }

    fn cs_fdo_path<'v>(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        this.check(eval)?;
        Ok(CppFragmentValue::optional(
            this.retained.projection().cs_fdo_path()?,
            eval.heap(),
        ))
    }

    fn proto_profile(
        this: &CppFragmentValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        this.check(eval)?;
        Ok(this.retained.projection().proto_profile()?)
    }
}

#[starlark_value(type = "cpp")]
impl<'v> StarlarkValue<'v> for CppFragmentValue {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(cpp_fragment_value(other).is_some_and(|other| self.retained == other))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.retained.hash(hasher);
        Ok(())
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // These are values, not native attribute methods: the engine's direct
        // method-call path invokes a native attribute getter without calling
        // its returned value. CppFragmentProjection::new validated the immutable
        // configuration, so these reads cannot subsequently fail conversion.
        match name {
            "copts" => Some(
                heap.alloc(AllocImmutableList(
                    self.retained
                        .projection()
                        .copts()
                        .expect("validated Cpp copts"),
                )),
            ),
            "conlyopts" => Some(
                heap.alloc(AllocImmutableList(
                    self.retained
                        .projection()
                        .conlyopts()
                        .expect("validated Cpp conlyopts"),
                )),
            ),
            "cxxopts" => Some(
                heap.alloc(AllocImmutableList(
                    self.retained
                        .projection()
                        .cxxopts()
                        .expect("validated Cpp cxxopts"),
                )),
            ),
            "linkopts" => Some(
                heap.alloc(AllocImmutableList(
                    self.retained
                        .projection()
                        .linkopts()
                        .expect("validated Cpp linkopts"),
                )),
            ),
            "apple_generate_dsym" => Some(Value::new_bool(
                self.retained
                    .projection()
                    .apple_generate_dsym()
                    .expect("validated Cpp apple_generate_dsym"),
            )),
            "_dont_enable_host_nonhost" => Some(Value::new_bool(
                self.retained
                    .projection()
                    .dont_enable_host_nonhost()
                    .expect("validated Cpp dont_enable_host_nonhost"),
            )),
            "_fdo_prefetch_hints_label" => Some(
                self.retained
                    .projection()
                    .fdo_prefetch_hints_label()
                    .expect("validated Cpp fdo_prefetch_hints_label")
                    .map_or_else(Value::new_none, |label| alloc_starlark_label(heap, label)),
            ),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        [
            "copts",
            "conlyopts",
            "cxxopts",
            "linkopts",
            "apple_generate_dsym",
            "_dont_enable_host_nonhost",
            "_fdo_prefetch_hints_label",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cpp_fragment_methods)
    }
}

#[cfg(test)]
#[path = "analysis_fragments/cc_feature_tests.rs"]
mod cc_feature_tests;

#[cfg(test)]
#[path = "analysis_fragments/compile_input_tests.rs"]
mod compile_input_tests;

#[cfg(test)]
#[path = "analysis_fragments/toolchain_provider_tests.rs"]
mod toolchain_provider_tests;

#[cfg(test)]
#[path = "analysis_fragments/apple_fragment_tests.rs"]
mod apple_fragment_tests;

/// Bazel's source-backed Apple fields for the admitted default/null path.
/// The projection validates the retained configuration before publication.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleFragmentValue {
    projection: AppleFragmentProjection,
}

impl AppleFragmentValue {
    pub fn new(projection: AppleFragmentProjection) -> Self {
        Self { projection }
    }

    fn optional<'v>(value: Option<&str>, heap: Heap<'v>) -> Value<'v> {
        value.map_or_else(Value::new_none, |value| heap.alloc_str(value).to_value())
    }
}

impl fmt::Display for AppleFragmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<apple configuration fragment>")
    }
}

starlark::starlark_simple_value!(AppleFragmentValue);

/// Bazel 9.2's Java ApplePlatform.MACOS singleton, not the distinct
/// `apple_common.platform.macos` Starlark struct. Only the default selector
/// may construct this evaluator-local value.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct DefaultMacosApplePlatform;

impl fmt::Display for DefaultMacosApplePlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("macos")
    }
}

starlark::starlark_simple_value!(DefaultMacosApplePlatform);

#[starlark_value(type = "apple_platform")]
impl<'v> StarlarkValue<'v> for DefaultMacosApplePlatform {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" | "platform_type" => Some(heap.alloc_str("macos").to_value()),
            "name_in_plist" => Some(heap.alloc_str("MacOSX").to_value()),
            "is_device" => Some(Value::new_bool(true)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        ["is_device", "name", "name_in_plist", "platform_type"]
            .map(str::to_owned)
            .into()
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "slug.apple-platform.macos".hash(hasher);
        Ok(())
    }
}

#[starlark_module]
fn apple_fragment_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn single_arch_platform<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        this.projection.single_arch_platform_is_default_macos()?;
        Ok(heap.alloc_simple(DefaultMacosApplePlatform))
    }

    #[starlark(attribute)]
    fn xcode_version_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.xcode_version_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn ios_sdk_version_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.ios_sdk_version_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn macos_sdk_version_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.macos_sdk_version_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn tvos_sdk_version_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.tvos_sdk_version_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn watchos_sdk_version_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.watchos_sdk_version_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn ios_minimum_os_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.ios_minimum_os_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn macos_minimum_os_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.macos_minimum_os_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn tvos_minimum_os_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.tvos_minimum_os_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn watchos_minimum_os_flag<'v>(
        this: &AppleFragmentValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(AppleFragmentValue::optional(
            this.projection.watchos_minimum_os_flag()?,
            heap,
        ))
    }

    #[starlark(attribute)]
    fn prefer_mutual_xcode(this: &AppleFragmentValue) -> anyhow::Result<bool> {
        Ok(this.projection.prefer_mutual_xcode()?)
    }

    #[starlark(attribute)]
    fn include_xcode_exec_requirements(this: &AppleFragmentValue) -> anyhow::Result<bool> {
        Ok(this.projection.include_xcode_exec_requirements()?)
    }
}

#[starlark_value(type = "apple")]
impl<'v> StarlarkValue<'v> for AppleFragmentValue {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(apple_fragment_methods)
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
    apple: FrozenValue,
    #[allocative(skip)]
    coverage: FrozenValue,
}

impl RuleFragmentCollection {
    pub fn new(
        token: AnalysisCallToken,
        declarations: Arc<SmallSet<CompactString>>,
        cpp: FrozenValue,
        apple: FrozenValue,
        coverage: FrozenValue,
    ) -> Self {
        Self {
            token,
            declarations,
            cpp,
            apple,
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
    fn apple<'v>(this: &RuleFragmentCollection) -> anyhow::Result<Value<'v>> {
        this.token
            .require_active("apple", "rule fragment collection")?;
        if !this.declarations.contains("apple") {
            anyhow::bail!(
                "rule has to declare 'apple' as a required fragment in order to access it"
            );
        }
        Ok(this.apple.to_value())
    }

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
        ACTIVE_FRAGMENT_NAMES_EXCEPT_APPLE_CPP_AND_COVERAGE
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
