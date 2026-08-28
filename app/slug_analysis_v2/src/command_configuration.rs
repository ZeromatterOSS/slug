//! Contextual preparation of command configuration occurrences.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostRootRepositoryMapping;
use slug_bzlmod_v2::HostRootRepositoryMappingKey;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationError;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::StarlarkOptionValue;
use slug_configuration_v2::StarlarkOptions;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_loading_v2::HostPackageInventory;
use slug_loading_v2::HostPackageInventoryErrorRef;
use slug_loading_v2::HostPackageInventoryKey;
use slug_loading_v2::HostPackageInventoryObservationError;
use slug_loading_v2::HostPackageInventoryObservationKey;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::package::BuildSettingDeclaration;
use slug_loading_v2::package::BuildSettingScope;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::build_setting::convert_command_occurrence;
use crate::build_setting::merge_command_occurrences;
use crate::build_setting::resolve_candidate;
use crate::dice::AnalysisError;
use crate::key::ConfigurationKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct CommandConfigurationPreparationKey {
    workspace: NormalizedAbsolutePath,
    base_configuration: ConfigurationKey,
    overlay: CommandConfigurationOverlay,
}

impl CommandConfigurationPreparationKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        base_configuration: ConfigurationKey,
        overlay: CommandConfigurationOverlay,
    ) -> Result<Self, AnalysisError> {
        if base_configuration.slug_configuration().is_none() {
            return Err(AnalysisError::message(
                "command configuration preparation requires a structural Slug configuration",
            ));
        }
        Ok(Self {
            workspace,
            base_configuration,
            overlay,
        })
    }
}

impl fmt::Display for CommandConfigurationPreparationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "command-configuration-preparation:{}:{}:{}",
            self.workspace,
            self.base_configuration,
            self.overlay.len()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct CommandConfigurationPreparationObservationKey(CommandConfigurationPreparationKey);

impl CommandConfigurationPreparationObservationKey {
    pub fn new(key: CommandConfigurationPreparationKey) -> Self {
        Self(key)
    }
}

impl fmt::Display for CommandConfigurationPreparationObservationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ObservedCommandConfiguration {
    result: Result<ConfigurationKey, AnalysisError>,
    observations: PathObservationEpoch,
}

impl ObservedCommandConfiguration {
    pub fn result(&self) -> &Result<ConfigurationKey, AnalysisError> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum CommandConfigurationPreparationOuterError {
    RootMapping(HostRootRepositoryMappingObservationError),
    Path(ObservedPathFrontierError),
}

impl fmt::Display for CommandConfigurationPreparationOuterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMapping(error) => write!(formatter, "{error:?}"),
            Self::Path(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommandConfigurationPreparationOuterError {}

pub type CommandConfigurationPreparationOutcome =
    LoadingPreparationOutcome<Arc<Result<ConfigurationKey, AnalysisError>>>;
pub type ObservedCommandConfigurationPreparationOutcome = LoadingPreparationOutcome<
    Result<Arc<ObservedCommandConfiguration>, CommandConfigurationPreparationOuterError>,
>;

#[derive(Clone, Copy)]
enum PreparationMode {
    Legacy,
    Observed,
}

struct PreparedDriverValue {
    result: Result<ConfigurationKey, AnalysisError>,
    observations: PathObservationEpoch,
}

type DriverOutcome = SourcePreparationOutcome<
    Result<PreparedDriverValue, CommandConfigurationPreparationOuterError>,
>;

struct MappingInput {
    result: Result<HostRootRepositoryMapping, AnalysisError>,
    observations: PathObservationEpoch,
}

type MappingOutcome =
    SourcePreparationOutcome<Result<MappingInput, CommandConfigurationPreparationOuterError>>;

async fn mapping_input(
    ctx: &mut DiceComputations<'_>,
    key: &CommandConfigurationPreparationKey,
    mode: PreparationMode,
) -> MappingOutcome {
    match mode {
        PreparationMode::Legacy => match ctx
            .compute(&HostRootRepositoryMappingKey::new(key.workspace.dupe()))
            .await
        {
            Err(error) => SourcePreparationOutcome::Complete(Ok(MappingInput {
                result: Err(AnalysisError::message(format!(
                    "loading final root repository mapping: {error}"
                ))),
                observations: PathObservationEpoch::empty(),
            })),
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => {
                SourcePreparationOutcome::Complete(Ok(MappingInput {
                    result: result
                        .as_ref()
                        .clone()
                        .map_err(|error| AnalysisError::message(error.to_string())),
                    observations: PathObservationEpoch::empty(),
                }))
            }
        },
        PreparationMode::Observed => match ctx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                key.workspace.dupe(),
            ))
            .await
        {
            Err(error) => SourcePreparationOutcome::Complete(Ok(MappingInput {
                result: Err(AnalysisError::message(format!(
                    "loading observed final root repository mapping: {error}"
                ))),
                observations: PathObservationEpoch::empty(),
            })),
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                SourcePreparationOutcome::Complete(Err(
                    CommandConfigurationPreparationOuterError::RootMapping(error),
                ))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                SourcePreparationOutcome::Complete(Ok(MappingInput {
                    result: observed
                        .result()
                        .as_ref()
                        .clone()
                        .map_err(|error| AnalysisError::message(error.to_string())),
                    observations: observed.observations().dupe(),
                }))
            }
        },
    }
}

fn prepared_complete(
    result: Result<ConfigurationKey, AnalysisError>,
    observations: PathObservationEpoch,
) -> DriverOutcome {
    SourcePreparationOutcome::Complete(Ok(PreparedDriverValue {
        result,
        observations,
    }))
}

fn root_option_mapping(
    mapping: &HostRootRepositoryMapping,
) -> Result<RepositoryMapping, AnalysisError> {
    let view = mapping
        .view()
        .ok_or_else(|| AnalysisError::message("final root repository mapping is unavailable"))?;
    let mut result = RepositoryMapping::new(
        RepositoryMappingId::new("command-configuration-root")
            .expect("static repository mapping id is valid"),
    );
    for (apparent, canonical) in view.mapping() {
        result.insert(apparent.clone(), canonical.clone());
    }
    Ok(result)
}

#[derive(Clone)]
struct ResolvedOccurrence {
    label: CanonicalLabel,
    raw_value: Option<compact_str::CompactString>,
    negated: bool,
}

struct DeclarationInput {
    result: Result<BuildSettingDeclaration, AnalysisError>,
    observations: PathObservationEpoch,
}

type DeclarationOutcome =
    SourcePreparationOutcome<Result<DeclarationInput, CommandConfigurationPreparationOuterError>>;

async fn declaration_input(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
    mode: PreparationMode,
) -> DeclarationOutcome {
    let (inventory, observations) = match mode {
        PreparationMode::Legacy => match ctx
            .compute(&HostPackageInventoryKey::new(
                workspace.dupe(),
                label.package().clone(),
            ))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(DeclarationInput {
                    result: Err(AnalysisError::message(format!(
                        "loading build-setting package for {label}: {error}"
                    ))),
                    observations: PathObservationEpoch::empty(),
                }));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(inventory)) => {
                (inventory, PathObservationEpoch::empty())
            }
        },
        PreparationMode::Observed => match ctx
            .compute(&HostPackageInventoryObservationKey::new(
                workspace.dupe(),
                label.package().clone(),
            ))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(DeclarationInput {
                    result: Err(AnalysisError::message(format!(
                        "loading observed build-setting package for {label}: {error}"
                    ))),
                    observations: PathObservationEpoch::empty(),
                }));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(
                HostPackageInventoryObservationError::Frontier(error),
            ))) => {
                return SourcePreparationOutcome::Complete(Err(
                    CommandConfigurationPreparationOuterError::Path(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Ok(DeclarationInput {
                    result: Err(AnalysisError::message(error.to_string())),
                    observations: PathObservationEpoch::empty(),
                }));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().dupe(), observed.observations().dupe())
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok(DeclarationInput {
        result: extract_declaration(label, &inventory),
        observations,
    }))
}

fn extract_declaration(
    label: &CanonicalLabel,
    inventory: &HostPackageInventory,
) -> Result<BuildSettingDeclaration, AnalysisError> {
    let package = inventory.loaded().map_err(|error| {
        let message = match error {
            HostPackageInventoryErrorRef::Root(error) => error.to_string(),
            HostPackageInventoryErrorRef::CanonicalRoute(error) => error.to_string(),
            HostPackageInventoryErrorRef::Canonical(error) => error.to_string(),
        };
        AnalysisError::message(message)
    })?;
    let target = package
        .targets
        .iter()
        .find(|target| target.name == label.target().as_str())
        .ok_or_else(|| AnalysisError::message(format!("build setting {label} is missing")))?;
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        return Err(AnalysisError::message(format!(
            "target {label} is not a Starlark build setting"
        )));
    };
    let declaration = rule
        .build_setting_declaration()
        .map_err(|error| {
            AnalysisError::message(format!(
                "invalid build-setting declaration for {label}: {error}"
            ))
        })?
        .ok_or_else(|| {
            AnalysisError::message(format!("target {label} is not a Starlark build setting"))
        })?;
    if declaration.scope() == BuildSettingScope::Project {
        return Err(AnalysisError::message(format!(
            "project-scoped build setting {label} is unsupported"
        )));
    }
    Ok(declaration)
}

fn union_needs(
    current: &mut Option<SourcePreparationNeeds>,
    next: SourcePreparationNeeds,
) -> Result<(), AnalysisError> {
    let merged = match current.as_ref() {
        Some(current) => current.try_union(&next).map_err(|error| {
            AnalysisError::message(format!("command preparation Needs disagree: {error:?}"))
        })?,
        None => next,
    };
    *current = Some(merged);
    Ok(())
}

fn union_observations(
    current: &PathObservationEpoch,
    next: &PathObservationEpoch,
) -> Result<PathObservationEpoch, CommandConfigurationPreparationOuterError> {
    PathObservationEpoch::from_shared(
        current
            .observations()
            .iter()
            .map(|(demand, value)| (demand.dupe(), value.dupe()))
            .chain(
                next.observations()
                    .iter()
                    .map(|(demand, value)| (demand.dupe(), value.dupe())),
            ),
    )
    .map_err(|error| {
        CommandConfigurationPreparationOuterError::Path(ObservedPathFrontierError::from(error))
    })
}

fn resolve_starlark_occurrence(
    apparent_label: &str,
    raw_value: Option<compact_str::CompactString>,
    negated: bool,
    mapping: &RepositoryMapping,
) -> Result<ResolvedOccurrence, AnalysisError> {
    let label = OptionLabelContext::MainRepository { mapping }
        .parse(apparent_label)
        .and_then(|resolved| CanonicalLabel::parse(&resolved.unambiguous_form()))
        .map_err(AnalysisError::message)?;
    Ok(ResolvedOccurrence {
        label,
        raw_value,
        negated,
    })
}

async fn prepare_driver(
    ctx: &mut DiceComputations<'_>,
    key: &CommandConfigurationPreparationKey,
    mode: PreparationMode,
) -> DriverOutcome {
    let base = key
        .base_configuration
        .slug_configuration()
        .expect("key validates structural configuration");
    let mut first_semantic = None;
    let prepared_native = match base.prepare_command_native_options(&key.overlay) {
        Ok(prepared) => Some(prepared),
        Err(error) => {
            first_semantic = Some(AnalysisError::message(error.to_string()));
            None
        }
    };

    let root_mapping = RepositoryMapping::new(
        RepositoryMappingId::new("command-configuration-root")
            .expect("static repository mapping id is valid"),
    );
    let mut resolved_rows = Vec::new();
    let mut distinct_labels = SmallSet::new();
    for occurrence in key.overlay.iter() {
        let CommandConfigurationOccurrence::Starlark {
            apparent_label,
            raw_value,
            negated,
        } = occurrence
        else {
            continue;
        };
        let resolved = if apparent_label.starts_with('@') {
            None
        } else {
            match resolve_starlark_occurrence(
                apparent_label,
                raw_value.clone(),
                *negated,
                &root_mapping,
            ) {
                Ok(resolved) => {
                    distinct_labels.insert(resolved.label.clone());
                    Some(resolved)
                }
                Err(error) => {
                    first_semantic.get_or_insert(error);
                    None
                }
            }
        };
        resolved_rows.push(resolved);
    }

    let requires_mapping = key.overlay.iter().any(|occurrence| {
        matches!(
            occurrence,
            CommandConfigurationOccurrence::Starlark { apparent_label, .. }
                if apparent_label.starts_with('@')
        )
    });
    let mut needs = None;
    let mut first_outer = None;
    let mut observations = PathObservationEpoch::empty();
    let mut option_mapping = None;
    if requires_mapping {
        match mapping_input(ctx, key, mode).await {
            SourcePreparationOutcome::Need(need) => {
                if let Err(error) = union_needs(&mut needs, need) {
                    first_semantic.get_or_insert(error);
                }
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                first_outer.get_or_insert(error);
            }
            SourcePreparationOutcome::Complete(Ok(mapping)) => {
                match union_observations(&observations, &mapping.observations) {
                    Ok(union) => observations = union,
                    Err(error) => {
                        first_outer.get_or_insert(error);
                    }
                }
                match mapping.result {
                    Ok(mapping) => match root_option_mapping(&mapping) {
                        Ok(mapping) => option_mapping = Some(mapping),
                        Err(error) => {
                            first_semantic.get_or_insert(error);
                        }
                    },
                    Err(error) => {
                        first_semantic.get_or_insert(error);
                    }
                }
            }
        }
    }
    if let Some(option_mapping) = option_mapping.as_ref() {
        let mut starlark_index = 0;
        for occurrence in key.overlay.iter() {
            let CommandConfigurationOccurrence::Starlark {
                apparent_label,
                raw_value,
                negated,
            } = occurrence
            else {
                continue;
            };
            if apparent_label.starts_with('@') {
                match resolve_starlark_occurrence(
                    apparent_label,
                    raw_value.clone(),
                    *negated,
                    option_mapping,
                ) {
                    Ok(resolved) => {
                        distinct_labels.insert(resolved.label.clone());
                        resolved_rows[starlark_index] = Some(resolved);
                    }
                    Err(error) => {
                        first_semantic.get_or_insert(error);
                    }
                }
            }
            starlark_index += 1;
        }
    }

    let mut declarations = SmallMap::new();
    for label in distinct_labels.iter() {
        match declaration_input(ctx, &key.workspace, label, mode).await {
            SourcePreparationOutcome::Need(need) => {
                if let Err(error) = union_needs(&mut needs, need) {
                    first_semantic.get_or_insert(error);
                }
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                first_outer.get_or_insert(error);
            }
            SourcePreparationOutcome::Complete(Ok(input)) => {
                match union_observations(&observations, &input.observations) {
                    Ok(union) => observations = union,
                    Err(error) => {
                        first_outer.get_or_insert(error);
                    }
                }
                match input.result {
                    Ok(declaration) => {
                        declarations.insert(label.clone(), declaration);
                    }
                    Err(error) => {
                        first_semantic.get_or_insert(error);
                    }
                }
            }
        }
    }
    if let Some(error) = first_outer {
        return SourcePreparationOutcome::Complete(Err(error));
    }
    if let Some(needs) = needs {
        return SourcePreparationOutcome::Need(needs);
    }
    if let Some(error) = first_semantic {
        return prepared_complete(Err(error), observations);
    }

    let mut groups: SmallMap<CanonicalLabel, Vec<StarlarkOptionValue>> = SmallMap::new();
    let mut first_conversion = None;
    for row in resolved_rows.iter().flatten() {
        let declaration = declarations
            .get(&row.label)
            .expect("complete declaration batch contains every resolved label");
        match convert_command_occurrence(
            &row.label,
            declaration,
            row.raw_value.as_deref(),
            row.negated,
        ) {
            Ok(value) => groups.entry(row.label.clone()).or_default().push(value),
            Err(error) => {
                first_conversion.get_or_insert_with(|| AnalysisError::message(error));
            }
        }
    }
    if let Some(error) = first_conversion {
        return prepared_complete(Err(error), observations);
    }

    let mut final_entries = base.starlark_options().iter().cloned().collect::<Vec<_>>();
    for (label, values) in groups.iter() {
        let declaration = declarations
            .get(label)
            .expect("group label has a loaded declaration");
        let merged = match merge_command_occurrences(declaration, values) {
            Ok(value) => value,
            Err(error) => {
                return prepared_complete(Err(AnalysisError::message(error)), observations);
            }
        };
        let resolved = match resolve_candidate(label.clone(), declaration, merged) {
            Ok(value) => value,
            Err(error) => {
                return prepared_complete(Err(AnalysisError::message(error)), observations);
            }
        };
        final_entries.retain(|entry| entry.label() != label);
        if let Some(resolved) = resolved {
            final_entries.push(resolved);
        }
    }
    let starlark_options = match StarlarkOptions::try_from_entries(final_entries) {
        Ok(options) => options,
        Err(error) => {
            return prepared_complete(Err(AnalysisError::message(error.to_string())), observations);
        }
    };
    let configuration = ConfigurationKey::from_slug(
        slug_configuration_v2::SlugConfiguration::with_prepared_command_configuration(
            prepared_native.expect("native conversion succeeds before semantic publication"),
            starlark_options,
        ),
    );
    prepared_complete(Ok(configuration), observations)
}

#[async_trait]
impl Key for CommandConfigurationPreparationKey {
    type Value = CommandConfigurationPreparationOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match prepare_driver(ctx, self, PreparationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(value)) => {
                SourcePreparationOutcome::Complete(Arc::new(value.result))
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy command configuration produced observed outer error: {error}")
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.complete_eq(right)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for CommandConfigurationPreparationObservationKey {
    type Value = ObservedCommandConfigurationPreparationOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match prepare_driver(ctx, &self.0, PreparationMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok(value)) => {
                SourcePreparationOutcome::Complete(Ok(Arc::new(ObservedCommandConfiguration {
                    result: value.result,
                    observations: value.observations,
                })))
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.complete_eq(right)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
