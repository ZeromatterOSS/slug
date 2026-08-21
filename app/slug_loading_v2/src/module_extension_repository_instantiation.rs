/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file. You may select either.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;

use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::module_extension::HostPureModuleExtensionInvocationReceipt;
use crate::module_extension::HostPureModuleExtensionInvocations;
use crate::module_extension::HostPureModuleExtensionInvocationsError;
use crate::module_extension::HostPureModuleExtensionInvocationsKey;
use crate::module_extension::HostPureModuleExtensionInvocationsObservationError;
use crate::module_extension::HostPureModuleExtensionInvocationsObservationKey;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;
use crate::module_extension_repository_rule::RepositoryRuleCallRecord;
use crate::module_extension_repository_rule::RepositoryRuleCallValue;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostInstantiatedModuleExtensionRepositories {
    predecessor: Arc<HostPureModuleExtensionInvocations>,
    extensions: Arc<[HostInstantiatedModuleExtensionRepositoriesForRequest]>,
}

impl HostInstantiatedModuleExtensionRepositories {
    pub(crate) fn parts(
        &self,
    ) -> (
        &Arc<HostPureModuleExtensionInvocations>,
        &[HostInstantiatedModuleExtensionRepositoriesForRequest],
    ) {
        (&self.predecessor, &self.extensions)
    }

    #[cfg(test)]
    pub(crate) fn with_truncated_extensions_for_test(&self) -> Self {
        Self {
            predecessor: self.predecessor.clone(),
            extensions: self.extensions[..self.extensions.len() - 1].into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_swapped_requests_for_test(&self) -> Self {
        let mut extensions = self.extensions.to_vec();
        let first = extensions[0].request.clone();
        extensions[0].request = extensions[1].request.clone();
        extensions[1].request = first;
        Self {
            predecessor: self.predecessor.clone(),
            extensions: extensions.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostInstantiatedModuleExtensionRepositoriesForRequest {
    request: HostSelectedExtensionDefinitionLoadRequest,
    mapping_entries: Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    repositories: Arc<[HostInstantiatedModuleExtensionRepository]>,
}

impl HostInstantiatedModuleExtensionRepositoriesForRequest {
    pub(crate) fn parts(
        &self,
    ) -> (
        &HostSelectedExtensionDefinitionLoadRequest,
        &[HostInstantiatedModuleExtensionRepository],
    ) {
        (&self.request, &self.repositories)
    }

    pub(crate) fn mapping_entries(&self) -> &Arc<SmallMap<ApparentRepoName, CanonicalRepoName>> {
        &self.mapping_entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostInstantiatedModuleExtensionRepository {
    generated_name: CompactString,
    canonical_name: CanonicalRepoName,
    call: RepositoryRuleCallRecord,
    repo_spec: RepoSpec,
}

impl HostInstantiatedModuleExtensionRepository {
    pub(crate) fn generated_name(&self) -> &str {
        &self.generated_name
    }

    pub(crate) fn spec_parts(&self) -> (&CanonicalRepoName, &RepoSpec) {
        (&self.canonical_name, &self.repo_spec)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostInstantiatedModuleExtensionRepositoriesError {
    Invocations(HostPureModuleExtensionInvocationsError),
    InvocationsCompute(CompactString),
    AfterInvocations {
        predecessor: Arc<HostPureModuleExtensionInvocations>,
        completed: Arc<[HostInstantiatedModuleExtensionRepositoriesForRequest]>,
        request: Option<HostSelectedExtensionDefinitionLoadRequest>,
        current: Arc<[HostInstantiatedModuleExtensionRepository]>,
        call: Option<RepositoryRuleCallRecord>,
        error: HostInstantiatedModuleExtensionRepositoryError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostInstantiatedModuleExtensionRepositoryError {
    Join(CompactString),
    Namespace(CompactString),
    Attribute(CompactString),
}

impl fmt::Display for HostInstantiatedModuleExtensionRepositoriesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostInstantiatedModuleExtensionRepositoriesError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostInstantiatedModuleExtensionRepositoriesKey {
    workspace: NormalizedAbsolutePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling; a later packet owns consumer activation.
struct HostInstantiatedModuleExtensionRepositoriesObservationKey(
    HostInstantiatedModuleExtensionRepositoriesKey,
);

#[allow(dead_code)]
impl HostInstantiatedModuleExtensionRepositoriesObservationKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostInstantiatedModuleExtensionRepositoriesKey::new(
            workspace,
        ))
    }
}

#[rustfmt::skip]
impl fmt::Display for HostInstantiatedModuleExtensionRepositoriesObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) }
}

impl HostInstantiatedModuleExtensionRepositoriesKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostInstantiatedModuleExtensionRepositoriesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-instantiated-module-extension-repositories:{}",
            self.workspace
        )
    }
}

type InstantiatedRepositoriesOutcome = SourcePreparationOutcome<InstantiatedRepositoriesResult>;

type InstantiatedRepositoriesResult = Arc<
    Result<
        HostInstantiatedModuleExtensionRepositories,
        HostInstantiatedModuleExtensionRepositoriesError,
    >,
>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed sibling.
struct ObservedHostInstantiatedModuleExtensionRepositories {
    result: InstantiatedRepositoriesResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostInstantiatedModuleExtensionRepositories {
    fn result(&self) -> &InstantiatedRepositoriesResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostInstantiatedModuleExtensionRepositoriesObservationError {
    Pure(HostPureModuleExtensionInvocationsObservationError),
}

type InstantiatedRepositoriesDriverOutcome = SourcePreparationOutcome<
    Result<
        (InstantiatedRepositoriesResult, PathObservationEpoch),
        HostInstantiatedModuleExtensionRepositoriesObservationError,
    >,
>;

#[derive(Clone, Copy)]
enum InstantiatedRepositoriesMode {
    Legacy,
    Observed,
}

fn complete(
    value: Result<
        HostInstantiatedModuleExtensionRepositories,
        HostInstantiatedModuleExtensionRepositoriesError,
    >,
    observations: PathObservationEpoch,
) -> InstantiatedRepositoriesDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

#[rustfmt::skip]
async fn compute_instantiated_repositories(ctx: &mut DiceComputations<'_>, key: &HostInstantiatedModuleExtensionRepositoriesKey, mode: InstantiatedRepositoriesMode) -> InstantiatedRepositoriesDriverOutcome {
    let child = match mode {
        InstantiatedRepositoriesMode::Legacy => match ctx.compute(&HostPureModuleExtensionInvocationsKey::new(key.workspace.dupe())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => (result, PathObservationEpoch::empty()),
            Err(error) => return complete(Err(HostInstantiatedModuleExtensionRepositoriesError::InvocationsCompute(error.to_string().into())), PathObservationEpoch::empty()),
        },
        InstantiatedRepositoriesMode::Observed => match ctx.compute(&HostPureModuleExtensionInvocationsObservationKey::new(key.workspace.dupe())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostInstantiatedModuleExtensionRepositoriesObservationError::Pure(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().dupe(), observed.observations().dupe()),
            Err(error) => return complete(Err(HostInstantiatedModuleExtensionRepositoriesError::InvocationsCompute(error.to_string().into())), PathObservationEpoch::empty()),
        },
    };
    let (result, observations) = child;
    let predecessor = match result.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => return complete(Err(HostInstantiatedModuleExtensionRepositoriesError::Invocations(error.clone())), observations),
    };
    complete(instantiate_repositories(predecessor), observations)
}

#[async_trait]
impl Key for HostInstantiatedModuleExtensionRepositoriesKey {
    type Value = InstantiatedRepositoriesOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_instantiated_repositories(ctx, self, InstantiatedRepositoriesMode::Legacy)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy instantiation has no observed outer")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostInstantiatedModuleExtensionRepositoriesObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostInstantiatedModuleExtensionRepositories,
            HostInstantiatedModuleExtensionRepositoriesObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_instantiated_repositories(
            ctx,
            &self.0,
            InstantiatedRepositoriesMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostInstantiatedModuleExtensionRepositories {
                        result,
                        observations,
                    },
                ))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn instantiate_repositories(
    predecessor: Arc<HostPureModuleExtensionInvocations>,
) -> Result<
    HostInstantiatedModuleExtensionRepositories,
    HostInstantiatedModuleExtensionRepositoriesError,
> {
    let after = |completed: &[HostInstantiatedModuleExtensionRepositoriesForRequest],
                 request: Option<&HostSelectedExtensionDefinitionLoadRequest>,
                 current: &[HostInstantiatedModuleExtensionRepository],
                 call: Option<&RepositoryRuleCallRecord>,
                 error| {
        HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
            predecessor: predecessor.clone(),
            completed: completed.into(),
            request: request.cloned(),
            current: current.into(),
            call: call.cloned(),
            error,
        }
    };
    if predecessor.prepared.inputs.len() != predecessor.invoked.len() {
        return Err(after(
            &[],
            None,
            &[],
            None,
            HostInstantiatedModuleExtensionRepositoryError::Join(
                "prepared and invoked extension counts differ".into(),
            ),
        ));
    }

    let mut completed = Vec::new();
    for (input, receipt) in predecessor
        .prepared
        .inputs
        .iter()
        .zip(predecessor.invoked.iter())
    {
        let expected = input.input.parts().0;
        if expected != &receipt.request {
            return Err(after(
                &completed,
                Some(&receipt.request),
                &[],
                None,
                HostInstantiatedModuleExtensionRepositoryError::Join(
                    "prepared and invoked extension requests differ".into(),
                ),
            ));
        }
        let mapping = namespace_mapping(receipt).map_err(|message| {
            after(
                &completed,
                Some(&receipt.request),
                &[],
                None,
                HostInstantiatedModuleExtensionRepositoryError::Namespace(message),
            )
        })?;
        let (unique_name, _, _, _) = receipt.request.namespace_parts();
        let mut current = Vec::new();
        for call in receipt.repository_rule_calls.iter() {
            let canonical_name = generated_repo(unique_name, &call.name).map_err(|message| {
                after(
                    &completed,
                    Some(&receipt.request),
                    &current,
                    Some(call),
                    HostInstantiatedModuleExtensionRepositoryError::Namespace(message),
                )
            })?;
            let repo_spec = instantiate_call(call, &mapping).map_err(|message| {
                after(
                    &completed,
                    Some(&receipt.request),
                    &current,
                    Some(call),
                    HostInstantiatedModuleExtensionRepositoryError::Attribute(message),
                )
            })?;
            current.push(HostInstantiatedModuleExtensionRepository {
                generated_name: call.name.clone(),
                canonical_name,
                call: call.clone(),
                repo_spec,
            });
        }
        completed.push(HostInstantiatedModuleExtensionRepositoriesForRequest {
            request: receipt.request.clone(),
            mapping_entries: mapping.1.clone(),
            repositories: current.into(),
        });
    }
    Ok(HostInstantiatedModuleExtensionRepositories {
        predecessor,
        extensions: completed.into(),
    })
}

fn generated_repo(
    unique_name: &CanonicalRepoName,
    name: &str,
) -> Result<CanonicalRepoName, CompactString> {
    CanonicalRepoName::new(format!("{}+{name}", unique_name.as_str())).map_err(Into::into)
}

fn namespace_mapping(
    receipt: &HostPureModuleExtensionInvocationReceipt,
) -> Result<
    (
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
    CompactString,
> {
    let (unique_name, context_repo, base, overrides) = receipt.request.namespace_parts();
    let mut entries = base.clone();
    for call in receipt.repository_rule_calls.iter() {
        let apparent = ApparentRepoName::new(call.name.as_str()).map_err(CompactString::from)?;
        entries.insert(apparent, generated_repo(unique_name, &call.name)?);
    }
    for override_value in overrides {
        let (generated, replacement, _) = override_value.parts();
        let apparent = ApparentRepoName::new(generated).map_err(CompactString::from)?;
        entries.insert(apparent, replacement.clone());
    }
    Ok((context_repo.clone(), Arc::new(entries)))
}

fn instantiate_call(
    call: &RepositoryRuleCallRecord,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<RepoSpec, CompactString> {
    let mut converted = SmallMap::new();
    for (name, raw) in call.kwargs.iter() {
        if is_legacy(name) || matches!(raw, RepositoryRuleCallValue::None) {
            continue;
        }
        let attribute = call
            .definition
            .attributes
            .iter()
            .find(|attribute| attribute.name == *name)
            .ok_or_else(|| CompactString::from(format!("unknown attribute '{name}' provided")))?;
        converted.insert(
            name.clone(),
            convert_supplied(attribute, raw, &call.definition.defining_label, mapping)?,
        );
    }
    for attribute in call.definition.attributes.iter() {
        if converted.contains_key(&attribute.name) {
            continue;
        }
        if attribute.mandatory {
            return Err(format!(
                "mandatory attribute '{}' isn't being specified",
                attribute.name
            )
            .into());
        }
        if let Some(default) = attribute.default.as_ref() {
            validate_default(attribute, default, &call.definition.defining_label, mapping)?;
        }
    }
    Ok(RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: call.definition.defining_label.clone(),
            rule_name: call.definition.exported_name.clone(),
        },
        attributes: Arc::new(converted),
    })
}

fn is_legacy(name: &str) -> bool {
    matches!(name, "name" | "tags" | "deprecation" | "visibility")
}

fn convert_supplied(
    attribute: &RepositoryRuleAttribute,
    raw: &RepositoryRuleCallValue,
    defining_label: &CanonicalLabel,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<OverrideAttributeValue, CompactString> {
    match (attribute.kind, raw) {
        (AttributeKind::String, RepositoryRuleCallValue::String(value)) => {
            Ok(OverrideAttributeValue::String(value.clone()))
        }
        (AttributeKind::Boolean, RepositoryRuleCallValue::Bool(value)) => {
            Ok(OverrideAttributeValue::Bool(*value))
        }
        (AttributeKind::Integer, RepositoryRuleCallValue::Int(value)) => {
            Ok(OverrideAttributeValue::Int(*value))
        }
        (AttributeKind::Label, RepositoryRuleCallValue::String(value)) => {
            resolve_label(value, defining_label, mapping).map(OverrideAttributeValue::Label)
        }
        (AttributeKind::Label, RepositoryRuleCallValue::Label(value)) => {
            ensure_visible(value, defining_label, mapping)?;
            Ok(OverrideAttributeValue::Label(value.clone()))
        }
        _ => Err(format!(
            "unsupported value for repository-rule {:?} attribute",
            attribute.kind
        )
        .into()),
    }
}

fn validate_default(
    attribute: &RepositoryRuleAttribute,
    default: &CoercedAttributeValue,
    defining_label: &CanonicalLabel,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<(), CompactString> {
    match (attribute.kind, default) {
        (AttributeKind::String, CoercedAttributeValue::String(_))
        | (AttributeKind::Boolean, CoercedAttributeValue::Boolean(_))
        | (AttributeKind::Integer, CoercedAttributeValue::Integer(_))
        | (AttributeKind::Label, CoercedAttributeValue::None) => Ok(()),
        (AttributeKind::Label, CoercedAttributeValue::Label(label)) => {
            ensure_visible(label, defining_label, mapping)
        }
        _ => Err(format!(
            "default for repository-rule attribute '{}' has the wrong kind",
            attribute.name
        )
        .into()),
    }
}

fn resolve_label(
    raw: &str,
    defining_label: &CanonicalLabel,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<CanonicalLabel, CompactString> {
    let defining_package = defining_label.package().package().as_str();
    let spelling = if let Some(target) = raw.strip_prefix(':') {
        format!("//{defining_package}:{target}")
    } else if !raw.starts_with('@') && !raw.starts_with("//") {
        format!("//{defining_package}:{raw}")
    } else {
        raw.to_owned()
    };
    let apparent = ApparentLabel::parse(&spelling).map_err(CompactString::from)?;
    let repository = if apparent.repo().is_root() {
        defining_label.package().repo()
    } else {
        mapping.1.get(apparent.repo()).ok_or_else(|| {
            CompactString::from(format!(
                "no repository visible as '@{}'",
                apparent.repo().as_str()
            ))
        })?
    };
    let canonical = if repository.is_root() {
        format!("@@//{}:{}", apparent.package(), apparent.target())
    } else {
        format!(
            "@@{}//{}:{}",
            repository.as_str(),
            apparent.package(),
            apparent.target()
        )
    };
    CanonicalLabel::parse(&canonical).map_err(Into::into)
}

fn ensure_visible(
    label: &CanonicalLabel,
    defining_label: &CanonicalLabel,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<(), CompactString> {
    let repository = label.package().repo();
    if repository == defining_label.package().repo()
        || mapping.1.values().any(|candidate| candidate == repository)
    {
        Ok(())
    } else {
        Err(format!(
            "no repository visible as '@{}', but referenced by label '{}'",
            repository.as_str(),
            label
        )
        .into())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::module_extension::HostPureModuleExtensionInvocationsObservationError;
    use crate::module_extension::HostPureModuleExtensionInvocationsObservationKey;
    use crate::module_extension::ObservedHostPureModuleExtensionInvocations;
    use crate::module_extension_repository_validation::HostValidatedModuleExtensionRepositoriesKey;

    #[test]
    #[rustfmt::skip]
    fn pure_observation_surface_is_instantiation_sibling_usable() {
        let key = HostPureModuleExtensionInvocationsObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap());
        assert_eq!(key.to_string(), "observed-host-pure-module-extension-invocations:\"/workspace\"");

        fn inspect(_value: &<HostPureModuleExtensionInvocationsObservationKey as Key>::Value, observed: &ObservedHostPureModuleExtensionInvocations, _error: &HostPureModuleExtensionInvocationsObservationError) {
            let _: &Arc<Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>> = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }

        let _ = inspect as fn(&SourcePreparationOutcome<Result<ObservedHostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsObservationError>>, &ObservedHostPureModuleExtensionInvocations, &HostPureModuleExtensionInvocationsObservationError);
    }

    fn schema(
        name: &str,
        kind: AttributeKind,
        mandatory: bool,
        default: Option<CoercedAttributeValue>,
    ) -> RepositoryRuleAttribute {
        RepositoryRuleAttribute {
            name: name.into(),
            kind,
            mandatory,
            default,
        }
    }

    fn call(
        schema: impl IntoIterator<Item = RepositoryRuleAttribute>,
        kwargs: impl IntoIterator<Item = (&'static str, RepositoryRuleCallValue)>,
    ) -> RepositoryRuleCallRecord {
        RepositoryRuleCallRecord {
            definition:
                crate::module_extension_repository_rule::RepositoryRuleDefinitionProjection {
                    defining_label: CanonicalLabel::parse("@@//defs:repo.bzl").unwrap(),
                    exported_name: "repo".into(),
                    attributes: schema.into_iter().collect(),
                },
            name: "generated".into(),
            kwargs: kwargs
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
            caller: crate::module_extension_repository_rule::RepositoryRuleCallSpan {
                file: "//:ext.bzl".into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
            },
            stack: Arc::from([]),
        }
    }

    fn mapping() -> (
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ) {
        (
            CanonicalRepoName::root(),
            Arc::new(SmallMap::from_iter([(
                ApparentRepoName::new("dep").unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            )])),
        )
    }

    #[test]
    fn pure_instantiation_stores_only_explicit_values_in_raw_order() {
        let call = call(
            [
                schema("text", AttributeKind::String, true, None),
                schema(
                    "enabled",
                    AttributeKind::Boolean,
                    false,
                    Some(CoercedAttributeValue::Boolean(true)),
                ),
                schema(
                    "count",
                    AttributeKind::Integer,
                    false,
                    Some(CoercedAttributeValue::Integer(3)),
                ),
                schema(
                    "target",
                    AttributeKind::Label,
                    false,
                    Some(CoercedAttributeValue::None),
                ),
            ],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("count", RepositoryRuleCallValue::Int(7)),
                ("text", RepositoryRuleCallValue::String("value".into())),
                ("target", RepositoryRuleCallValue::None),
                ("tags", RepositoryRuleCallValue::String("ignored".into())),
            ],
        );
        let spec = instantiate_call(&call, &mapping()).unwrap();
        assert_eq!(spec.rule_id.bzl_file.to_string(), "@@//defs:repo.bzl");
        assert_eq!(spec.rule_id.rule_name, "repo");
        assert_eq!(
            spec.attributes
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["count", "text"]
        );
        assert_eq!(
            spec.attributes.get("count"),
            Some(&OverrideAttributeValue::Int(7))
        );
        assert_eq!(
            spec.attributes.get("text"),
            Some(&OverrideAttributeValue::String("value".into()))
        );
        assert!(!spec.attributes.contains_key("name"));
        assert!(!spec.attributes.contains_key("enabled"));
        assert!(!spec.attributes.contains_key("target"));
    }

    #[test]
    fn pure_instantiation_preserves_two_phase_errors_and_label_semantics() {
        let attributes = [
            schema("required", AttributeKind::String, true, None),
            schema("target", AttributeKind::Label, false, None),
        ];
        let unknown = call(
            attributes.clone(),
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("unknown", RepositoryRuleCallValue::String("x".into())),
            ],
        );
        assert!(
            instantiate_call(&unknown, &mapping())
                .unwrap_err()
                .contains("unknown attribute")
        );

        let wrong = call(
            attributes.clone(),
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("target", RepositoryRuleCallValue::Bool(true)),
            ],
        );
        assert!(
            instantiate_call(&wrong, &mapping())
                .unwrap_err()
                .contains("unsupported value")
        );

        let missing = call(
            attributes.clone(),
            [("name", RepositoryRuleCallValue::String("generated".into()))],
        );
        assert!(
            instantiate_call(&missing, &mapping())
                .unwrap_err()
                .contains("mandatory attribute 'required'")
        );

        for (raw, expected) in [
            (":local", "@@//defs:local"),
            ("other", "@@//defs:other"),
            ("@dep//pkg:item", "@@dep+//pkg:item"),
        ] {
            let labeled = call(
                attributes.clone(),
                [
                    ("name", RepositoryRuleCallValue::String("generated".into())),
                    ("required", RepositoryRuleCallValue::String("ok".into())),
                    ("target", RepositoryRuleCallValue::String(raw.into())),
                ],
            );
            assert_eq!(
                labeled.repo_spec(&mapping()).attributes.get("target"),
                Some(&OverrideAttributeValue::Label(
                    CanonicalLabel::parse(expected).unwrap()
                ))
            );
        }
        let canonical = call(
            attributes,
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("required", RepositoryRuleCallValue::String("ok".into())),
                (
                    "target",
                    RepositoryRuleCallValue::Label(
                        CanonicalLabel::parse("@@dep+//pkg:item").unwrap(),
                    ),
                ),
            ],
        );
        assert!(instantiate_call(&canonical, &mapping()).is_ok());

        let invisible_default = call(
            [
                schema("required", AttributeKind::String, true, None),
                schema(
                    "target",
                    AttributeKind::Label,
                    false,
                    Some(CoercedAttributeValue::Label(
                        CanonicalLabel::parse("@@hidden+//:item").unwrap(),
                    )),
                ),
            ],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("required", RepositoryRuleCallValue::String("ok".into())),
            ],
        );
        assert!(
            instantiate_call(&invisible_default, &mapping())
                .unwrap_err()
                .contains("no repository visible")
        );
        let mandatory_first = call(
            [
                schema("required", AttributeKind::String, true, None),
                schema(
                    "target",
                    AttributeKind::Label,
                    false,
                    Some(CoercedAttributeValue::Label(
                        CanonicalLabel::parse("@@hidden+//:item").unwrap(),
                    )),
                ),
            ],
            [("name", RepositoryRuleCallValue::String("generated".into()))],
        );
        assert!(
            instantiate_call(&mandatory_first, &mapping())
                .unwrap_err()
                .contains("mandatory attribute 'required'")
        );
    }

    trait TestCall {
        fn repo_spec(
            &self,
            mapping: &(
                CanonicalRepoName,
                Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
            ),
        ) -> RepoSpec;
    }

    impl TestCall for RepositoryRuleCallRecord {
        fn repo_spec(
            &self,
            mapping: &(
                CanonicalRepoName,
                Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
            ),
        ) -> RepoSpec {
            instantiate_call(self, mapping).unwrap()
        }
    }

    pub(crate) const WORKSPACE: &str = "/module-extension-repository-instantiation";

    #[derive(Debug)]
    struct InstantiationActivation {
        key: String,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct InstantiationTracker {
        activations: Mutex<Vec<InstantiationActivation>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl InstantiationTracker {
        fn take(&self) -> Vec<InstantiationActivation> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }

        fn take_dependencies(&self) -> Vec<(String, Vec<String>)> {
            std::mem::take(&mut *self.dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for InstantiationTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.activations
                .lock()
                .unwrap()
                .push(InstantiationActivation {
                    key: key.to_string(),
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
        }
    }

    fn instantiation_base_observations(
        module_source: &str,
        metadata_bias: i64,
    ) -> Vec<(PathObservationDemand, PathObservationResult)> {
        ["/", WORKSPACE]
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::Directory,
                        index as i64 + 1 + metadata_bias,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                )
            })
            .chain(
                ["REPO.bazel", ".bazelignore", "BUILD", "MODULE.bazel.lock"]
                    .into_iter()
                    .map(|name| {
                        (
                            PathObservationDemand::new(
                                PathObservationNamespace::Host,
                                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                                PathObservationOperation::Lstat,
                            ),
                            PathObservationResult::Lstat(PathOperationResult::Missing),
                        )
                    }),
            )
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/MODULE.bazel")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    9 + metadata_bias,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/MODULE.bazel")).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    module_source.as_bytes(),
                ))),
            )))
            .collect()
    }
    async fn transaction(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        metadata_bias: i64,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut user_data = UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module_source.to_owned())),
                        ),
                        (
                            workspace.as_path().join("ext.bzl"),
                            if extension_present {
                                WorkspaceFileValue::Present(Arc::new(extension_source.to_owned()))
                            } else {
                                WorkspaceFileValue::Absent
                            },
                        ),
                        (
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let observations = instantiation_base_observations(module_source, metadata_bias)
            .into_iter()
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/BUILD.bazel")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    10 + metadata_bias,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(if extension_present {
                    PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        11 + metadata_bias,
                        1,
                        1,
                        1,
                        0o644,
                    ))
                } else {
                    PathOperationResult::Missing
                }),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(if extension_present {
                    PathOperationResult::Present(Arc::from(extension_source.as_bytes()))
                } else {
                    PathOperationResult::Missing
                }),
            )));
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations).unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    pub(crate) async fn transaction_untracked(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
    ) -> dice::DiceTransaction {
        transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            0,
            None,
        )
        .await
    }

    pub(crate) async fn transaction_with_tracker(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        tracker: Arc<dyn ActivationTracker>,
    ) -> dice::DiceTransaction {
        transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            0,
            Some(tracker),
        )
        .await
    }

    async fn compute(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
    ) -> InstantiatedRepositoriesOutcome {
        transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            0,
            None,
        )
        .await
        .compute(&HostInstantiatedModuleExtensionRepositoriesKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        ))
        .await
        .unwrap()
    }

    async fn compute_observed(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        metadata_bias: i64,
        tracker: Option<Arc<InstantiationTracker>>,
    ) -> <HostInstantiatedModuleExtensionRepositoriesObservationKey as Key>::Value {
        transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            metadata_bias,
            tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        )
        .await
        .compute(
            &HostInstantiatedModuleExtensionRepositoriesObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ),
        )
        .await
        .unwrap()
    }

    fn observed_carrier(
        value: &<HostInstantiatedModuleExtensionRepositoriesObservationKey as Key>::Value,
    ) -> &ObservedHostInstantiatedModuleExtensionRepositories {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed instantiation carrier: {value:?}"),
        }
    }
    #[tokio::test]
    async fn real_key_builds_complete_namespace_and_restores_a_b_a() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = |target: &str| {
            format!(
                "module(name='bazel_tools')\n\
                 e=use_extension('//:ext.bzl','ext')\n\
                 use_repo(e, replacement='replacement')\n\
                 override_repo(e, second='{target}')\n"
            )
        };
        let source = |value: &str| {
            format!(
                r#"repo=repository_rule(lambda ctx: None, attrs={{'text':attr.string(mandatory=True),'target':attr.label(),'peer':attr.label()}})
def impl(ctx):
    repo(name='first', text='{value}')
    repo(name='second', text='two', target='@second//:item', peer='@first//:item')
ext=module_extension(implementation=impl)
"#
            )
        };
        let a = compute(&dice, &module("replacement"), &source("one"), true).await;
        let warm = compute(&dice, &module("replacement"), &source("one"), true).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &warm
        ));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("instantiation must complete")
        };
        let a_value = a_value.as_ref().as_ref().unwrap();
        assert_eq!(a_value.extensions.len(), 1);
        assert_eq!(a_value.extensions[0].repositories.len(), 2);
        let rows = &a_value.extensions[0].repositories;
        assert_eq!(rows[0].generated_name, "first");
        assert_eq!(rows[0].canonical_name.as_str(), "+ext+first");
        assert_eq!(rows[1].canonical_name.as_str(), "+ext+second");
        let replacement = a_value.extensions[0].request.namespace_parts().3[0]
            .parts()
            .1
            .clone();
        assert_eq!(
            rows[1].repo_spec.attributes.get("target"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse(&format!("@@{}//:item", replacement.as_str())).unwrap()
            ))
        );
        assert_eq!(
            rows[1].repo_spec.attributes.get("peer"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@+ext+first//:item").unwrap()
            ))
        );
        assert_eq!(
            a_value.extensions[0].request.namespace_parts().3[0]
                .parts()
                .0,
            "second"
        );

        let b = compute(&dice, &module("replacement"), &source("changed"), true).await;
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &b
        ));
        let restored = compute(&dice, &module("replacement"), &source("one"), true).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &restored
        ));
        let empty_source = "def impl(ctx):\n    pass\next=module_extension(implementation=impl)\n";
        let empty = compute(&dice, &module("replacement"), empty_source, true).await;
        let SourcePreparationOutcome::Complete(empty_value) = &empty else {
            panic!("empty instantiation must complete")
        };
        assert!(
            empty_value.as_ref().as_ref().unwrap().extensions[0]
                .repositories
                .is_empty()
        );

        let failing_source = r#"repo=repository_rule(lambda ctx: None, attrs={'text':attr.string(mandatory=True)})
def impl(ctx):
    repo(name='first', text='one')
    repo(name='second', text='two', unknown='bad')
ext=module_extension(implementation=impl)
"#;
        let failed = compute(&dice, &module("replacement"), failing_source, true).await;
        let SourcePreparationOutcome::Complete(failed_value) = &failed else {
            panic!("attribute failure must complete")
        };
        assert!(matches!(
            failed_value.as_ref(),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                completed,
                current,
                call: Some(call),
                error: HostInstantiatedModuleExtensionRepositoryError::Attribute(message),
                ..
            }) if completed.is_empty()
                && current.len() == 1
                && call.name == "second"
                && message.contains("unknown attribute")
        ));
        let two_module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first_ext')\nb=use_extension('//:ext.bzl','second_ext')\n";
        let two_source = r#"repo=repository_rule(lambda ctx: None, attrs={'text':attr.string(mandatory=True)})
def first_impl(ctx):
    repo(name='first', text='one')
first_ext=module_extension(implementation=first_impl)
def second_impl(ctx):
    repo(name='second', text='two')
    repo(name='third', text='three', unknown='bad')
second_ext=module_extension(implementation=second_impl)
"#;
        let failed = compute(&dice, two_module, two_source, true).await;
        assert!(matches!(
            failed,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                        completed,
                        current,
                        call: Some(call),
                        error: HostInstantiatedModuleExtensionRepositoryError::Attribute(message),
                        ..
                    }) if completed.len() == 1
                        && completed[0].repositories[0].generated_name == "first"
                        && current.len() == 1
                        && current[0].generated_name == "second"
                        && call.name == "third"
                        && message.contains("unknown attribute")
                )
        ));
        let missing = compute(&dice, &module("replacement"), &source("one"), false).await;
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostInstantiatedModuleExtensionRepositoriesError::Invocations(_)))
        ));

        let predecessor = a_value.predecessor.clone();
        let count_mismatch = Arc::new(HostPureModuleExtensionInvocations {
            prepared: predecessor.prepared.clone(),
            invoked: Arc::from([]),
        });
        assert!(matches!(
            instantiate_repositories(count_mismatch),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                error: HostInstantiatedModuleExtensionRepositoryError::Join(message),
                ..
            }) if message.contains("counts differ")
        ));

        let alternate_module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, alias='replacement')\noverride_repo(e, second='alias')\n";
        let alternate = compute(&dice, alternate_module, &source("one"), true).await;
        let SourcePreparationOutcome::Complete(alternate_value) = alternate else {
            panic!("alternate request must complete")
        };
        let alternate_value = alternate_value.as_ref().as_ref().unwrap();
        let request_mismatch = Arc::new(HostPureModuleExtensionInvocations {
            prepared: predecessor.prepared.clone(),
            invoked: alternate_value.predecessor.invoked.clone(),
        });
        assert!(matches!(
            instantiate_repositories(request_mismatch),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                error: HostInstantiatedModuleExtensionRepositoryError::Join(message),
                ..
            }) if message.contains("requests differ")
        ));

        let mut bad_call = predecessor.invoked[0].repository_rule_calls[0].clone();
        bad_call.name = "bad/name".into();
        bad_call.kwargs = Arc::from([(
            CompactString::from("unknown"),
            RepositoryRuleCallValue::String("bad".into()),
        )]);
        let invalid_namespace = Arc::new(HostPureModuleExtensionInvocations {
            prepared: predecessor.prepared.clone(),
            invoked: Arc::from([HostPureModuleExtensionInvocationReceipt {
                request: predecessor.invoked[0].request.clone(),
                repository_rule_calls: Arc::from([bad_call]),
            }]),
        });
        assert!(matches!(
            instantiate_repositories(invalid_namespace),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                current,
                error: HostInstantiatedModuleExtensionRepositoryError::Namespace(_),
                ..
            }) if current.is_empty()
        ));
    }

    #[tokio::test]
    async fn real_key_need_reuse_events_and_identity_matrix_are_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(InstantiationTracker::default());
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let source = "repo=repository_rule(lambda ctx: None, attrs={'value':attr.string()})\ndef impl(ctx):\n    repo(name='generated', value='A')\next=module_extension(implementation=impl)\n";
        let compute_tracked = |module_source: &str, extension_source: &str| {
            let dice = dice.clone();
            let tracker = tracker.clone();
            let module_source = module_source.to_owned();
            let extension_source = extension_source.to_owned();
            async move {
                transaction(
                    &dice,
                    &module_source,
                    &extension_source,
                    true,
                    0,
                    Some(tracker),
                )
                .await
                .compute(&HostInstantiatedModuleExtensionRepositoriesKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                ))
                .await
                .unwrap()
            }
        };
        let a = compute_tracked(module, source).await;
        let warm = compute_tracked(module, source).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &warm
        ));
        let activations = tracker.take();
        assert!(activations.iter().any(|row| {
            row.key
                .starts_with("host-instantiated-module-extension-repositories:")
                && row.kind == ActivationKind::Evaluated
                && row.batch.is_none()
        }));
        assert!(activations.iter().any(|row| {
            row.key
                .starts_with("host-instantiated-module-extension-repositories:")
                && row.kind == ActivationKind::Reused
                && row.batch.is_none()
        }));

        let variants = [
            source.replace("attr.string()", "attr.string(default='d')"),
            source.replace("generated", "renamed"),
            source.replace("value='A'", "value='B'"),
            source.replace("name='generated', value='A'", "value='A', name='generated'"),
            source.replace("def impl", "\ndef impl"),
        ];
        for variant in variants {
            let b = compute_tracked(module, &variant).await;
            assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
                &a, &b
            ));
            let restored = compute_tracked(module, source).await;
            assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
                &a, &restored
            ));
        }

        let mapping_b = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, alias='generated')\n";
        let b = compute_tracked(mapping_b, source).await;
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &b
        ));
        let restored = compute_tracked(module, source).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &restored
        ));

        let need_module = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n";
        let need = compute_tracked(need_module, source).await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::validity(
            &need
        ));
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &need, &need
        ));
        assert!(tracker.take().iter().all(|row| {
            !row.key
                .starts_with("host-instantiated-module-extension-repositories:")
                || row.batch.is_none()
        }));
    }
    #[tokio::test]
    async fn observed_instantiation_identity_finisher_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(workspace.dupe());
        let same = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(workspace.dupe());
        let other = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |key: &HostInstantiatedModuleExtensionRepositoriesObservationKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(
            key.to_string(),
            "observed-host-instantiated-module-extension-repositories:\"/module-extension-repository-instantiation\""
        );
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let success_source = "repo=repository_rule(lambda ctx: None)\ndef impl(ctx):\n    repo(name='generated')\next=module_extension(implementation=impl)\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let success = compute_observed(&dice, module, success_source, true, 0, None).await;
        let carrier = observed_carrier(&success);
        assert!(carrier.result().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostInstantiatedModuleExtensionRepositoriesObservationKey::validity(&success));
        assert!(
            HostInstantiatedModuleExtensionRepositoriesObservationKey::equality(&success, &success)
        );

        let predecessor = carrier
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .predecessor
            .dupe();
        let count_mismatch = Arc::new(HostPureModuleExtensionInvocations {
            prepared: predecessor.prepared.dupe(),
            invoked: Arc::from([]),
        });
        assert!(matches!(
            instantiate_repositories(count_mismatch),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                completed,
                request: None,
                current,
                call: None,
                error: HostInstantiatedModuleExtensionRepositoryError::Join(message),
                ..
            }) if completed.is_empty() && current.is_empty() && message.contains("counts differ")
        ));

        let mut bad_call = predecessor.invoked[0].repository_rule_calls[0].clone();
        bad_call.name = "bad/name".into();
        let invalid_namespace = Arc::new(HostPureModuleExtensionInvocations {
            prepared: predecessor.prepared.dupe(),
            invoked: Arc::from([HostPureModuleExtensionInvocationReceipt {
                request: predecessor.invoked[0].request.clone(),
                repository_rule_calls: Arc::from([bad_call]),
            }]),
        });
        assert!(matches!(
            instantiate_repositories(invalid_namespace),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                completed,
                request: Some(_),
                current,
                call: None,
                error: HostInstantiatedModuleExtensionRepositoryError::Namespace(_),
                ..
            }) if completed.is_empty() && current.is_empty()
        ));
        for (source, terminal) in [(
            "repo=repository_rule(lambda ctx: None,attrs={'value':attr.string()})\ndef impl(ctx):\n    repo(name='first',value='ok')\n    repo(name='second',unknown='bad')\next=module_extension(implementation=impl)\n",
            "attribute",
        )] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy = compute(&case_dice, module, source, true).await;
            let observed = compute_observed(&case_dice, module, source, true, 0, None).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("{terminal} legacy must complete")
            };
            assert_eq!(
                legacy.as_ref(),
                observed_carrier(&observed).result().as_ref()
            );
            let error = observed_carrier(&observed)
                .result()
                .as_ref()
                .as_ref()
                .unwrap_err();
            assert!(
                matches!(
                    (terminal, error),
                    (
                        "namespace",
                        HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                            completed,
                            request: Some(_),
                            current,
                            call: Some(call),
                            error: HostInstantiatedModuleExtensionRepositoryError::Namespace(_),
                            ..
                        }
                    ) if completed.is_empty() && current.is_empty() && call.name == "bad/name"
                ) || matches!(
                    (terminal, error),
                    (
                        "attribute",
                        HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                            completed,
                            request: Some(_),
                            current,
                            call: Some(call),
                            error: HostInstantiatedModuleExtensionRepositoryError::Attribute(_),
                            ..
                        }
                    ) if completed.is_empty()
                        && current.len() == 1
                        && current[0].generated_name == "first"
                        && call.name == "second"
                )
            );
        }

        let pure_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy = compute(&pure_dice, module, success_source, false).await;
        let observed = compute_observed(&pure_dice, module, success_source, false, 0, None).await;
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("pure failure must complete")
        };
        assert_eq!(
            legacy.as_ref(),
            observed_carrier(&observed).result().as_ref()
        );
        assert!(matches!(
            observed_carrier(&observed).result().as_ref(),
            Err(HostInstantiatedModuleExtensionRepositoriesError::Invocations(_))
        ));

        let need_module = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n";
        let tracker = Arc::new(InstantiationTracker::default());
        let need = compute_observed(
            &Dice::builder().build(DetectCycles::Enabled),
            need_module,
            success_source,
            true,
            0,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostInstantiatedModuleExtensionRepositoriesObservationKey::validity(&need));
        assert!(!HostInstantiatedModuleExtensionRepositoriesObservationKey::equality(&need, &need));
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        let dependencies = tracker.take_dependencies();
        assert_eq!(
            dependencies
                .iter()
                .find(|(name, _)| name == &key.to_string())
                .unwrap()
                .1,
            [HostPureModuleExtensionInvocationsObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap()
            )
            .to_string()]
        );

        let source = include_str!("module_extension_repository_instantiation.rs");
        let producer = &source[source.find("type InstantiatedRepositoriesOutcome").unwrap()
            ..source.find("fn instantiate_repositories(").unwrap()];
        assert_eq!(
            producer
                .matches("HostPureModuleExtensionInvocationsObservationKey::new")
                .count(),
            1
        );
        assert!(!producer.contains("HostValidatedModuleExtensionRepositoriesKey"));
        assert!(!producer.contains("store_evaluation_data"));
        assert!(!producer.contains("union_"));
        assert!(
            producer.contains(
                "HostInstantiatedModuleExtensionRepositoriesObservationError::Pure(error)"
            )
        );
    }

    #[tokio::test]
    async fn observed_instantiation_real_order_events_and_parity() {
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let source = "print('load')\nrepo=repository_rule(lambda ctx: None,attrs={'value':attr.string()})\ndef impl(ctx):\n    print('invoke')\n    repo(name='generated',value='ok')\next=module_extension(implementation=impl)\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(InstantiationTracker::default());
        let observed =
            compute_observed(&dice, module, source, true, 0, Some(tracker.clone())).await;
        let mut legacy_tx =
            transaction(&dice, module, source, true, 0, Some(tracker.clone())).await;
        let legacy = legacy_tx
            .compute(&HostInstantiatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy must complete")
        };
        assert_eq!(
            legacy.as_ref(),
            observed_carrier(&observed).result().as_ref()
        );

        let key = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let legacy_key = HostInstantiatedModuleExtensionRepositoriesKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let dependencies = tracker.take_dependencies();
        assert_eq!(
            dependencies
                .iter()
                .find(|(name, _)| name == &key.to_string())
                .unwrap()
                .1,
            [HostPureModuleExtensionInvocationsObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap()
            )
            .to_string()]
        );
        assert_eq!(
            dependencies
                .iter()
                .find(|(name, _)| name == &legacy_key.to_string())
                .unwrap()
                .1,
            [HostPureModuleExtensionInvocationsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap()
            )
            .to_string()]
        );
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| { row.kind == ActivationKind::Evaluated && row.batch.is_none() })
        );
        let observed_prints = rows
            .iter()
            .filter(|row| row.key.starts_with("observed-"))
            .filter_map(|row| row.batch.as_ref())
            .flat_map(EventBatch::events)
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(observed_prints, ["load", "invoke"]);

        let warm_tracker = Arc::new(InstantiationTracker::default());
        let mut warm_tx =
            transaction(&dice, module, source, true, 0, Some(warm_tracker.clone())).await;
        let warm = warm_tx.compute(&key).await.unwrap();
        assert!(
            HostInstantiatedModuleExtensionRepositoriesObservationKey::equality(&observed, &warm)
        );
        assert!(Arc::ptr_eq(
            observed_carrier(&observed).result(),
            observed_carrier(&warm).result()
        ));
        let pure_key = HostPureModuleExtensionInvocationsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let _ = warm_tx.compute(&pure_key).await.unwrap();
        let warm_rows = warm_tracker.take();
        assert!(warm_rows.iter().any(|row| {
            row.key == key.to_string() && row.kind == ActivationKind::Reused && row.batch.is_none()
        }));
        assert!(warm_rows.iter().any(|row| {
            row.key == pure_key.to_string()
                && row.kind == ActivationKind::Reused
                && row.batch.is_none()
        }));
        assert!(warm_rows.iter().all(|row| row.batch.is_none()));

        let failing = "repo=repository_rule(lambda ctx: None,attrs={'value':attr.string()})\ndef impl(ctx):\n    repo(name='first',value='ok')\n    repo(name='second',unknown='bad')\n    repo(name='suppressed',value='no')\next=module_extension(implementation=impl)\n";
        let terminal_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let terminal_tracker = Arc::new(InstantiationTracker::default());
        let observed = compute_observed(
            &terminal_dice,
            module,
            failing,
            true,
            0,
            Some(terminal_tracker.clone()),
        )
        .await;
        let legacy = compute(&terminal_dice, module, failing, true).await;
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy failure must complete")
        };
        assert_eq!(
            legacy.as_ref(),
            observed_carrier(&observed).result().as_ref()
        );
        assert!(matches!(
            observed_carrier(&observed).result().as_ref(),
            Err(HostInstantiatedModuleExtensionRepositoriesError::AfterInvocations {
                completed,
                request: Some(_),
                current,
                call: Some(call),
                error: HostInstantiatedModuleExtensionRepositoryError::Attribute(_),
                ..
            }) if completed.is_empty()
                && current.len() == 1
                && current[0].generated_name == "first"
                && call.name == "second"
        ));
        assert!(
            terminal_tracker
                .take()
                .iter()
                .filter(|row| row
                    .key
                    .contains("instantiated-module-extension-repositories:"))
                .all(|row| row.batch.is_none())
        );
    }

    #[tokio::test]
    async fn observed_instantiation_lifecycle_cancellation_and_nonactivation() {
        let module_a = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let module_mapping = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e,alias='generated')\n";
        let source_a = "repo=repository_rule(lambda ctx: None,attrs={'value':attr.string()})\ndef impl(ctx):\n    repo(name='generated',value='A')\next=module_extension(implementation=impl)\n";
        let source_call = source_a.replace("value='A'", "value='B'");
        let source_schema =
            source_a.replace("attr.string()", "attr.string(default='schema-change')");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(workspace.dupe());
        let mut held = Vec::new();
        for (module, source, bias) in [
            (module_a, source_a, 0),
            (module_a, source_call.as_str(), 0),
            (module_a, source_a, 0),
            (module_a, source_schema.as_str(), 0),
            (module_a, source_a, 0),
            (module_mapping, source_a, 0),
            (module_a, source_a, 0),
            (module_a, source_a, 100),
        ] {
            let mut tx = transaction(&dice, module, source, true, bias, None).await;
            let global = tx.compute(&PathObservationEpochKey).await.unwrap();
            let value = tx.compute(&key).await.unwrap();
            let carrier = observed_carrier(&value).dupe();
            let pure = tx
                .compute(&HostPureModuleExtensionInvocationsObservationKey::new(
                    workspace.dupe(),
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(pure)) = pure else {
                panic!("observed pure child must complete")
            };
            assert_eq!(carrier.observations(), pure.observations());
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            held.push(carrier);
        }
        for (a, b, restored) in [(0, 1, 2), (2, 3, 4), (4, 5, 6)] {
            assert_ne!(held[a].result(), held[b].result());
            assert_eq!(held[a].result(), held[restored].result());
        }
        assert_eq!(held[0].result(), held[7].result());
        assert_ne!(held[0].observations(), held[7].observations());

        let tracker = Arc::new(InstantiationTracker::default());
        let mut warm_tx =
            transaction(&dice, module_a, source_a, true, 0, Some(tracker.clone())).await;
        let first = observed_carrier(&warm_tx.compute(&key).await.unwrap()).dupe();
        tracker.take();
        tracker.take_dependencies();
        let repeated = observed_carrier(&warm_tx.compute(&key).await.unwrap()).dupe();
        assert!(Arc::ptr_eq(first.result(), repeated.result()));
        assert!(tracker.take().iter().any(|row| {
            row.key == key.to_string() && row.kind == ActivationKind::Reused && row.batch.is_none()
        }));

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(InstantiationTracker::default());
        let mut cancelled = transaction(
            &cancel_dice,
            module_a,
            source_a,
            true,
            0,
            Some(cancel_tracker.clone()),
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            cancel_tracker
                .take()
                .iter()
                .all(|row| row.key != key.to_string())
        );
        assert!(
            cancel_tracker
                .take_dependencies()
                .iter()
                .all(|(name, _)| name != &key.to_string())
        );

        let mut recovery = transaction(
            &cancel_dice,
            module_a,
            source_a,
            true,
            0,
            Some(cancel_tracker.clone()),
        )
        .await;
        let own_global = recovery.compute(&PathObservationEpochKey).await.unwrap();
        let recovered = observed_carrier(&recovery.compute(&key).await.unwrap()).dupe();
        assert!(recovered.result().is_ok());
        for (demand, result) in recovered.observations().observations() {
            assert_eq!(result.as_ref(), own_global.get(demand).unwrap().as_ref());
        }
        let rows = cancel_tracker.take();
        let dependencies = cancel_tracker.take_dependencies();
        let validation = HostValidatedModuleExtensionRepositoriesKey::new(workspace).to_string();
        assert!(rows.iter().all(|row| row.key != validation));
        assert!(dependencies.iter().all(|(name, children)| {
            name != &validation && children.iter().all(|child| child != &validation)
        }));
    }
}
