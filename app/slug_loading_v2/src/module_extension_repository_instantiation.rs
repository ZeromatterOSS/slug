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
use slug_bzlmod_v2::HostSelectedExtensionDefinitionOverride;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputs;
use slug_bzlmod_v2::OverrideAttributeKey;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;
#[cfg(test)]
use starlark_map::small_set::SmallSet;

use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::module_extension::HostPureModuleExtensionInvocationReceipt;
use crate::module_extension::HostPureModuleExtensionInvocations;
use crate::module_extension::HostPureModuleExtensionInvocationsError;
use crate::module_extension::HostPureModuleExtensionInvocationsKey;
use crate::module_extension::HostPureModuleExtensionInvocationsObservationError;
use crate::module_extension::HostPureModuleExtensionInvocationsObservationKey;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;
use crate::module_extension_repository_rule::RepositoryRuleCallKey;
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostInstantiatedInnateRepositoryOwner {
    inputs: Arc<HostSelectedInnateRepositoryOwnerInputs>,
    mapping_entries: Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    repositories: Arc<[HostInstantiatedModuleExtensionRepository]>,
}

impl HostInstantiatedInnateRepositoryOwner {
    pub(crate) fn parts(
        &self,
    ) -> (
        &Arc<HostSelectedInnateRepositoryOwnerInputs>,
        &[HostInstantiatedModuleExtensionRepository],
    ) {
        (&self.inputs, &self.repositories)
    }

    pub(crate) fn mapping_entries(&self) -> &Arc<SmallMap<ApparentRepoName, CanonicalRepoName>> {
        &self.mapping_entries
    }
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

    pub(crate) fn call(&self) -> &RepositoryRuleCallRecord {
        &self.call
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostInstantiateModuleExtensionRequestError {
    pub(crate) current: Arc<[HostInstantiatedModuleExtensionRepository]>,
    pub(crate) call: Option<RepositoryRuleCallRecord>,
    pub(crate) error: HostInstantiatedModuleExtensionRepositoryError,
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
pub(crate) struct HostInstantiatedModuleExtensionRepositoriesObservationKey(
    HostInstantiatedModuleExtensionRepositoriesKey,
);

#[allow(dead_code)]
impl HostInstantiatedModuleExtensionRepositoriesObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
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
pub(crate) struct ObservedHostInstantiatedModuleExtensionRepositories {
    result: InstantiatedRepositoriesResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostInstantiatedModuleExtensionRepositories {
    pub(crate) fn result(
        &self,
    ) -> &Arc<
        Result<
            HostInstantiatedModuleExtensionRepositories,
            HostInstantiatedModuleExtensionRepositoriesError,
        >,
    > {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum InstantiatedModuleExtensionRepositoriesObservationError {
    Pure(HostPureModuleExtensionInvocationsObservationError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostInstantiatedModuleExtensionRepositoriesObservationError(
    InstantiatedModuleExtensionRepositoriesObservationError,
);

type InstantiatedRepositoriesDriverOutcome = SourcePreparationOutcome<
    Result<
        (InstantiatedRepositoriesResult, PathObservationEpoch),
        InstantiatedModuleExtensionRepositoriesObservationError,
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
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(InstantiatedModuleExtensionRepositoriesObservationError::Pure(error))),
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
                SourcePreparationOutcome::Complete(Err(
                    HostInstantiatedModuleExtensionRepositoriesObservationError(error),
                ))
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
        match instantiate_request(receipt) {
            Ok(value) => completed.push(value),
            Err(error) => {
                return Err(after(
                    &completed,
                    Some(&receipt.request),
                    &error.current,
                    error.call.as_ref(),
                    error.error,
                ));
            }
        }
    }
    Ok(HostInstantiatedModuleExtensionRepositories {
        predecessor,
        extensions: completed.into(),
    })
}

pub(crate) fn instantiate_request(
    receipt: &HostPureModuleExtensionInvocationReceipt,
) -> Result<
    HostInstantiatedModuleExtensionRepositoriesForRequest,
    HostInstantiateModuleExtensionRequestError,
> {
    let (label_conversion_base, _, _, _) = receipt.request.parts();
    let (unique_name, context_repo, base, overrides) = receipt.request.namespace_parts();
    let (mapping_entries, repositories) = instantiate_parts(
        unique_name,
        context_repo,
        base,
        overrides,
        &receipt.repository_rule_calls,
        Some(label_conversion_base),
        None,
    )?;
    Ok(HostInstantiatedModuleExtensionRepositoriesForRequest {
        request: receipt.request.clone(),
        mapping_entries,
        repositories,
    })
}

pub(crate) fn instantiate_innate_request(
    inputs: Arc<HostSelectedInnateRepositoryOwnerInputs>,
    calls: &[RepositoryRuleCallRecord],
) -> Result<HostInstantiatedInnateRepositoryOwner, HostInstantiateModuleExtensionRequestError> {
    let (unique_name, context_repo, base, overrides) = inputs.namespace_parts();
    let (_, _, label_conversion_base, label_conversion_mapping) = inputs.definition_parts();
    let (mapping_entries, repositories) = instantiate_parts(
        unique_name,
        context_repo,
        base,
        overrides,
        calls,
        Some(label_conversion_base),
        Some(label_conversion_mapping),
    )?;
    Ok(HostInstantiatedInnateRepositoryOwner {
        inputs,
        mapping_entries,
        repositories,
    })
}

fn generated_repo(
    unique_name: &CanonicalRepoName,
    name: &str,
) -> Result<CanonicalRepoName, CompactString> {
    CanonicalRepoName::new(format!("{}+{name}", unique_name.as_str())).map_err(Into::into)
}

fn namespace_mapping(
    unique_name: &CanonicalRepoName,
    context_repo: &CanonicalRepoName,
    base: &SmallMap<ApparentRepoName, CanonicalRepoName>,
    overrides: &[HostSelectedExtensionDefinitionOverride],
    calls: &[RepositoryRuleCallRecord],
) -> Result<
    (
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
    CompactString,
> {
    let mut entries = base.clone();
    for call in calls {
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

fn instantiate_parts(
    unique_name: &CanonicalRepoName,
    context_repo: &CanonicalRepoName,
    base: &SmallMap<ApparentRepoName, CanonicalRepoName>,
    overrides: &[HostSelectedExtensionDefinitionOverride],
    calls: &[RepositoryRuleCallRecord],
    label_conversion_base: Option<&CanonicalLabel>,
    label_conversion_mapping: Option<&SmallMap<ApparentRepoName, CanonicalRepoName>>,
) -> Result<
    (
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
        Arc<[HostInstantiatedModuleExtensionRepository]>,
    ),
    HostInstantiateModuleExtensionRequestError,
> {
    let fail = |current: &[HostInstantiatedModuleExtensionRepository],
                call: Option<&RepositoryRuleCallRecord>,
                error| HostInstantiateModuleExtensionRequestError {
        current: current.into(),
        call: call.cloned(),
        error,
    };
    let mapping = namespace_mapping(unique_name, context_repo, base, overrides, calls).map_err(
        |message| {
            fail(
                &[],
                None,
                HostInstantiatedModuleExtensionRepositoryError::Namespace(message),
            )
        },
    )?;
    let mut current = Vec::new();
    for call in calls {
        let canonical_name = generated_repo(unique_name, &call.name).map_err(|message| {
            fail(
                &current,
                Some(call),
                HostInstantiatedModuleExtensionRepositoryError::Namespace(message),
            )
        })?;
        let label_conversion_base =
            label_conversion_base.unwrap_or(&call.definition.defining_label);
        let label_conversion_mapping = label_conversion_mapping.unwrap_or(mapping.1.as_ref());
        let repo_spec =
            instantiate_call_with_conversion(call, label_conversion_base, label_conversion_mapping)
                .map_err(|message| {
                    fail(
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
    Ok((mapping.1, current.into()))
}

#[cfg(test)]
fn instantiate_call(
    call: &RepositoryRuleCallRecord,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
) -> Result<RepoSpec, CompactString> {
    instantiate_call_with_base(call, mapping, &call.definition.defining_label)
}

#[cfg(test)]
fn instantiate_call_with_base(
    call: &RepositoryRuleCallRecord,
    mapping: &(
        CanonicalRepoName,
        Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
    ),
    label_conversion_base: &CanonicalLabel,
) -> Result<RepoSpec, CompactString> {
    instantiate_call_with_conversion(call, label_conversion_base, mapping.1.as_ref())
}

fn instantiate_call_with_conversion(
    call: &RepositoryRuleCallRecord,
    label_conversion_base: &CanonicalLabel,
    label_conversion_mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
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
        let conversion_base = match attribute.kind {
            AttributeKind::Output | AttributeKind::OutputList => &call.definition.defining_label,
            _ => label_conversion_base,
        };
        converted.insert(
            name.clone(),
            convert_supplied(attribute, raw, conversion_base, label_conversion_mapping)?,
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
            validate_default(
                attribute,
                default,
                &call.definition.defining_label,
                label_conversion_mapping,
            )?;
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
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<OverrideAttributeValue, CompactString> {
    coerce_value(attribute.kind, raw, defining_label, mapping)
}

fn coerce_value(
    kind: AttributeKind,
    raw: &RepositoryRuleCallValue,
    base: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<OverrideAttributeValue, CompactString> {
    match kind {
        AttributeKind::String => string_value(raw),
        AttributeKind::Boolean => bool_value(raw),
        AttributeKind::Integer => int_value(raw),
        AttributeKind::IntegerList => sequence_value(raw, int_value),
        AttributeKind::Label => label_value(raw, base, mapping, false),
        AttributeKind::Output => label_value(raw, base, mapping, true),
        AttributeKind::StringList => sequence_value(raw, |value| string_value(value)),
        AttributeKind::LabelList => {
            sequence_value(raw, |value| label_value(value, base, mapping, false))
        }
        AttributeKind::OutputList => {
            sequence_value(raw, |value| label_value(value, base, mapping, true))
        }
        AttributeKind::StringDict => {
            map_value(raw, |key| string_key(key), |value| string_value(value))
        }
        AttributeKind::StringListDict => map_value(
            raw,
            |key| string_key(key),
            |value| sequence_value(value, |value| string_value(value)),
        ),
        AttributeKind::StringKeyedLabelDict => map_value(
            raw,
            |key| string_key(key),
            |value| label_value(value, base, mapping, false),
        ),
        AttributeKind::LabelKeyedStringDict => label_keyed_map_value(raw, base, mapping),
        AttributeKind::LabelListDict => map_value(
            raw,
            |key| string_key(key),
            |value| sequence_value(value, |value| label_value(value, base, mapping, false)),
        ),
    }
}

fn string_value(raw: &RepositoryRuleCallValue) -> Result<OverrideAttributeValue, CompactString> {
    match raw {
        RepositoryRuleCallValue::String(value) => Ok(OverrideAttributeValue::String(value.clone())),
        _ => repository_value_error(),
    }
}

fn bool_value(raw: &RepositoryRuleCallValue) -> Result<OverrideAttributeValue, CompactString> {
    match raw {
        RepositoryRuleCallValue::Bool(value) => Ok(OverrideAttributeValue::Bool(*value)),
        _ => repository_value_error(),
    }
}

fn int_value(raw: &RepositoryRuleCallValue) -> Result<OverrideAttributeValue, CompactString> {
    match raw {
        RepositoryRuleCallValue::Int(value) => Ok(OverrideAttributeValue::Int(*value)),
        _ => repository_value_error(),
    }
}

fn label_value(
    raw: &RepositoryRuleCallValue,
    base: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
    output: bool,
) -> Result<OverrideAttributeValue, CompactString> {
    let label = match raw {
        RepositoryRuleCallValue::String(value) if output => {
            resolve_repository_output_label(value, base, mapping)?
        }
        RepositoryRuleCallValue::String(value) => resolve_label(value, base, mapping)?,
        RepositoryRuleCallValue::Label(value) if output => {
            let repository = value.package().repo();
            if repository != base.package().repo()
                && !mapping.values().any(|candidate| candidate == repository)
            {
                return Err(format!("label '{value}' is not visible from '{base}'").into());
            }
            value.clone()
        }
        RepositoryRuleCallValue::Label(value) => value.clone(),
        _ => return repository_value_error(),
    };
    if output && label.package() != base.package() {
        return Err(format!("label '{label}' is not in the current package").into());
    }
    Ok(OverrideAttributeValue::Label(label))
}

fn resolve_repository_output_label(
    raw: &str,
    base: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CanonicalLabel, CompactString> {
    if raw.starts_with("@@") || (raw.starts_with('@') && !raw.contains("//")) {
        return Err(format!("unsupported output label '{raw}'").into());
    }
    let rewritten = raw.strip_prefix("@//").map(|rest| format!("//{rest}"));
    let raw = rewritten.as_deref().unwrap_or(raw);
    let label = resolve_label(raw, base, mapping)?;
    if label.package().repo().is_root()
        && !base.package().repo().is_root()
        && raw.starts_with("//")
        && matches!(
            label.package().package().as_str(),
            "conditions" | "visibility"
        )
    {
        return label
            .rebind_provisional_root_repository(base.package().repo())
            .map_err(CompactString::from);
    }
    Ok(label)
}

fn sequence_value(
    raw: &RepositoryRuleCallValue,
    convert: impl Fn(&RepositoryRuleCallValue) -> Result<OverrideAttributeValue, CompactString>,
) -> Result<OverrideAttributeValue, CompactString> {
    let RepositoryRuleCallValue::Sequence(values) = raw else {
        return repository_value_error();
    };
    values
        .iter()
        .map(convert)
        .collect::<Result<Vec<_>, _>>()
        .map(|values| OverrideAttributeValue::Iterable(values.into()))
}

fn map_value(
    raw: &RepositoryRuleCallValue,
    key: impl Fn(&RepositoryRuleCallKey) -> Result<OverrideAttributeKey, CompactString>,
    value: impl Fn(&RepositoryRuleCallValue) -> Result<OverrideAttributeValue, CompactString>,
) -> Result<OverrideAttributeValue, CompactString> {
    let RepositoryRuleCallValue::Map(values) = raw else {
        return repository_value_error();
    };
    values
        .iter()
        .map(|(raw_key, raw_value)| Ok((key(raw_key)?, value(raw_value)?)))
        .collect::<Result<SmallMap<_, _>, CompactString>>()
        .map(|values| OverrideAttributeValue::Map(Arc::new(values)))
}

fn label_keyed_map_value(
    raw: &RepositoryRuleCallValue,
    base: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<OverrideAttributeValue, CompactString> {
    let RepositoryRuleCallValue::Map(values) = raw else {
        return repository_value_error();
    };
    let mut converted = SmallMap::new();
    let mut labels = Vec::with_capacity(values.len());
    for (raw_key, raw_value) in values.iter() {
        let label = label_key(raw_key, base, mapping)?;
        if labels
            .iter()
            .any(|existing: &CanonicalLabel| existing.bazel_natural_cmp(&label).is_eq())
        {
            return Err(format!("duplicate canonical label dictionary key '{label}'").into());
        }
        labels.push(label.clone());
        converted.insert(OverrideAttributeKey::Label(label), string_value(raw_value)?);
    }
    Ok(OverrideAttributeValue::Map(Arc::new(converted)))
}

fn string_key(raw: &RepositoryRuleCallKey) -> Result<OverrideAttributeKey, CompactString> {
    match raw {
        RepositoryRuleCallKey::String(value) => Ok(OverrideAttributeKey::String(value.clone())),
        RepositoryRuleCallKey::Label(_) => repository_value_error(),
    }
}

fn label_key(
    raw: &RepositoryRuleCallKey,
    base: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CanonicalLabel, CompactString> {
    let value = match raw {
        RepositoryRuleCallKey::String(value) => resolve_label(value, base, mapping)?,
        RepositoryRuleCallKey::Label(value) => value.clone(),
    };
    Ok(value)
}

fn repository_value_error<T>() -> Result<T, CompactString> {
    Err("unsupported value for repository-rule attribute".into())
}

fn validate_default(
    attribute: &RepositoryRuleAttribute,
    default: &CoercedAttributeValue,
    defining_label: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<(), CompactString> {
    if matches!(
        (attribute.kind, default),
        (AttributeKind::Label, CoercedAttributeValue::None)
    ) {
        return Ok(());
    }
    let default = default_call_value(default).ok_or_else(|| {
        CompactString::from(format!(
            "default for repository-rule attribute '{}' has the wrong kind",
            attribute.name
        ))
    })?;
    coerce_value(attribute.kind, &default, defining_label, mapping).map(|_| ())
}

fn default_call_value(value: &CoercedAttributeValue) -> Option<RepositoryRuleCallValue> {
    Some(match value {
        CoercedAttributeValue::None => RepositoryRuleCallValue::None,
        CoercedAttributeValue::Boolean(value) => RepositoryRuleCallValue::Bool(*value),
        CoercedAttributeValue::Integer(value) => RepositoryRuleCallValue::Int(*value),
        CoercedAttributeValue::IntegerList(values) => {
            sequence_call(values.iter().copied().map(RepositoryRuleCallValue::Int))
        }
        CoercedAttributeValue::String(value) => RepositoryRuleCallValue::String(value.clone()),
        CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => {
            RepositoryRuleCallValue::Label(value.clone())
        }
        CoercedAttributeValue::StringList(values) => {
            sequence_call(values.iter().cloned().map(RepositoryRuleCallValue::String))
        }
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            sequence_call(values.iter().cloned().map(RepositoryRuleCallValue::Label))
        }
        CoercedAttributeValue::StringDict(values) => map_call(values.iter().map(|(key, value)| {
            (
                RepositoryRuleCallKey::String(key.clone()),
                RepositoryRuleCallValue::String(value.clone()),
            )
        })),
        CoercedAttributeValue::StringListDict(values) => {
            map_call(values.iter().map(|(key, values)| {
                (
                    RepositoryRuleCallKey::String(key.clone()),
                    sequence_call(values.iter().cloned().map(RepositoryRuleCallValue::String)),
                )
            }))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            map_call(values.iter().map(|(key, value)| {
                (
                    RepositoryRuleCallKey::String(key.clone()),
                    RepositoryRuleCallValue::Label(value.clone()),
                )
            }))
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            map_call(values.iter().map(|(key, value)| {
                (
                    RepositoryRuleCallKey::Label(key.clone()),
                    RepositoryRuleCallValue::String(value.clone()),
                )
            }))
        }
        CoercedAttributeValue::LabelListDict(values) => {
            map_call(values.iter().map(|(key, values)| {
                (
                    RepositoryRuleCallKey::String(key.clone()),
                    sequence_call(values.iter().cloned().map(RepositoryRuleCallValue::Label)),
                )
            }))
        }
        CoercedAttributeValue::Selector { .. } | CoercedAttributeValue::Concatenation(_, _) => {
            return None;
        }
    })
}

fn sequence_call(values: impl Iterator<Item = RepositoryRuleCallValue>) -> RepositoryRuleCallValue {
    RepositoryRuleCallValue::Sequence(values.collect::<Vec<_>>().into())
}

fn map_call(
    values: impl Iterator<Item = (RepositoryRuleCallKey, RepositoryRuleCallValue)>,
) -> RepositoryRuleCallValue {
    RepositoryRuleCallValue::Map(values.collect::<Vec<_>>().into())
}

fn resolve_label(
    raw: &str,
    defining_label: &CanonicalLabel,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CanonicalLabel, CompactString> {
    CanonicalLabel::parse_with_package_context(raw, defining_label.package(), |requested| {
        mapping
            .iter()
            .find_map(|(name, repository)| (name.as_str() == requested).then(|| repository.clone()))
            .ok_or_else(|| format!("no repository visible as '@{requested}'"))
    })
    .map_err(Into::into)
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
    use slug_bzlmod_v2::NonrootAttributeKey;
    use slug_bzlmod_v2::NonrootAttributeValue;
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
    use crate::module_extension_innate_repository::call_value_for_test;
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
            file_admissibility: crate::attrs::FileAdmissibility::default(),
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
                    local: false,
                    configure: false,
                    environment: Arc::new(SmallSet::new()),
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
            Arc::new(SmallMap::from_iter([
                (
                    ApparentRepoName::new("dep").unwrap(),
                    CanonicalRepoName::new("dep+").unwrap(),
                ),
                (
                    ApparentRepoName::root(),
                    CanonicalRepoName::new("empty+").unwrap(),
                ),
            ])),
        )
    }

    #[test]
    fn pure_instantiation_stores_only_explicit_values_in_raw_order() {
        let record = call(
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
        let spec = instantiate_call(&record, &mapping()).unwrap();
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
    fn complete_repository_attribute_family_coerces_recursively() {
        let sequence = |values: Vec<_>| RepositoryRuleCallValue::Sequence(values.into());
        let map = |values| RepositoryRuleCallValue::Map(Arc::from(values));
        let local = CanonicalLabel::parse("@@//defs:l").unwrap();
        let output = CanonicalLabel::parse("@@//defs:o").unwrap();
        let dep = CanonicalLabel::parse("@@dep+//p:l").unwrap();
        let call = call(
            [
                schema("b", AttributeKind::Boolean, false, None),
                schema("i", AttributeKind::Integer, false, None),
                schema("il", AttributeKind::IntegerList, false, None),
                schema("s", AttributeKind::String, false, None),
                schema("l", AttributeKind::Label, false, None),
                schema("o", AttributeKind::Output, false, None),
                schema("sl", AttributeKind::StringList, false, None),
                schema("ll", AttributeKind::LabelList, false, None),
                schema("ol", AttributeKind::OutputList, false, None),
                schema("sd", AttributeKind::StringDict, false, None),
                schema("sld", AttributeKind::StringListDict, false, None),
                schema("skld", AttributeKind::StringKeyedLabelDict, false, None),
                schema("lksd", AttributeKind::LabelKeyedStringDict, false, None),
                schema("lld", AttributeKind::LabelListDict, false, None),
            ],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("b", RepositoryRuleCallValue::Bool(true)),
                ("i", RepositoryRuleCallValue::Int(7)),
                (
                    "il",
                    sequence(vec![
                        RepositoryRuleCallValue::Int(1),
                        RepositoryRuleCallValue::Int(-2),
                    ]),
                ),
                ("s", RepositoryRuleCallValue::String("s".into())),
                ("l", RepositoryRuleCallValue::Label(dep.clone())),
                ("o", RepositoryRuleCallValue::Label(output.clone())),
                (
                    "sl",
                    sequence(vec![RepositoryRuleCallValue::String("s".into())]),
                ),
                (
                    "ll",
                    sequence(vec![RepositoryRuleCallValue::Label(local.clone())]),
                ),
                (
                    "ol",
                    sequence(vec![RepositoryRuleCallValue::Label(output.clone())]),
                ),
                (
                    "sd",
                    map([(
                        RepositoryRuleCallKey::String("k".into()),
                        RepositoryRuleCallValue::String("v".into()),
                    )]),
                ),
                (
                    "sld",
                    map([(
                        RepositoryRuleCallKey::String("k".into()),
                        sequence(vec![RepositoryRuleCallValue::String("v".into())]),
                    )]),
                ),
                (
                    "skld",
                    map([(
                        RepositoryRuleCallKey::String("k".into()),
                        RepositoryRuleCallValue::Label(local.clone()),
                    )]),
                ),
                (
                    "lksd",
                    map([(
                        RepositoryRuleCallKey::Label(local.clone()),
                        RepositoryRuleCallValue::String("v".into()),
                    )]),
                ),
                (
                    "lld",
                    map([(
                        RepositoryRuleCallKey::String("k".into()),
                        sequence(vec![RepositoryRuleCallValue::Label(local.clone())]),
                    )]),
                ),
            ],
        );
        let spec = instantiate_call(&call, &mapping()).unwrap();
        assert_eq!(spec.attributes.len(), 14);
        assert_eq!(
            spec.attributes.get("b"),
            Some(&OverrideAttributeValue::Bool(true))
        );
        assert_eq!(
            spec.attributes.get("i"),
            Some(&OverrideAttributeValue::Int(7))
        );
        assert_eq!(
            spec.attributes.get("il"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Int(1),
                OverrideAttributeValue::Int(-2),
            ])))
        );
        assert_eq!(
            spec.attributes.get("s"),
            Some(&OverrideAttributeValue::String("s".into()))
        );
        assert_eq!(
            spec.attributes.get("l"),
            Some(&OverrideAttributeValue::Label(dep))
        );
        assert_eq!(
            spec.attributes.get("o"),
            Some(&OverrideAttributeValue::Label(output.clone()))
        );
        assert_eq!(
            spec.attributes.get("sl"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::String("s".into())
            ])))
        );
        assert_eq!(
            spec.attributes.get("ll"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Label(local.clone())
            ])))
        );
        assert_eq!(
            spec.attributes.get("ol"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Label(output)
            ])))
        );
        let expected_map =
            |key, value| OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(key, value)])));
        assert_eq!(
            spec.attributes.get("sd"),
            Some(&expected_map(
                OverrideAttributeKey::String("k".into()),
                OverrideAttributeValue::String("v".into())
            ))
        );
        assert_eq!(
            spec.attributes.get("sld"),
            Some(&expected_map(
                OverrideAttributeKey::String("k".into()),
                OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
                    "v".into()
                )]))
            ))
        );
        assert_eq!(
            spec.attributes.get("skld"),
            Some(&expected_map(
                OverrideAttributeKey::String("k".into()),
                OverrideAttributeValue::Label(local.clone())
            ))
        );
        assert_eq!(
            spec.attributes.get("lksd"),
            Some(&expected_map(
                OverrideAttributeKey::Label(local.clone()),
                OverrideAttributeValue::String("v".into())
            ))
        );
        assert_eq!(
            spec.attributes.get("lld"),
            Some(&expected_map(
                OverrideAttributeKey::String("k".into()),
                OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::Label(local)]))
            ))
        );
    }

    #[test]
    fn nested_label_positions_accept_raw_and_authenticated_forms() {
        let sequence = |values: Vec<_>| RepositoryRuleCallValue::Sequence(values.into());
        let map = |values: Vec<_>| RepositoryRuleCallValue::Map(values.into());
        let dep_typed = CanonicalLabel::parse("@@dep+//p:typed").unwrap();
        let dep_key = CanonicalLabel::parse("@@dep+//p:key_typed").unwrap();
        let local_output = CanonicalLabel::parse("@@//defs:typed.out").unwrap();
        let record = call(
            [
                schema("ll", AttributeKind::LabelList, false, None),
                schema("ol", AttributeKind::OutputList, false, None),
                schema("skld", AttributeKind::StringKeyedLabelDict, false, None),
                schema("lksd", AttributeKind::LabelKeyedStringDict, false, None),
                schema("lld", AttributeKind::LabelListDict, false, None),
            ],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                (
                    "ll",
                    sequence(vec![
                        RepositoryRuleCallValue::String("@dep//p:raw".into()),
                        RepositoryRuleCallValue::Label(dep_typed.clone()),
                    ]),
                ),
                (
                    "ol",
                    sequence(vec![
                        RepositoryRuleCallValue::String(":raw.out".into()),
                        RepositoryRuleCallValue::Label(local_output.clone()),
                    ]),
                ),
                (
                    "skld",
                    map(vec![
                        (
                            RepositoryRuleCallKey::String("raw".into()),
                            RepositoryRuleCallValue::String("@dep//p:value_raw".into()),
                        ),
                        (
                            RepositoryRuleCallKey::String("typed".into()),
                            RepositoryRuleCallValue::Label(dep_typed.clone()),
                        ),
                    ]),
                ),
                (
                    "lksd",
                    map(vec![
                        (
                            RepositoryRuleCallKey::String("@dep//p:key_raw".into()),
                            RepositoryRuleCallValue::String("raw".into()),
                        ),
                        (
                            RepositoryRuleCallKey::Label(dep_key.clone()),
                            RepositoryRuleCallValue::String("typed".into()),
                        ),
                    ]),
                ),
                (
                    "lld",
                    map(vec![(
                        RepositoryRuleCallKey::String("group".into()),
                        sequence(vec![
                            RepositoryRuleCallValue::String("@dep//p:nested_raw".into()),
                            RepositoryRuleCallValue::Label(dep_typed.clone()),
                        ]),
                    )]),
                ),
            ],
        );
        let spec = instantiate_call(&record, &mapping()).unwrap();
        assert_eq!(
            spec.attributes.get("ll"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Label(CanonicalLabel::parse("@@dep+//p:raw").unwrap()),
                OverrideAttributeValue::Label(dep_typed.clone()),
            ])))
        );
        assert_eq!(
            spec.attributes.get("ol"),
            Some(&OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Label(CanonicalLabel::parse("@@//defs:raw.out").unwrap()),
                OverrideAttributeValue::Label(local_output),
            ])))
        );
        assert_eq!(
            spec.attributes.get("lksd"),
            Some(&OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter(
                [
                    (
                        OverrideAttributeKey::Label(
                            CanonicalLabel::parse("@@dep+//p:key_raw").unwrap()
                        ),
                        OverrideAttributeValue::String("raw".into()),
                    ),
                    (
                        OverrideAttributeKey::Label(dep_key),
                        OverrideAttributeValue::String("typed".into()),
                    ),
                ]
            ))))
        );
        assert!(matches!(
            spec.attributes.get("skld"),
            Some(OverrideAttributeValue::Map(values))
                if matches!(values.get(&OverrideAttributeKey::String("raw".into())), Some(OverrideAttributeValue::Label(label)) if label.to_string() == "@@dep+//p:value_raw")
                    && matches!(values.get(&OverrideAttributeKey::String("typed".into())), Some(OverrideAttributeValue::Label(label)) if label == &dep_typed)
        ));
        assert!(matches!(
            spec.attributes.get("lld"),
            Some(OverrideAttributeValue::Map(values))
                if matches!(values.get(&OverrideAttributeKey::String("group".into())), Some(OverrideAttributeValue::Iterable(labels))
                    if matches!(labels.as_ref(), [OverrideAttributeValue::Label(raw), OverrideAttributeValue::Label(typed)]
                        if raw.to_string() == "@@dep+//p:nested_raw" && typed == &dep_typed))
        ));

        let missing = call(
            [schema(
                "value",
                AttributeKind::StringKeyedLabelDict,
                false,
                None,
            )],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                (
                    "value",
                    map(vec![(
                        RepositoryRuleCallKey::String("k".into()),
                        RepositoryRuleCallValue::String("@missing//:x".into()),
                    )]),
                ),
            ],
        );
        assert!(
            instantiate_call(&missing, &mapping())
                .unwrap_err()
                .contains("no repository visible")
        );
        let hidden = CanonicalLabel::parse("@@hidden+//:x").unwrap();
        let typed = call(
            [schema(
                "value",
                AttributeKind::LabelKeyedStringDict,
                false,
                None,
            )],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                (
                    "value",
                    map(vec![(
                        RepositoryRuleCallKey::Label(hidden.clone()),
                        RepositoryRuleCallValue::String("v".into()),
                    )]),
                ),
            ],
        );
        assert!(matches!(
            instantiate_call(&typed, &mapping())
                .unwrap()
                .attributes
                .get("value"),
            Some(OverrideAttributeValue::Map(values))
                if values.contains_key(&OverrideAttributeKey::Label(hidden))
        ));
    }

    #[test]
    fn ordinary_and_innate_collections_publish_equal_repo_specs() {
        let innate = NonrootAttributeValue::Tuple(Arc::from([
            NonrootAttributeValue::String("first".into()),
            NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
                (
                    NonrootAttributeKey::String("z".into()),
                    NonrootAttributeValue::List(Arc::from([
                        NonrootAttributeValue::String("one".into()),
                        NonrootAttributeValue::String("two".into()),
                    ])),
                ),
                (
                    NonrootAttributeKey::String("a".into()),
                    NonrootAttributeValue::List(Arc::from([NonrootAttributeValue::String(
                        "three".into(),
                    )])),
                ),
            ]))),
        ]));
        let innate = call_value_for_test(&innate).unwrap();
        let RepositoryRuleCallValue::Sequence(innate_values) = &innate else {
            panic!("innate tuple must normalize to the shared sequence carrier")
        };
        let ordinary = RepositoryRuleCallValue::Sequence(Arc::from([
            RepositoryRuleCallValue::String("first".into()),
            RepositoryRuleCallValue::Map(Arc::from([
                (
                    RepositoryRuleCallKey::String("z".into()),
                    RepositoryRuleCallValue::Sequence(Arc::from([
                        RepositoryRuleCallValue::String("one".into()),
                        RepositoryRuleCallValue::String("two".into()),
                    ])),
                ),
                (
                    RepositoryRuleCallKey::String("a".into()),
                    RepositoryRuleCallValue::Sequence(Arc::from([
                        RepositoryRuleCallValue::String("three".into()),
                    ])),
                ),
            ])),
        ]));
        assert_eq!(innate, ordinary);
        assert_eq!(innate_values.len(), 2);

        let parity_schema = [
            schema("words", AttributeKind::StringList, false, None),
            schema("groups", AttributeKind::StringListDict, false, None),
        ];
        let split = |raw: RepositoryRuleCallValue| {
            let RepositoryRuleCallValue::Sequence(values) = raw else {
                unreachable!()
            };
            call(
                parity_schema.clone(),
                [
                    ("name", RepositoryRuleCallValue::String("generated".into())),
                    (
                        "words",
                        RepositoryRuleCallValue::Sequence(Arc::from([values[0].clone()])),
                    ),
                    ("groups", values[1].clone()),
                ],
            )
        };
        let ordinary_spec = instantiate_call(&split(ordinary), &mapping()).unwrap();
        let innate_spec = instantiate_call(&split(innate), &mapping()).unwrap();
        assert_eq!(ordinary_spec, innate_spec);
        assert_eq!(
            ordinary_spec.attributes.keys().collect::<Vec<_>>(),
            innate_spec.attributes.keys().collect::<Vec<_>>()
        );

        let innate_labels = NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
            (
                NonrootAttributeKey::Label("@@dep+//p:key".into()),
                NonrootAttributeValue::Label("@@dep+//p:value".into()),
            ),
            (
                NonrootAttributeKey::String("raw".into()),
                NonrootAttributeValue::String("@dep//p:raw".into()),
            ),
        ])));
        assert_eq!(
            call_value_for_test(&innate_labels).unwrap(),
            RepositoryRuleCallValue::Map(Arc::from([
                (
                    RepositoryRuleCallKey::Label(CanonicalLabel::parse("@@dep+//p:key").unwrap()),
                    RepositoryRuleCallValue::Label(
                        CanonicalLabel::parse("@@dep+//p:value").unwrap()
                    ),
                ),
                (
                    RepositoryRuleCallKey::String("raw".into()),
                    RepositoryRuleCallValue::String("@dep//p:raw".into()),
                ),
            ]))
        );
        assert!(
            call_value_for_test(&NonrootAttributeValue::Label(
                "@dep//p:not_canonical".into()
            ))
            .is_err()
        );
        assert!(
            call_value_for_test(&NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter(
                [(
                    NonrootAttributeKey::Label("@dep//p:not_canonical".into()),
                    NonrootAttributeValue::String("value".into()),
                ),]
            ))))
            .is_err()
        );

        let ordered_innate = |reversed| {
            let entries = if reversed {
                [("a", "one"), ("z", "two")]
            } else {
                [("z", "two"), ("a", "one")]
            };
            NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter(entries.map(
                |(key, value)| {
                    (
                        NonrootAttributeKey::String(key.into()),
                        NonrootAttributeValue::String(value.into()),
                    )
                },
            ))))
        };
        let ordered_ordinary = |reversed| {
            let entries = if reversed {
                [("a", "one"), ("z", "two")]
            } else {
                [("z", "two"), ("a", "one")]
            };
            RepositoryRuleCallValue::Map(Arc::from(entries.map(|(key, value)| {
                (
                    RepositoryRuleCallKey::String(key.into()),
                    RepositoryRuleCallValue::String(value.into()),
                )
            })))
        };
        let ordered_spec = |value| {
            instantiate_call(
                &call(
                    [schema("values", AttributeKind::StringDict, false, None)],
                    [
                        ("name", RepositoryRuleCallValue::String("generated".into())),
                        ("values", value),
                    ],
                ),
                &mapping(),
            )
            .unwrap()
        };
        let ordinary_a = ordered_spec(ordered_ordinary(false));
        let ordinary_b = ordered_spec(ordered_ordinary(true));
        let ordinary_restored = ordered_spec(ordered_ordinary(false));
        let innate_a = ordered_spec(call_value_for_test(&ordered_innate(false)).unwrap());
        let innate_b = ordered_spec(call_value_for_test(&ordered_innate(true)).unwrap());
        let innate_restored = ordered_spec(call_value_for_test(&ordered_innate(false)).unwrap());
        assert_eq!(ordinary_a, innate_a);
        assert_eq!(ordinary_b, innate_b);
        assert_eq!(ordinary_restored, innate_restored);
        assert_ne!(ordinary_a, ordinary_b);
        assert_eq!(ordinary_a, ordinary_restored);
    }

    #[test]
    fn omitted_all_kinds_and_defaults_validate_without_publication() {
        let omitted = call(
            [
                schema("b", AttributeKind::Boolean, false, None),
                schema("i", AttributeKind::Integer, false, None),
                schema("s", AttributeKind::String, false, None),
                schema("l", AttributeKind::Label, false, None),
                schema("o", AttributeKind::Output, false, None),
                schema("sl", AttributeKind::StringList, false, None),
                schema("ll", AttributeKind::LabelList, false, None),
                schema("ol", AttributeKind::OutputList, false, None),
                schema("sd", AttributeKind::StringDict, false, None),
                schema("sld", AttributeKind::StringListDict, false, None),
                schema("skld", AttributeKind::StringKeyedLabelDict, false, None),
                schema("lksd", AttributeKind::LabelKeyedStringDict, false, None),
                schema("lld", AttributeKind::LabelListDict, false, None),
            ],
            [("name", RepositoryRuleCallValue::String("generated".into()))],
        );
        assert!(
            instantiate_call(&omitted, &mapping())
                .unwrap()
                .attributes
                .is_empty()
        );

        let label = CanonicalLabel::parse("@@//defs:l").unwrap();
        let defaults = call(
            [
                schema(
                    "b",
                    AttributeKind::Boolean,
                    false,
                    Some(CoercedAttributeValue::Boolean(true)),
                ),
                schema(
                    "i",
                    AttributeKind::Integer,
                    false,
                    Some(CoercedAttributeValue::Integer(1)),
                ),
                schema(
                    "s",
                    AttributeKind::String,
                    false,
                    Some(CoercedAttributeValue::String("s".into())),
                ),
                schema(
                    "l",
                    AttributeKind::Label,
                    false,
                    Some(CoercedAttributeValue::Label(label.clone())),
                ),
                schema(
                    "sl",
                    AttributeKind::StringList,
                    false,
                    Some(CoercedAttributeValue::StringList(Arc::from([
                        CompactString::new("s"),
                    ]))),
                ),
                schema(
                    "ll",
                    AttributeKind::LabelList,
                    false,
                    Some(CoercedAttributeValue::LabelList(Arc::from([label.clone()]))),
                ),
                schema(
                    "sd",
                    AttributeKind::StringDict,
                    false,
                    Some(CoercedAttributeValue::StringDict(Arc::from([(
                        "k".into(),
                        "v".into(),
                    )]))),
                ),
                schema(
                    "sld",
                    AttributeKind::StringListDict,
                    false,
                    Some(CoercedAttributeValue::StringListDict(Arc::from([(
                        "k".into(),
                        Arc::from(["v".into()]),
                    )]))),
                ),
                schema(
                    "skld",
                    AttributeKind::StringKeyedLabelDict,
                    false,
                    Some(CoercedAttributeValue::StringKeyedLabelDict(Arc::from([(
                        "k".into(),
                        label.clone(),
                    )]))),
                ),
                schema(
                    "lksd",
                    AttributeKind::LabelKeyedStringDict,
                    false,
                    Some(CoercedAttributeValue::LabelKeyedStringDict(Arc::from([(
                        label.clone(),
                        "v".into(),
                    )]))),
                ),
                schema(
                    "lld",
                    AttributeKind::LabelListDict,
                    false,
                    Some(CoercedAttributeValue::LabelListDict(Arc::from([(
                        "k".into(),
                        Arc::from([label]),
                    )]))),
                ),
            ],
            [("name", RepositoryRuleCallValue::String("generated".into()))],
        );
        assert!(
            instantiate_call(&defaults, &mapping())
                .unwrap()
                .attributes
                .is_empty()
        );
    }

    #[test]
    fn innate_call_uses_module_base_and_calling_mapping_but_keeps_actual_rule_id() {
        let mut call = call(
            [
                schema("target", AttributeKind::Label, true, None),
                schema("mapped", AttributeKind::Label, true, None),
                schema(
                    "default_target",
                    AttributeKind::Label,
                    false,
                    Some(CoercedAttributeValue::Label(
                        CanonicalLabel::parse("@@hidden+//defs:default").unwrap(),
                    )),
                ),
            ],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("target", RepositoryRuleCallValue::String(":dep".into())),
                ("mapped", RepositoryRuleCallValue::String("@dep".into())),
            ],
        );
        call.definition.defining_label = CanonicalLabel::parse("@@hidden+//defs:repo.bzl").unwrap();
        let module_base = CanonicalLabel::parse("@@//:MODULE.bazel").unwrap();
        let caller_mapping = SmallMap::from_iter([(
            ApparentRepoName::new("dep").unwrap(),
            CanonicalRepoName::new("caller+").unwrap(),
        )]);
        let innate =
            instantiate_call_with_conversion(&call, &module_base, &caller_mapping).unwrap();
        assert_eq!(
            innate.rule_id.bzl_file.to_string(),
            "@@hidden+//defs:repo.bzl"
        );
        assert_eq!(
            innate.attributes.get("target"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@//:dep").unwrap()
            ))
        );
        assert_eq!(
            innate.attributes.get("mapped"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@caller+//:dep").unwrap()
            ))
        );
        assert!(!innate.attributes.contains_key("default_target"));
        let ordinary = instantiate_call(&call, &mapping()).unwrap();
        assert_eq!(
            ordinary.attributes.get("target"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@hidden+//defs:dep").unwrap()
            ))
        );
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
            ("@dep", "@@dep+//:dep"),
            ("@@direct+", "@@direct+//:direct+"),
            ("@//pkg:item", "@@empty+//pkg:item"),
            ("@@//pkg:item", "@@//pkg:item"),
            ("//pkg/item", "@@//pkg/item:item"),
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

        let collision = call(
            [schema(
                "values",
                AttributeKind::LabelKeyedStringDict,
                false,
                None,
            )],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                (
                    "values",
                    RepositoryRuleCallValue::Map(Arc::from([
                        (
                            RepositoryRuleCallKey::String("@dep//:same".into()),
                            RepositoryRuleCallValue::String("first".into()),
                        ),
                        (
                            RepositoryRuleCallKey::Label(
                                CanonicalLabel::parse("@@dep+//:same").unwrap(),
                            ),
                            RepositoryRuleCallValue::String("second".into()),
                        ),
                    ])),
                ),
            ],
        );
        assert!(
            instantiate_call(&collision, &mapping())
                .unwrap_err()
                .contains("duplicate canonical label")
        );

        for (kind, value, expected) in [
            (
                AttributeKind::Label,
                RepositoryRuleCallValue::String("@missing//:x".into()),
                "no repository visible",
            ),
            (
                AttributeKind::Output,
                RepositoryRuleCallValue::String("//other:x".into()),
                "not in the current package",
            ),
            (
                AttributeKind::OutputList,
                RepositoryRuleCallValue::Sequence(Arc::from([RepositoryRuleCallValue::String(
                    "//other:x".into(),
                )])),
                "not in the current package",
            ),
            (
                AttributeKind::Output,
                RepositoryRuleCallValue::String("@@//defs:x".into()),
                "unsupported output label",
            ),
            (
                AttributeKind::Output,
                RepositoryRuleCallValue::String("@dep".into()),
                "unsupported output label",
            ),
        ] {
            let invalid = call(
                [schema("value", kind, false, None)],
                [
                    ("name", RepositoryRuleCallValue::String("generated".into())),
                    ("value", value),
                ],
            );
            assert!(
                instantiate_call(&invalid, &mapping())
                    .unwrap_err()
                    .contains(expected)
            );
        }

        let hidden = CanonicalLabel::parse("@@hidden+//:x").unwrap();
        let typed_hidden = call(
            [schema("value", AttributeKind::Label, false, None)],
            [
                ("name", RepositoryRuleCallValue::String("generated".into())),
                ("value", RepositoryRuleCallValue::Label(hidden.clone())),
            ],
        );
        assert_eq!(
            instantiate_call(&typed_hidden, &mapping())
                .unwrap()
                .attributes
                .get("value"),
            Some(&OverrideAttributeValue::Label(hidden))
        );

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
        assert!(instantiate_call(&invisible_default, &mapping()).is_ok());
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
                 use_repo(e, {target}='{target}')\n\
                 override_repo(e, second='{target}')\n"
            )
        };
        let source = |value: &str, default_target: &str| {
            format!(
                r#"repo=repository_rule(lambda ctx: None, attrs={{'text':attr.string(mandatory=True),'target':attr.label(),'peer':attr.label(),'default_target':attr.label(default='{default_target}')}}, local=True, configure=True, environ=['B','A','B'])
def impl(ctx):
    repo(name='first', text='{value}')
    repo(name='second', text='two', target='@second//:item', peer='@first//:item')
ext=module_extension(implementation=impl)
"#
            )
        };
        let source_a = source("one", ":default_a");
        let a = compute(&dice, &module("replacement"), &source_a, true).await;
        let warm = compute(&dice, &module("replacement"), &source_a, true).await;
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
        assert!(matches!(
            rows[0]
                .call()
                .definition
                .attributes
                .iter()
                .find(|attribute| attribute.name == "default_target")
                .and_then(|attribute| attribute.default.as_ref()),
            Some(CoercedAttributeValue::Label(label))
                if label == &CanonicalLabel::parse("@@//:default_a").unwrap()
        ));
        assert!(rows[0].call().definition.local);
        assert!(rows[0].call().definition.configure);
        assert_eq!(
            rows[0]
                .call()
                .definition
                .environment
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        assert!(Arc::ptr_eq(
            &rows[0].call().definition.environment,
            &rows[1].call().definition.environment
        ));
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

        let mut imported_receipt = a_value.predecessor.invoked[0].clone();
        let mut imported_call = imported_receipt.repository_rule_calls[0].clone();
        imported_call.definition.defining_label =
            CanonicalLabel::parse("@@defs+//rules:repo.bzl").unwrap();
        let mut imported_attributes = imported_call.definition.attributes.to_vec();
        imported_attributes
            .iter_mut()
            .find(|attribute| attribute.name == "default_target")
            .unwrap()
            .default = Some(CoercedAttributeValue::Label(
            CanonicalLabel::parse("@@defs+//rules:default").unwrap(),
        ));
        imported_attributes.push(schema("output", AttributeKind::Output, false, None));
        imported_call.definition.attributes = imported_attributes.into();
        let mut imported_kwargs = imported_call.kwargs.to_vec();
        imported_kwargs.push((
            "target".into(),
            RepositoryRuleCallValue::String(":explicit".into()),
        ));
        imported_kwargs.push((
            "output".into(),
            RepositoryRuleCallValue::String(":generated".into()),
        ));
        imported_call.kwargs = imported_kwargs.into();
        imported_receipt.repository_rule_calls = Arc::from([imported_call]);
        let imported = instantiate_request(&imported_receipt).unwrap();
        let imported_row = &imported.repositories[0];
        assert_eq!(
            imported_row.repo_spec.attributes.get("target"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@//:explicit").unwrap()
            ))
        );
        assert_eq!(
            imported_row.repo_spec.attributes.get("output"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@defs+//rules:generated").unwrap()
            ))
        );
        assert_eq!(
            imported_row.repo_spec.rule_id.bzl_file,
            CanonicalLabel::parse("@@defs+//rules:repo.bzl").unwrap()
        );
        assert!(matches!(
            imported_row
                .call
                .definition
                .attributes
                .iter()
                .find(|attribute| attribute.name == "default_target")
                .and_then(|attribute| attribute.default.as_ref()),
            Some(CoercedAttributeValue::Label(label))
                if label == &CanonicalLabel::parse("@@defs+//rules:default").unwrap()
        ));
        for package in ["conditions", "visibility"] {
            assert_eq!(
                resolve_repository_output_label(
                    &format!("//{package}:generated"),
                    &imported_row.call.definition.defining_label,
                    imported.mapping_entries.as_ref(),
                )
                .unwrap(),
                CanonicalLabel::parse(&format!("@@defs+//{package}:generated")).unwrap()
            );
        }

        let b = compute(
            &dice,
            &module("replacement"),
            &source("changed", ":default_a"),
            true,
        )
        .await;
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &b
        ));
        let restored = compute(&dice, &module("replacement"), &source_a, true).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &restored
        ));
        let mapping_b = compute(&dice, &module("other"), &source_a, true).await;
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &mapping_b
        ));
        let SourcePreparationOutcome::Complete(mapping_b_value) = &mapping_b else {
            panic!("mapping-changed instantiation must complete")
        };
        assert_ne!(
            rows[1].repo_spec.attributes.get("target"),
            mapping_b_value.as_ref().as_ref().unwrap().extensions[0].repositories[1]
                .repo_spec
                .attributes
                .get("target")
        );
        let mapping_restored = compute(&dice, &module("replacement"), &source_a, true).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a,
            &mapping_restored
        ));
        let default_b = compute(
            &dice,
            &module("replacement"),
            &source("one", ":default_b"),
            true,
        )
        .await;
        assert!(!HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a, &default_b
        ));
        let SourcePreparationOutcome::Complete(default_b_value) = &default_b else {
            panic!("default-changed instantiation must complete")
        };
        assert!(matches!(
            default_b_value.as_ref().as_ref().unwrap().extensions[0].repositories[0]
                .call()
                .definition
                .attributes
                .iter()
                .find(|attribute| attribute.name == "default_target")
                .and_then(|attribute| attribute.default.as_ref()),
            Some(CoercedAttributeValue::Label(label))
                if label == &CanonicalLabel::parse("@@//:default_b").unwrap()
        ));
        let default_restored = compute(&dice, &module("replacement"), &source_a, true).await;
        assert!(HostInstantiatedModuleExtensionRepositoriesKey::equality(
            &a,
            &default_restored
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
        let missing = compute(&dice, &module("replacement"), &source_a, false).await;
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
        let alternate = compute(&dice, alternate_module, &source_a, true).await;
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
                metadata: predecessor.invoked[0].metadata.clone(),
                root_usage: predecessor.invoked[0].root_usage,
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
                metadata: predecessor.invoked[0].metadata.clone(),
                root_usage: predecessor.invoked[0].root_usage,
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
            producer
                .contains("InstantiatedModuleExtensionRepositoriesObservationError::Pure(error)")
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
