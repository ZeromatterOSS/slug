//! Immutable inputs of one configured rule evaluation, audited before retention.

use std::any::Any;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::size_of;

use slug_build_api_v2::AnalysisTargetIdentity;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_build_api_v2::published_value::FunctionEvaluationOwner;
use slug_build_api_v2::published_value::FunctionOwnerDomain;
use slug_build_api_v2::published_value::PublicationShape;
use slug_build_api_v2::published_value::PublishedFunctionSymbol;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_build_api_v2::published_value::StructuralFunctionOwnerPart;
use slug_loading_v2::package::StarlarkRuleImplementation;
use starlark::eval::FunctionSymbolSession;
use starlark::eval::FunctionSymbolStateRef;
use starlark::eval::inspect_function_symbol;
use starlark::values::FrozenHeap;
use starlark::values::FrozenHeapRef;

use super::cc_context::CcContextProjection;
use super::*;

mod audit;
mod audit_inputs;
use audit::Audit;

#[derive(Debug, Allocative)]
struct StableKey {
    key: ConfiguredTargetKey,
    shape: PublicationShape,
}

#[derive(Debug, PartialEq, Eq, Hash, Allocative)]
struct CallableWitness {
    symbol: PublishedFunctionSymbol,
    // Global language equality deliberately ignores this ordinal. Selecting an
    // implementation must still distinguish two closures exported to one name.
    creation_ordinal: u64,
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct DependencyMetadata {
    key: ConfiguredNodeKey,
    attribute: CompactString,
    filtered: bool,
    executable: bool,
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct ConfiguredAttributeMetadata {
    owner: Option<Arc<SubruleIdentity>>,
    user_name: Option<CompactString>,
    executable: bool,
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct ResolvedAttributeMetadata {
    declaration: CompactString,
    kind: AttributeKind,
    sequence: bool,
    value: CoercedAttributeValue,
}

#[derive(Debug, Allocative)]
struct InputValidity {
    implementation: StarlarkRuleImplementation,
    callables: Vec<CallableWitness>,
    dependencies: Vec<DependencyMetadata>,
    configured_attributes: Vec<ConfiguredAttributeMetadata>,
    resolved_attributes: Vec<ResolvedAttributeMetadata>,
    exec_groups: ConfiguredExecGroupCollection,
    platform_cpu: CompactString,
    workspace_status: Arc<WorkspaceStatusArtifacts>,
    runfiles_packages: RunfilesPackageDepset,
    cc_context: CcContextProjection,
    // One comparison state covers every provider and native attribute, including
    // aliases spanning otherwise unrelated dependency and execution-group rows.
    native_values: AnalysisValue,
    imports: Vec<PublishedValueArena>,
    retained_owners: Vec<FunctionEvaluationOwner>,
    shape: PublicationShape,
    #[allocative(skip)]
    _original_heaps: FrozenHeapRef,
}

impl PartialEq for InputValidity {
    fn eq(&self, other: &Self) -> bool {
        self.implementation == other.implementation
            && self.callables == other.callables
            && self.dependencies == other.dependencies
            && self.configured_attributes == other.configured_attributes
            && self.resolved_attributes == other.resolved_attributes
            && self.exec_groups == other.exec_groups
            && self.platform_cpu == other.platform_cpu
            && self.workspace_status == other.workspace_status
            && self.runfiles_packages == other.runfiles_packages
            && self.cc_context == other.cc_context
            && self.native_values.publication_eq(&other.native_values)
    }
}
impl Eq for InputValidity {}

impl StructuralFunctionOwnerPart for StableKey {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.key == other.key)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.key.hash(&mut state);
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
impl StructuralFunctionOwnerPart for InputValidity {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        // Equal snapshots necessarily select these exact callable witnesses.
        self.callables.hash(&mut state);
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.retained_owners
    }
}

fn witness(value: FrozenValue) -> Result<CallableWitness, String> {
    let symbol = slug_loading_v2::function_owner::snapshot_function_symbol(value.to_value())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "configured implementation requires an actual Starlark Def".to_owned())?;
    Ok(CallableWitness {
        symbol,
        creation_ordinal: inspect_function_symbol(value.to_value())
            .expect("audited Def")
            .creation_ordinal(),
    })
}

fn append_toolchain_values(
    values: &mut Vec<AnalysisValue>,
    context: &crate::result::ConfiguredActionToolchainContext,
) {
    for selected in context.rows().iter().filter_map(|row| row.selected()) {
        values.push(AnalysisValue::provider(selected.info().clone()));
        if let Some(info) = selected.template_variable_info() {
            values.push(AnalysisValue::template_variable_info(info.clone()));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    package: &LoadedPackage,
    implementation: &StarlarkRuleImplementation,
    key: &ConfiguredTargetKey,
    workspace_status: &Arc<WorkspaceStatusArtifacts>,
    dependencies: &[PreparedDependency],
    resolved_attributes: &[ResolvedRuleAttribute],
    configured_attributes: &[PreparedConfiguredAttribute],
    exec_groups: &ConfiguredExecGroupCollection,
    platform_cpu: &str,
    runfiles_packages: &RunfilesPackageDepset,
    cc_context: &CcContextProjection,
) -> Result<FunctionSymbolSession, String> {
    let mut audit = Audit::default();
    audit.row::<StableKey>()?;
    audit.key(key)?;
    let stable_shape = audit.native.intrinsic_shape();
    audit.row::<InputValidity>()?;
    audit.row::<AnalysisValue>()?; // enclosing joint tuple
    implementation.visit_owner_metadata(&mut |charge| {
        audit
            .native
            .charge_storage(charge)
            .map_err(|e| e.to_string())
    })?;
    let rule_function = implementation
        .owner_rule_function()
        .ok_or("configured implementation requires an actual Starlark Def")?;
    audit.callable(rule_function)?;
    for value in implementation.owner_subrule_functions() {
        audit.callable(value.ok_or("configured subrule requires an actual Starlark Def")?)?;
    }
    audit.dependency_inputs(dependencies, configured_attributes, resolved_attributes)?;
    audit.execution_inputs(exec_groups)?;
    audit.configuration_inputs(
        platform_cpu,
        workspace_status,
        runfiles_packages,
        cc_context,
    )?;

    // The snapshots below own separate handle arrays. The declared prior
    // owners/arenas keep their own full structural identity, and the shared
    // publication walker accounts for their payloads independently.
    let owner_count = audit.native.owners().len();
    let import_count = audit.native.imports().len();
    audit
        .native
        .charge_owner_edges(owner_count.saturating_add(import_count))
        .map_err(|e| e.to_string())?;
    audit.bytes(
        owner_count
            .saturating_mul(size_of::<FunctionEvaluationOwner>())
            .saturating_add(import_count.saturating_mul(size_of::<PublishedValueArena>())),
    )?;

    // Everything below retains immutable data only after the complete borrowed
    // audit, including all imported publication closures, has succeeded.
    let mut values = Vec::new();
    let mut dependency_metadata = Vec::new();
    for dependency in dependencies {
        let identity = match &dependency.key {
            ConfiguredNodeKey::Configured(key) => AnalysisTargetIdentity::Configured(
                crate::analysis_value::analysis_configured_key(key),
            ),
            ConfiguredNodeKey::Null(label) => AnalysisTargetIdentity::null(label.clone()),
        };
        values.push(AnalysisValue::configured_target(
            ConfiguredTargetValue::new(identity, dependency.providers.clone()),
        ));
        values.push(
            dependency
                .executable
                .as_ref()
                .map_or_else(AnalysisValue::none, |value| {
                    AnalysisValue::provider(value.to_occurrence())
                }),
        );
        dependency_metadata.push(DependencyMetadata {
            key: dependency.key.clone(),
            attribute: dependency.attribute.clone(),
            filtered: dependency.filtered,
            executable: dependency.executable.is_some(),
        });
    }
    values.extend(
        configured_attributes
            .iter()
            .map(|attribute| attribute.value.clone()),
    );
    let topology = exec_groups.toolchain_topology();
    for row in exec_groups.rows() {
        if let Some(toolchain) = row.action_context().toolchain() {
            append_toolchain_values(&mut values, toolchain);
        }
    }
    if let Some(toolchain) = topology.toolchain() {
        append_toolchain_values(&mut values, toolchain);
    }
    if let Some(toolchain) = cc_context.owner_default_execution().toolchain() {
        append_toolchain_values(&mut values, toolchain);
    }
    let mut callables = vec![witness(rule_function)?];
    for value in implementation.owner_subrule_functions() {
        callables.push(witness(
            value.ok_or("configured subrule requires an actual Starlark Def")?,
        )?);
    }
    let heaps = FrozenHeap::new();
    package.retain_bzl_heaps(&heaps);
    let validity = InputValidity {
        implementation: implementation.clone(),
        callables,
        dependencies: dependency_metadata,
        configured_attributes: configured_attributes
            .iter()
            .map(|attribute| ConfiguredAttributeMetadata {
                owner: attribute.owner.clone(),
                user_name: attribute.user_name.clone(),
                executable: attribute.executable,
            })
            .collect(),
        resolved_attributes: resolved_attributes
            .iter()
            .map(|attribute| ResolvedAttributeMetadata {
                declaration: attribute.declaration_name.clone(),
                kind: attribute.kind,
                sequence: attribute.sequence,
                value: attribute.value.clone(),
            })
            .collect(),
        exec_groups: exec_groups.clone(),
        platform_cpu: CompactString::from(platform_cpu),
        workspace_status: workspace_status.clone(),
        runfiles_packages: runfiles_packages.clone(),
        cc_context: cc_context.clone(),
        native_values: AnalysisValue::tuple(values),
        imports: audit.native.imports().to_vec(),
        retained_owners: audit.native.owners().to_vec(),
        shape: {
            let full = audit.native.intrinsic_shape();
            PublicationShape {
                nodes: full.nodes - stable_shape.nodes,
                edges: full.edges - stable_shape.edges,
                bytes: full.bytes - stable_shape.bytes,
                visits: full.visits - stable_shape.visits,
                depth: full.depth,
            }
        },
        _original_heaps: heaps.into_ref(),
    };
    let owner = FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        Arc::new(StableKey {
            key: key.clone(),
            shape: stable_shape,
        }),
        Arc::new(validity),
    )
    .map_err(|e| e.to_string())?;
    Ok(slug_loading_v2::function_owner::function_symbol_session(
        owner,
    ))
}

#[cfg(test)]
mod tests;
