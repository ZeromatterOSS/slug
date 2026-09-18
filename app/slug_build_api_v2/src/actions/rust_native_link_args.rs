//! Retained projections for pinned rules_rust native-library argument mappers.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use starlark_map::small_set::SmallSet;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum RustNativeLinkArgMapper {
    Directories,
    DefaultDirect,
    DefaultIndirect,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RustNativeLinkArgsError(&'static str);

impl fmt::Display for RustNativeLinkArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rules_rust native link Args {}", self.0)
    }
}

impl std::error::Error for RustNativeLinkArgsError {}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedRustNativeLinkArgs {
    rows: Arc<[AnalysisValue]>,
    mapper: RustNativeLinkArgMapper,
}

impl RetainedRustNativeLinkArgs {
    pub fn new(
        rows: impl Into<Arc<[AnalysisValue]>>,
        mapper: RustNativeLinkArgMapper,
    ) -> Result<Self, RustNativeLinkArgsError> {
        let rows = rows.into();
        for row in rows.iter() {
            project(row, mapper)?;
        }
        Ok(Self { rows, mapper })
    }

    pub(super) fn render(&self) -> Vec<String> {
        self.rows
            .iter()
            .flat_map(|row| project(row, self.mapper).expect("validated native link Args"))
            .collect()
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.mapper == other.mapper
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(other.rows.iter())
                .all(|(left, right)| left.publication_eq_with(right, state))
    }
}

fn field<'a>(
    value: &'a AnalysisValue,
    name: &'static str,
) -> Result<&'a AnalysisValue, RustNativeLinkArgsError> {
    match value.kind() {
        AnalysisValueKind::Struct(fields) => fields.get(name),
        AnalysisValueKind::Provider(provider) => provider.field(name),
        _ => None,
    }
    .ok_or(RustNativeLinkArgsError(name))
}

fn sequence(value: &AnalysisValue) -> Result<&[AnalysisValue], RustNativeLinkArgsError> {
    match value.kind() {
        AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => Ok(values),
        _ => Err(RustNativeLinkArgsError("requires an ordered sequence")),
    }
}

fn boolean(value: &AnalysisValue) -> Result<bool, RustNativeLinkArgsError> {
    match value.kind() {
        AnalysisValueKind::Boolean(value) => Ok(value),
        _ => Err(RustNativeLinkArgsError("requires bool selectors")),
    }
}

fn regular_file(value: &AnalysisValue) -> Result<&AnalysisArtifact, RustNativeLinkArgsError> {
    match value.kind() {
        AnalysisValueKind::Artifact(artifact) if !matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.kind() != ActionOutputKind::File) => {
            Ok(artifact)
        }
        _ => Err(RustNativeLinkArgsError("requires regular Files")),
    }
}

fn optional_file(
    value: &AnalysisValue,
) -> Result<Option<&AnalysisArtifact>, RustNativeLinkArgsError> {
    if matches!(value.kind(), AnalysisValueKind::None) {
        Ok(None)
    } else {
        regular_file(value).map(Some)
    }
}

fn project(
    row: &AnalysisValue,
    mapper: RustNativeLinkArgMapper,
) -> Result<Vec<String>, RustNativeLinkArgsError> {
    let AnalysisValueKind::Tuple(row) = row.kind() else {
        return Err(RustNativeLinkArgsError("requires four-element tuple rows"));
    };
    let [input, pic, ambiguous, include] = row else {
        return Err(RustNativeLinkArgsError("requires four-element tuple rows"));
    };
    let use_pic = boolean(pic)?;
    let include = boolean(include)?;
    let AnalysisValueKind::Dictionary(ambiguous) = ambiguous.kind() else {
        return Err(RustNativeLinkArgsError("requires an ambiguity dictionary"));
    };
    for (key, value) in ambiguous {
        key.as_str()
            .ok_or(RustNativeLinkArgsError("ambiguity keys must be strings"))?;
        regular_file(value)?;
    }
    let user_flags = sequence(field(input, "user_link_flags")?)?;
    for flag in user_flags {
        flag.as_str()
            .ok_or(RustNativeLinkArgsError("user link flags must be strings"))?;
    }
    let mut result = Vec::new();
    let mut directories = SmallSet::new();
    for library in sequence(field(input, "libraries")?)? {
        let static_lib = optional_file(field(library, "static_library")?)?;
        let pic_lib = optional_file(field(library, "pic_static_library")?)?;
        let interface_lib = optional_file(field(library, "interface_library")?)?;
        let dynamic_lib = optional_file(field(library, "dynamic_library")?)?;
        let alwayslink = boolean(field(library, "alwayslink")?)?;
        let preferred = if use_pic {
            pic_lib.or(static_lib)
        } else {
            static_lib.or(pic_lib)
        }
        .or(interface_lib)
        .or(dynamic_lib)
        .ok_or(RustNativeLinkArgsError("requires a preferred library File"))?;
        if mapper == RustNativeLinkArgMapper::Directories {
            let directory = preferred.dirname();
            if directories.insert(directory.clone()) {
                result.push(directory);
            }
        } else if alwayslink {
            let prefix = if mapper == RustNativeLinkArgMapper::DefaultDirect {
                ""
            } else {
                "-Wl,"
            };
            result.extend([
                format!("-Clink-arg={prefix}--whole-archive"),
                format!("-Clink-arg={}", preferred.path()),
                format!("-Clink-arg={prefix}--no-whole-archive"),
            ]);
        } else if include {
            let short_path = preferred.short_path();
            let replacement = ambiguous
                .iter()
                .find(|(key, _)| key.as_str() == Some(short_path.as_ref()));
            let artifact = match replacement {
                Some((_, value)) => regular_file(value)?,
                None => preferred,
            };
            let path = artifact.path();
            let basename = path.rsplit('/').next().unwrap_or_default();
            let name = library_name(basename);
            if static_lib.is_some() || pic_lib.is_some() {
                result.push(format!("-lstatic={name}"));
                let standard = path.contains("lib/rustlib")
                    && ["libtest-", "libstd-", "test-", "std-"]
                        .iter()
                        .any(|prefix| basename.starts_with(prefix));
                if !standard {
                    result.push(format!("-Clink-arg=-l{name}"));
                }
            } else {
                result.push(format!("-ldylib={name}"));
            }
        }
    }
    if mapper != RustNativeLinkArgMapper::Directories {
        result.extend(user_flags.iter().map(|value| {
            format!(
                "--codegen=link-arg={}",
                value.as_str().expect("validated user flag")
            )
        }));
    }
    Ok(result)
}

// Pinned utils.get_lib_name_default, with Bazel StringModule.isDigit's ASCII rule.
fn library_name(basename: &str) -> String {
    let mut components = basename.split('.').collect::<Vec<_>>();
    while components
        .last()
        .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        components.pop();
    }
    components.pop();
    let name = components.join(".");
    name.strip_prefix("lib").unwrap_or(&name).to_owned()
}
