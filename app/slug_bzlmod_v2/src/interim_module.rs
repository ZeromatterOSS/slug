use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

/// The identity a non-root module was requested under. It intentionally stays
/// separate from the `module()` declaration recorded in an evaluated file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct NonrootModuleKey {
    pub name: CompactString,
    pub version: CompactString,
}

impl NonrootModuleKey {
    pub fn new(name: impl Into<CompactString>, version: impl Into<CompactString>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

/// An identifier assigned by the module evaluator, rather than a filesystem
/// path. This keeps cached evaluated state independent of materialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct LogicalModuleFileId(pub CompactString);

impl LogicalModuleFileId {
    pub fn new(value: impl Into<CompactString>) -> Self {
        Self(value.into())
    }
}

/// A half-open logical source range. Lines and columns are one-based.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct LogicalSpan {
    pub file: LogicalModuleFileId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootDependency {
    pub name: CompactString,
    pub version: CompactString,
    max_compatibility_level: i32,
}

impl NonrootDependency {
    pub const MAX_COMPATIBILITY_LEVEL: i32 = -1;

    pub fn new(name: impl Into<CompactString>, version: impl Into<CompactString>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            max_compatibility_level: Self::MAX_COMPATIBILITY_LEVEL,
        }
    }

    pub fn max_compatibility_level(&self) -> i32 {
        self.max_compatibility_level
    }
}

/// Heap-independent Starlark values retained by module-extension and innate
/// repository-rule calls.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum NonrootAttributeValue {
    None,
    Bool(bool),
    Int(NonrootAttributeInt),
    String(CompactString),
    Label(CompactString),
    /// Starlark lists remain distinct from tuples in retained module state.
    List(Arc<[NonrootAttributeValue]>),
    /// Starlark tuples remain distinct from lists in retained module state.
    Tuple(Arc<[NonrootAttributeValue]>),
    Dict(Arc<SmallMap<NonrootAttributeKey, NonrootAttributeValue>>),
    /// The exact deferred-invalid values established by the raw-attribute
    /// oracle. These are diagnostic tokens, never evaluator values.
    Float314,
    BuiltinPrint,
    ExtensionProxy,
    SelfList,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootAttributeInt(NonrootAttributeIntRepr);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum NonrootAttributeIntRepr {
    Small(i32),
    BigDecimal(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum NonrootAttributeKey {
    String(CompactString),
    Label(CompactString),
    /// The sole non-string/non-label dictionary key retained by the oracle.
    DeferredFloat314,
}

/// An ordered, location-free projection for the later lockfile adapter phase.
/// It deliberately does not define retained semantic equality: `SmallMap`
/// remains the retained dictionary representation and keeps its
/// order-insensitive equality.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) struct NonrootAttributeAdapterProjection {
    pub attributes: Arc<[(CompactString, NonrootAttributeAdapterValue)]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) enum NonrootAttributeAdapterValue {
    None,
    Bool(bool),
    Int(NonrootAttributeInt),
    String(CompactString),
    Label(CompactString),
    Sequence(Arc<[NonrootAttributeAdapterValue]>),
    Dict(Arc<[(NonrootAttributeAdapterKey, NonrootAttributeAdapterValue)]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) enum NonrootAttributeAdapterKey {
    String(CompactString),
    Label(CompactString),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) enum NonrootAttributeAdapterError {
    ExactFloat314,
    IntegerOutsideI32,
    UnsupportedDeferredValue,
}

impl NonrootAttributeInt {
    /// Construct a canonical decimal integer without routing through a
    /// Starlark heap. Values outside `i32` remain exact decimal strings.
    pub fn from_decimal(decimal: &str) -> Result<Self, CompactString> {
        let digits = decimal.strip_prefix('-').unwrap_or(decimal);
        let negative = decimal.starts_with('-');
        if digits.is_empty()
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
            || (negative && digits == "0")
        {
            return Err(CompactString::from("integer must be canonical decimal"));
        }

        match decimal.parse::<i32>() {
            Ok(value) => Ok(Self(NonrootAttributeIntRepr::Small(value))),
            Err(_) => Ok(Self(NonrootAttributeIntRepr::BigDecimal(
                CompactString::from(decimal),
            ))),
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match &self.0 {
            NonrootAttributeIntRepr::Small(value) => Some(*value),
            NonrootAttributeIntRepr::BigDecimal(_) => None,
        }
    }

    pub fn to_decimal(&self) -> CompactString {
        match &self.0 {
            NonrootAttributeIntRepr::Small(value) => CompactString::from(value.to_string()),
            NonrootAttributeIntRepr::BigDecimal(value) => value.clone(),
        }
    }
}

impl NonrootAttributeValue {
    pub fn integer(decimal: &str) -> Result<Self, CompactString> {
        Ok(Self::Int(NonrootAttributeInt::from_decimal(decimal)?))
    }
}

/// Project a finalized compact snapshot into the ordered shape consumed by the
/// lockfile adapter. This is intentionally separate from retained semantic
/// state: it omits locations and preserves the iteration order that `SmallMap`
/// recorded for kwargs and dictionaries.
#[allow(dead_code)]
pub(crate) fn project_nonroot_attributes_for_adapter(
    attributes: &SmallMap<CompactString, NonrootAttributeValue>,
) -> Result<NonrootAttributeAdapterProjection, NonrootAttributeAdapterError> {
    attributes
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                project_nonroot_attribute_value_for_adapter(value)?,
            ))
        })
        .collect::<Result<Arc<_>, _>>()
        .map(|attributes| NonrootAttributeAdapterProjection { attributes })
}

fn project_nonroot_attribute_value_for_adapter(
    value: &NonrootAttributeValue,
) -> Result<NonrootAttributeAdapterValue, NonrootAttributeAdapterError> {
    match value {
        NonrootAttributeValue::None => Ok(NonrootAttributeAdapterValue::None),
        NonrootAttributeValue::Bool(value) => Ok(NonrootAttributeAdapterValue::Bool(*value)),
        NonrootAttributeValue::Int(value) if value.as_i32().is_some() => {
            Ok(NonrootAttributeAdapterValue::Int(value.clone()))
        }
        NonrootAttributeValue::Int(_) => Err(NonrootAttributeAdapterError::IntegerOutsideI32),
        NonrootAttributeValue::String(value) => {
            Ok(NonrootAttributeAdapterValue::String(value.clone()))
        }
        NonrootAttributeValue::Label(value) => {
            Ok(NonrootAttributeAdapterValue::Label(value.clone()))
        }
        NonrootAttributeValue::List(values) | NonrootAttributeValue::Tuple(values) => values
            .iter()
            .map(project_nonroot_attribute_value_for_adapter)
            .collect::<Result<Arc<_>, _>>()
            .map(NonrootAttributeAdapterValue::Sequence),
        NonrootAttributeValue::Dict(values) => values
            .iter()
            .map(|(key, value)| {
                let key = match key {
                    NonrootAttributeKey::String(value) => {
                        NonrootAttributeAdapterKey::String(value.clone())
                    }
                    NonrootAttributeKey::Label(value) => {
                        NonrootAttributeAdapterKey::Label(value.clone())
                    }
                    NonrootAttributeKey::DeferredFloat314 => {
                        return Err(NonrootAttributeAdapterError::ExactFloat314);
                    }
                };
                Ok((key, project_nonroot_attribute_value_for_adapter(value)?))
            })
            .collect::<Result<Arc<_>, _>>()
            .map(NonrootAttributeAdapterValue::Dict),
        NonrootAttributeValue::Float314 => Err(NonrootAttributeAdapterError::ExactFloat314),
        NonrootAttributeValue::BuiltinPrint
        | NonrootAttributeValue::ExtensionProxy
        | NonrootAttributeValue::SelfList => {
            Err(NonrootAttributeAdapterError::UnsupportedDeferredValue)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootExtensionUsage {
    pub bzl_label: CompactString,
    pub extension_name: CompactString,
    pub proxies: Arc<[NonrootExtensionProxy]>,
    pub tags: Arc<[NonrootExtensionTag]>,
    pub repo_overrides: Arc<SmallMap<CompactString, NonrootRepoOverride>>,
    pub isolation: Option<NonrootExtensionIsolationKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootExtensionProxy {
    pub proxy_name: CompactString,
    pub containing_file: LogicalModuleFileId,
    pub dev_dependency: bool,
    pub location: LogicalSpan,
    pub imports: NonrootRepoImports,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootRepoImports {
    pub local_order: Arc<[CompactString]>,
    pub local_to_exported: Arc<SmallMap<CompactString, CompactString>>,
    pub exported_to_local: Arc<SmallMap<CompactString, CompactString>>,
}

impl NonrootRepoImports {
    /// Builds both compact directions while rejecting ambiguous exports. The
    /// transient `SmallSet` keeps validation on the same compact collection
    /// family without storing duplicate state in the evaluated module.
    pub fn from_local_to_exported(
        local_to_exported: SmallMap<CompactString, CompactString>,
    ) -> Result<Self, CompactString> {
        let local_order = local_to_exported.keys().cloned().collect::<Arc<_>>();
        let mut exported_to_local = SmallMap::with_capacity(local_to_exported.len());
        let mut seen_exports = SmallSet::with_capacity(local_to_exported.len());
        for (local, exported) in local_to_exported.iter() {
            if !seen_exports.insert(exported.clone()) {
                return Err(CompactString::from(
                    "extension import exports the same name twice",
                ));
            }
            exported_to_local.insert(exported.clone(), local.clone());
        }
        Ok(Self {
            local_order,
            local_to_exported: Arc::new(local_to_exported),
            exported_to_local: Arc::new(exported_to_local),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct NonrootExtensionIsolationKey {
    pub module: NonrootModuleKey,
    pub exported_proxy_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootExtensionTag {
    pub tag_class: CompactString,
    pub attributes: Arc<SmallMap<CompactString, NonrootAttributeValue>>,
    pub dev_dependency: bool,
    pub location: LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootRepoOverride {
    pub overriding_repo_name: CompactString,
    pub must_exist: bool,
    pub location: LogicalSpan,
}

/// Bazel MODULE registration text retained exactly as declared. Parsing into
/// labels or target patterns belongs to the later owner/mapping-aware stage.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleRegistrationPattern(CompactString);

impl ModuleRegistrationPattern {
    pub fn parse(raw: &str) -> Result<Self, CompactString> {
        if raw.starts_with("//") || raw.starts_with('@') {
            Ok(Self(raw.into()))
        } else {
            Err("registration labels must be absolute target patterns".into())
        }
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ModuleRegistrationPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Module state shared by ordinary and synthetic (innate repo-rule) extension
/// usages. Synthetic rules are represented in `extension_usages`, not here.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootModuleBase {
    pub expected_key: NonrootModuleKey,
    pub declared_name: CompactString,
    pub declared_version: CompactString,
    pub repo_name: CompactString,
    pub compatibility_level: i32,
    pub bazel_compatibility: Arc<[CompactString]>,
    pub dependencies: Arc<SmallMap<CompactString, NonrootDependency>>,
    pub original_dependencies: Arc<SmallMap<CompactString, NonrootDependency>>,
    pub nodep_dependencies: Arc<[NonrootDependency]>,
    pub execution_platforms: Arc<[ModuleRegistrationPattern]>,
    pub toolchains: Arc<[ModuleRegistrationPattern]>,
    pub flag_aliases: Arc<SmallMap<CompactString, CompactString>>,
}

/// Evaluator-owned state for a non-root module. This is deliberately not
/// named `InterimModule`: registry/source provenance is added only by the
/// later discovery wrapper that constructs Bazel's final graph value.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct EvaluatedNonrootModule {
    pub base: NonrootModuleBase,
    pub extension_usages: Arc<[NonrootExtensionUsage]>,
}

/// Transient evaluator state finalized into an [`EvaluatedNonrootModule`].
/// Retained output uses compact Arc-backed collections; ordinary `Vec`s here
/// exist only while one MODULE file is being evaluated.
#[derive(Debug)]
pub struct NonrootModuleBuilder {
    pub expected_key: NonrootModuleKey,
    pub declared_name: CompactString,
    pub declared_version: CompactString,
    pub repo_name: CompactString,
    pub bazel_compatibility: Vec<CompactString>,
    pub dependencies: SmallMap<CompactString, NonrootDependency>,
    pub nodep_dependencies: Vec<NonrootDependency>,
    pub execution_platforms: Vec<ModuleRegistrationPattern>,
    pub toolchains: Vec<ModuleRegistrationPattern>,
    pub flag_aliases: SmallMap<CompactString, CompactString>,
    pub extension_usages: Vec<NonrootExtensionUsage>,
}

impl NonrootModuleBuilder {
    pub fn new(
        expected_key: NonrootModuleKey,
        declared_name: impl Into<CompactString>,
        declared_version: impl Into<CompactString>,
        repo_name: impl Into<CompactString>,
    ) -> Self {
        Self {
            expected_key,
            declared_name: declared_name.into(),
            declared_version: declared_version.into(),
            repo_name: repo_name.into(),
            bazel_compatibility: Vec::new(),
            dependencies: SmallMap::new(),
            nodep_dependencies: Vec::new(),
            execution_platforms: Vec::new(),
            toolchains: Vec::new(),
            flag_aliases: SmallMap::new(),
            extension_usages: Vec::new(),
        }
    }

    /// Applies the evaluator-owned non-root finalization invariants. Bazel 9
    /// inserts the singleton built-in only after user declarations, reports
    /// any repo-name collision, and snapshots `originalDeps` afterward.
    pub fn build(mut self) -> Result<EvaluatedNonrootModule, CompactString> {
        if self
            .extension_usages
            .iter()
            .any(|usage| !usage.repo_overrides.is_empty())
        {
            return Err(CompactString::from(
                "non-root module extension repo overrides must be empty",
            ));
        }
        self.extension_usages
            .retain(|usage| !usage.proxies.is_empty());

        const BAZEL_TOOLS: &str = "bazel_tools";
        if self.expected_key.name != BAZEL_TOOLS {
            let imported_builtin = self
                .extension_usages
                .iter()
                .flat_map(|usage| usage.proxies.iter())
                .any(|proxy| proxy.imports.local_to_exported.contains_key(BAZEL_TOOLS));
            if self.repo_name == BAZEL_TOOLS
                || self.dependencies.contains_key(BAZEL_TOOLS)
                || imported_builtin
            {
                return Err(CompactString::from(
                    "bazel_tools is a built-in dependency and its repo name is reserved",
                ));
            }
            self.dependencies.insert(
                CompactString::from(BAZEL_TOOLS),
                NonrootDependency::new(BAZEL_TOOLS, ""),
            );
        }

        let dependencies = Arc::new(self.dependencies);
        Ok(EvaluatedNonrootModule {
            base: NonrootModuleBase {
                expected_key: self.expected_key,
                declared_name: self.declared_name,
                declared_version: self.declared_version,
                repo_name: self.repo_name,
                compatibility_level: 0,
                bazel_compatibility: self.bazel_compatibility.into(),
                original_dependencies: Arc::clone(&dependencies),
                dependencies,
                nodep_dependencies: self.nodep_dependencies.into(),
                execution_platforms: self.execution_platforms.into(),
                toolchains: self.toolchains.into(),
                flag_aliases: Arc::new(self.flag_aliases),
            },
            extension_usages: self.extension_usages.into(),
        })
    }
}
