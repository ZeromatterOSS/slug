//! Retained projections for the admitted pinned rules_rust dependency mappers.

use std::convert::Infallible;
use std::fmt;

use allocative::Allocative;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ProviderOccurrence;
use crate::analysis_value::PublicationEqState;

const CRATE_SOURCE: &str = "@@rules_rust+//rust/private:providers.bzl";
const ALIAS_SOURCE: &str = "@@rules_rust+//rust/private:rustc.bzl";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum RustCrateArgMapper {
    Extern,
    ExternMetadata,
    DependencyDir,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RustCrateArgsError(&'static str);

impl fmt::Display for RustCrateArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rules_rust crate Args {}", self.0)
    }
}

impl std::error::Error for RustCrateArgsError {}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedRustCrateArgs {
    crates: AnalysisDepset,
    mapper: RustCrateArgMapper,
}

impl RetainedRustCrateArgs {
    pub fn new(
        crates: AnalysisDepset,
        mapper: RustCrateArgMapper,
    ) -> Result<Self, RustCrateArgsError> {
        crates.visit(|value| project(value, mapper).map(|_| ()))?;
        Ok(Self { crates, mapper })
    }

    pub(super) fn render(&self) -> Vec<String> {
        let mut values = Vec::new();
        self.crates
            .visit(|value| {
                let (name, artifact) = project(value, self.mapper).expect("validated crate Args");
                values.push(match name {
                    Some(name) => format!("--extern={name}={}", artifact.path()),
                    None => artifact.dirname(),
                });
                Ok::<_, Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        values
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.mapper == other.mapper && self.crates.publication_eq_with(&other.crates, state)
    }
}

fn is_provider(provider: &ProviderOccurrence, source: &str, name: &str) -> bool {
    provider
        .identity()
        .user_id()
        .is_some_and(|id| id.source_label() == source && id.exported_name() == name)
}

fn crate_provider(value: &AnalysisValue) -> Result<&ProviderOccurrence, RustCrateArgsError> {
    match value.kind() {
        AnalysisValueKind::Provider(provider)
            if is_provider(provider, CRATE_SOURCE, "CrateInfo") =>
        {
            Ok(provider)
        }
        _ => Err(RustCrateArgsError("requires pinned CrateInfo")),
    }
}

fn field<'a>(
    provider: &'a ProviderOccurrence,
    name: &'static str,
) -> Result<&'a AnalysisValue, RustCrateArgsError> {
    provider.field(name).ok_or(RustCrateArgsError(name))
}

fn regular_file(value: &AnalysisValue) -> Result<&AnalysisArtifact, RustCrateArgsError> {
    match value.kind() {
        AnalysisValueKind::Artifact(artifact) if !matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.kind() != ActionOutputKind::File) => {
            Ok(artifact)
        }
        _ => Err(RustCrateArgsError("requires a regular File")),
    }
}

fn project(
    value: &AnalysisValue,
    mapper: RustCrateArgMapper,
) -> Result<(Option<&str>, &AnalysisArtifact), RustCrateArgsError> {
    if mapper == RustCrateArgMapper::DependencyDir {
        return Ok((
            None,
            regular_file(field(crate_provider(value)?, "output")?)?,
        ));
    }
    let provider = match value.kind() {
        AnalysisValueKind::Provider(provider) => provider,
        _ => {
            return Err(RustCrateArgsError(
                "requires pinned CrateInfo or AliasableDepInfo",
            ));
        }
    };
    let info = if is_provider(provider, ALIAS_SOURCE, "AliasableDepInfo") {
        crate_provider(field(provider, "dep")?)?
    } else {
        crate_provider(value)?
    };
    let name = field(provider, "name")?
        .as_str()
        .ok_or(RustCrateArgsError("name must be a string"))?;
    if mapper == RustCrateArgMapper::ExternMetadata {
        let metadata = field(info, "metadata")?;
        if !matches!(metadata.kind(), AnalysisValueKind::None) {
            let metadata = regular_file(metadata)?;
            match field(info, "metadata_supports_pipelining")?.kind() {
                AnalysisValueKind::Boolean(true) => return Ok((Some(name), metadata)),
                AnalysisValueKind::Boolean(false) => {}
                _ => {
                    return Err(RustCrateArgsError(
                        "metadata_supports_pipelining must be a bool",
                    ));
                }
            }
        }
    }
    Ok((Some(name), regular_file(field(info, "output")?)?))
}
