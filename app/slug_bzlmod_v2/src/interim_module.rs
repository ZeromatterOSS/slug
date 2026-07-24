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
    Iterable(Arc<[NonrootAttributeValue]>),
    Dict(Arc<SmallMap<NonrootAttributeKey, NonrootAttributeValue>>),
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
    pub execution_platforms: Arc<[CompactString]>,
    pub toolchains: Arc<[CompactString]>,
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
    pub execution_platforms: Vec<CompactString>,
    pub toolchains: Vec<CompactString>,
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
