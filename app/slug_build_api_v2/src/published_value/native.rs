//! Borrowed, closed native frontier. Every byte/work charge precedes retention.
use super::*;
use crate::AnalysisArtifact;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::FilesToRunProvider;
use crate::cc_features::*;
#[path = "native_depset.rs"]
mod depset;
#[path = "native_owner.rs"]
mod owner;

pub(super) fn text(b: &mut PublishedValueArenaBuilder, s: &str) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.charge_owner_edge()?;
    b.charge_bytes(s.len() + std::mem::size_of::<compact_str::CompactString>())
}
fn strings<'a>(
    b: &mut PublishedValueArenaBuilder,
    values: impl IntoIterator<Item = &'a compact_str::CompactString>,
) -> Result<(), PublishedValueError> {
    for s in values {
        text(b, s)?;
    }
    Ok(())
}
fn label(
    b: &mut PublishedValueArenaBuilder,
    l: &slug_identity_v2::CanonicalLabel,
) -> Result<(), PublishedValueError> {
    text(b, l.package().repo().as_str())?;
    text(b, l.package().package().as_str())?;
    text(b, l.target().as_str())?;
    if let Some(mapping) = l.mapping_id() {
        text(b, mapping.as_str())?;
    }
    Ok(())
}
fn configured_key(
    b: &mut PublishedValueArenaBuilder,
    key: &crate::AnalysisConfiguredTargetKey,
) -> Result<(), PublishedValueError> {
    label(b, key.label())?;
    b.charge_bytes(key.configuration().len())?;
    if let Some(platform) = key.toolchain_execution_platform() {
        label(b, platform)?;
    }
    Ok(())
}
fn artifact(
    b: &mut PublishedValueArenaBuilder,
    a: &AnalysisArtifact,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    match a {
        AnalysisArtifact::Source(l) => label(b, l),
        AnalysisArtifact::Derived { owner, output } => {
            configured_key(b, owner)?;
            text(b, output.path())?;
            b.charge_bytes(output.root().configuration().canonical_bytes().len())
        }
        AnalysisArtifact::WorkspaceStatus(a) => b.charge_bytes(
            a.root()
                .workspace()
                .as_path()
                .as_os_str()
                .as_encoded_bytes()
                .len(),
        ),
    }
}
/// Common original native header and strict origin barrier. A successful
/// owner-mode import is terminal; strict typed proof callers never import.
pub(super) fn audit_header(
    b: &mut PublishedValueArenaBuilder,
    value: &AnalysisValue,
    depth: usize,
) -> Result<bool, PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<AnalysisValue>())?;
    if let Some(origin) = value.original_origin() {
        if b.owner_audit {
            b.import(origin.arena().clone())?;
            return Ok(true);
        }
        return Err(PublishedValueError::Invalid("arena-bearing native atom"));
    }
    Ok(false)
}
pub(super) fn audit(
    b: &mut PublishedValueArenaBuilder,
    value: &AnalysisValue,
    depth: usize,
) -> Result<(), PublishedValueError> {
    if audit_header(b, value, depth)? {
        return Ok(());
    }
    match value.kind() {
        AnalysisValueKind::None | AnalysisValueKind::Boolean(_) => Ok(()),
        AnalysisValueKind::Number(n) => {
            if let Some(i) = n.as_integer() {
                b.charge_bytes(i.magnitude().len())?;
            }
            Ok(())
        }
        AnalysisValueKind::String(s) => text(b, s),
        AnalysisValueKind::Label(l) => label(b, l),
        AnalysisValueKind::Artifact(a) => artifact(b, a),
        AnalysisValueKind::ArtifactRoot(r) => match r {
            crate::AnalysisArtifactRoot::Source(repo) => text(b, repo.as_str()),
            crate::AnalysisArtifactRoot::Derived(r) => {
                b.charge_bytes(r.configuration().canonical_bytes().len())
            }
            crate::AnalysisArtifactRoot::WorkspaceStatus(r) => {
                b.charge_bytes(r.workspace().as_path().as_os_str().as_encoded_bytes().len())
            }
        },
        AnalysisValueKind::CppFragment(p) => {
            b.charge_bytes(p.projection().configuration_ref().canonical_bytes().len())
        }
        AnalysisValueKind::CcHeaderInfo(h) => headers(b, h, depth + 1),
        AnalysisValueKind::PackageSpecificationInfo(v) => owner::package_groups(b, v, depth + 1),
        AnalysisValueKind::CcToolchainFeatures(f) => {
            feature_storage::outer(b, false)?;
            features(b, f.model(), depth + 1)
        }
        AnalysisValueKind::CcFeatureConfiguration(f) => {
            feature_storage::outer(b, true)?;
            features(b, &f.model().model, depth + 1)?;
            feature_configuration_suffix(b, f.model())
        }
        AnalysisValueKind::TemplateVariableInfo(v) => {
            for (k, v) in v.variables() {
                text(b, k)?;
                text(b, v)?;
            }
            Ok(())
        }
        AnalysisValueKind::Runfiles(v) => runfiles(b, v.retained(), depth + 1),
        AnalysisValueKind::RunEnvironmentInfo(v) => {
            for (k, v) in v.environment() {
                text(b, k)?;
                text(b, v)?;
            }
            strings(b, v.inherited_environment())
        }
        AnalysisValueKind::Depset(d) => files(b, d, depth + 1),
        _ if b.owner_audit => owner::audit(b, value, depth),
        _ => Err(PublishedValueError::Unsupported(
            "native atom requires decomposed/audited class",
        )),
    }
}
pub(super) fn audit_depset_borrowed(
    b: &mut PublishedValueArenaBuilder,
    value: &crate::AnalysisDepset,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(0)?;
    b.charge_bytes(std::mem::size_of::<AnalysisValue>())?;
    files(b, value, 1)
}
pub(super) fn audit_package_specification(
    b: &mut PublishedValueArenaBuilder,
    value: &crate::PackageSpecificationInfo,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<AnalysisValue>())?;
    owner::package_groups(b, value, depth + 1)
}
pub(super) fn audit_configured_target(
    b: &mut PublishedValueArenaBuilder,
    value: &crate::ConfiguredTargetValue,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<AnalysisValue>())?;
    owner::configured_target(b, value, depth)
}
fn feature_strings<'a>(
    b: &mut PublishedValueArenaBuilder,
    values: impl IntoIterator<Item = &'a compact_str::CompactString>,
) -> Result<(), PublishedValueError> {
    for value in values {
        feature_storage::text(b, value)?;
    }
    Ok(())
}
pub(super) fn feature_outer(
    b: &mut PublishedValueArenaBuilder,
    configuration: bool,
) -> Result<(), PublishedValueError> {
    feature_storage::outer(b, configuration)
}
pub(super) fn feature_configuration_suffix(
    b: &mut PublishedValueArenaBuilder,
    c: &CcFeatureConfigurationModel,
) -> Result<(), PublishedValueError> {
    feature_storage::configuration_sets(b, c)?;
    feature_strings(b, c.requested.iter())?;
    feature_strings(b, c.enabled_features.iter())?;
    feature_strings(b, c.enabled_action_configs.iter())
}
fn predicates(
    b: &mut PublishedValueArenaBuilder,
    p: &CcSet<CcWithFeatureSet>,
) -> Result<(), PublishedValueError> {
    feature_storage::set(b, p)?;
    for p in p.iter() {
        b.charge_visit()?;
        feature_storage::set(b, &p.features)?;
        feature_storage::set(b, &p.not_features)?;
        feature_strings(b, p.features.iter())?;
        feature_strings(b, p.not_features.iter())?;
    }
    Ok(())
}
fn chunks(
    b: &mut PublishedValueArenaBuilder,
    c: &[CcStringChunk],
) -> Result<(), PublishedValueError> {
    feature_storage::array(b, c)?;
    for c in c {
        match c {
            CcStringChunk::Literal(s)
            | CcStringChunk::Variable(s)
            | CcStringChunk::RelativePath(s) => feature_storage::text(b, s)?,
        }
    }
    Ok(())
}
fn flags(
    b: &mut PublishedValueArenaBuilder,
    sets: &[CcFlagSet],
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.observe_depth(depth)?;
    feature_storage::array(b, sets)?;
    for s in sets {
        b.charge_visit()?;
        feature_storage::set(b, &s.actions)?;
        feature_storage::array(b, &s.flag_groups)?;
        feature_strings(b, s.actions.iter())?;
        predicates(b, &s.with_features)?;
        for g in &s.flag_groups {
            group(b, g, depth + 1)?;
        }
    }
    Ok(())
}
fn group(
    b: &mut PublishedValueArenaBuilder,
    g: &CcFlagGroup,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    for s in [
        &g.iterate_over,
        &g.expand_if_available,
        &g.expand_if_not_available,
        &g.expand_if_true,
        &g.expand_if_false,
    ]
    .into_iter()
    .flatten()
    {
        feature_storage::text(b, s)?;
    }
    if let Some(e) = &g.expand_if_equal {
        feature_storage::text(b, &e.name)?;
        feature_storage::text(b, &e.value)?;
    }
    feature_storage::array(b, &g.expandables)?;
    for e in &g.expandables {
        b.charge_visit()?;
        match e {
            CcExpandable::Flag(c) => chunks(b, c)?,
            CcExpandable::Group(g) => group(b, g, depth + 1)?,
        }
    }
    Ok(())
}
pub(super) fn features(
    b: &mut PublishedValueArenaBuilder,
    m: &CcToolchainFeatureModel,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    feature_storage::model(b, m)?;
    feature_storage::text(b, &m.tools_directory)?;
    feature_strings(b, &m.default_features_and_action_configs)?;
    for f in &m.features {
        b.charge_visit()?;
        feature_storage::feature(b, f)?;
        feature_storage::text(b, &f.name)?;
        flags(b, &f.flag_sets, depth + 1)?;
        feature_strings(b, &f.implies)?;
        feature_strings(b, &f.provides)?;
        for r in &f.requires {
            b.charge_visit()?;
            feature_storage::set(b, r)?;
            feature_strings(b, r.iter())?;
        }
        for s in &f.env_sets {
            b.charge_visit()?;
            feature_storage::set(b, &s.actions)?;
            feature_storage::array(b, &s.env_entries)?;
            feature_strings(b, s.actions.iter())?;
            predicates(b, &s.with_features)?;
            for e in &s.env_entries {
                b.charge_visit()?;
                feature_storage::text(b, &e.key)?;
                chunks(b, &e.value)?;
                if let Some(s) = &e.expand_if_available {
                    feature_storage::text(b, s)?;
                }
            }
        }
    }
    for a in &m.action_configs {
        b.charge_visit()?;
        feature_storage::action(b, a)?;
        feature_storage::text(b, &a.action_name)?;
        flags(b, &a.flag_sets, depth + 1)?;
        feature_strings(b, &a.implies)?;
        for t in &a.tools {
            b.charge_visit()?;
            text(b, t.path())?;
            b.charge_bytes(t.path_capacity())?;
            feature_storage::set(b, &t.execution_requirements)?;
            if let Some(a) = t.artifact() {
                artifact(b, a)?;
            }
            predicates(b, &t.with_features)?;
            feature_strings(b, t.execution_requirements.iter())?;
        }
    }
    for p in &m.artifact_name_patterns {
        b.charge_visit()?;
        feature_storage::text(b, &p.category)?;
        feature_storage::text(b, &p.prefix)?;
        feature_storage::text(b, &p.extension)?;
    }
    Ok(())
}
fn files(
    b: &mut PublishedValueArenaBuilder,
    d: &crate::AnalysisDepset,
    depth: usize,
) -> Result<(), PublishedValueError> {
    depset::class_depth(d.depth())?;
    b.observe_depth(depth)?;
    d.visit_storage_bounded(|event| {
        depset::event(b, event, depth, &mut |b, v, depth| audit(b, v, depth))
    })
}
fn runfiles(
    b: &mut PublishedValueArenaBuilder,
    r: &crate::RetainedRunfiles,
    depth: usize,
) -> Result<(), PublishedValueError> {
    files(b, &r.files, depth + 1)?;
    text(b, &r.repository_prefix)?;
    for d in [&r.symlinks, &r.root_symlinks] {
        depset::inspect(b, d, depth + 1, |b, s, _| {
            text(b, &s.path)?;
            artifact(b, &s.artifact)
        })?;
    }
    depset::inspect(b, &r.empty_filenames, depth + 1, |b, s, _| text(b, s))
}
pub(super) fn files_to_run(
    b: &mut PublishedValueArenaBuilder,
    f: &FilesToRunProvider,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    files(b, f.files(), depth + 1)?;
    if let Some(a) = &f.executable {
        artifact(b, a)?;
    }
    if let Some(s) = &f.support {
        runfiles(b, &s.runfiles, depth + 1)?;
        artifact(b, &s.tree)?;
        artifact(b, &s.input_manifest)?;
        for a in [&s.manifest, &s.repo_mapping_manifest]
            .into_iter()
            .flatten()
        {
            artifact(b, a)?;
        }
        for (k, v) in s.launch_environment.fixed().iter() {
            text(b, k)?;
            text(b, v)?;
        }
        for n in s.launch_environment.inherited().iter() {
            text(b, n)?;
        }
    }
    Ok(())
}

pub(super) fn provider_value(
    b: &mut PublishedValueArenaBuilder,
    value: &crate::ProviderValue,
    depth: usize,
) -> Result<(), PublishedValueError> {
    use crate::ProviderValue as P;
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<P>())?;
    match value {
        P::Occurrence(p) if b.owner_audit => owner::provider(b, p, depth + 1),
        P::Occurrence(_) => Err(PublishedValueError::Invalid(
            "occurrence must be decomposed",
        )),
        P::TemplateVariableInfo(v) => {
            for (k, v) in v.variables() {
                text(b, k)?;
                text(b, v)?;
            }
            Ok(())
        }
        P::RunEnvironmentInfo(v) => {
            for (k, v) in v.environment() {
                text(b, k)?;
                text(b, v)?;
            }
            strings(b, v.inherited_environment())
        }
        P::DefaultInfo(v) => {
            files(b, v.files(), depth + 1)?;
            runfiles(b, &v.default_runfiles, depth + 1)?;
            runfiles(b, &v.data_runfiles, depth + 1)?;
            if let Some(a) = &v.executable {
                artifact(b, a)?;
            }
            files_to_run(b, &v.files_to_run, depth + 1)
        }
        P::FilesToRunProvider(v) => files_to_run(b, v, depth + 1),
        P::OutputGroupInfo(v) => {
            for (name, d) in v.groups() {
                text(b, name)?;
                files(b, d, depth + 1)?;
            }
            Ok(())
        }
        P::PlatformInfo(v) => {
            text(b, &v.label)?;
            for (k, v) in v.constraints.iter().chain(v.exec_properties.iter()) {
                text(b, k)?;
                text(b, v)?;
            }
            Ok(())
        }
        P::XcodeVersionInfo(_) => Ok(()),
        P::FeatureFlagInfo(v) => text(b, v.value()),
        P::PackageSpecificationInfo(v) => owner::package_groups(b, v, depth + 1),
    }
}

fn headers(
    b: &mut PublishedValueArenaBuilder,
    h: &crate::CcHeaderInfo,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    let m = h.model();
    b.charge_bytes(std::mem::size_of::<crate::CcHeaderInfoInput>())?;
    for a in [
        &m.header_module,
        &m.pic_header_module,
        &m.separate_module,
        &m.separate_pic_module,
    ]
    .into_iter()
    .flatten()
    {
        artifact(b, a)?;
    }
    for a in m
        .modular_public_headers
        .iter()
        .chain(m.modular_private_headers.iter())
        .chain(m.textual_headers.iter())
        .chain(m.separate_module_headers.iter())
    {
        b.charge_bytes(std::mem::size_of::<AnalysisArtifact>())?;
        artifact(b, a)?;
    }
    for d in &m.deps {
        headers(b, d, depth + 1)?;
    }
    Ok(())
}

pub(super) fn audit_provider_occurrence(
    b: &mut PublishedValueArenaBuilder,
    value: &crate::ProviderOccurrence,
    depth: usize,
) -> Result<(), PublishedValueError> {
    owner::provider(b, value, depth)
}

#[cfg(test)]
#[path = "native_label_budget_tests.rs"]
mod native_label_budget_tests;

#[cfg(test)]
#[path = "native_target_budget_tests.rs"]
mod native_target_budget_tests;

#[cfg(test)]
#[path = "native_package_specification_tests.rs"]
mod native_package_specification_tests;

#[path = "feature_storage.rs"]
mod feature_storage;
