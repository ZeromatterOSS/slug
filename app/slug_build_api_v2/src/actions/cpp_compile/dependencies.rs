//! Supplied-byte reconciliation only: no acquisition, discovery state or execution authority.
use compact_str::CompactString;
use slug_configuration_v2::CppFragmentProjection;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use super::CppCompileSpec;
use super::input_sets::CppCompileInputSets;
use super::input_sets::require_regular;
use crate::AnalysisArtifact;
use crate::AnalysisValueKind;
use crate::cc_features::normalize_cc_path_posix;

mod dotd;

#[derive(Debug, Eq, PartialEq)]
pub struct CppCompileDependencies {
    discovered: Vec<AnalysisArtifact>,
    post_discovery: Vec<AnalysisArtifact>,
}
impl CppCompileDependencies {
    pub fn discovered(&self) -> &[AnalysisArtifact] {
        &self.discovered
    }
    pub fn post_discovery(&self) -> &[AnalysisArtifact] {
        &self.post_discovery
    }
}

fn normalize(path: &str) -> Result<CompactString, String> {
    normalize_cc_path_posix(path).map_err(|e| e.to_string())
}

/// PathFragment component prefix, including the empty relative fragment.
pub(super) fn beneath(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return !path.starts_with('/');
    }
    path == prefix
        || (prefix == "/" && path.starts_with('/'))
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(super) fn context_directories(
    spec: &CppCompileSpec,
    field: &str,
) -> Result<Vec<CompactString>, String> {
    let AnalysisValueKind::Provider(context) = spec.data().compilation_context.kind() else {
        return Err("CppCompile requires a compilation context provider".into());
    };
    let value = context
        .fields()
        .get(field)
        .ok_or_else(|| format!("CppCompile compilation context is missing {field}"))?;
    let AnalysisValueKind::Depset(paths) = value.kind() else {
        return Err(format!(
            "CppCompile compilation context {field} must be a depset of strings"
        ));
    };
    paths
        .to_list()
        .iter()
        .map(|path| {
            let path = path.as_str().ok_or_else(|| {
                format!("CppCompile compilation context {field} must be a depset of strings")
            })?;
            normalize(path)
        })
        .collect()
}

/// CppCompileAction.validateInclusions is separate from HeaderDiscovery's
/// resolution checks. It consumes the action configuration, not the toolchain's.
fn validate_inclusions(
    spec: &CppCompileSpec,
    sets: &CppCompileInputSets,
    discovered: &SmallSet<AnalysisArtifact>,
    mut ignored: Vec<CompactString>,
) -> Result<(), String> {
    let strict = CppFragmentProjection::new(spec.object_output().root().configuration().clone())
        .map_err(|e| e.to_string())?
        .strict_system_includes()
        .map_err(|e| e.to_string())?;
    if !strict {
        ignored.extend(context_directories(spec, "system_includes")?);
        ignored.extend(context_directories(spec, "external_includes")?);
    }
    let allowed: SmallSet<_> = sets
        .mandatory()
        .iter()
        .chain(sets.additional())
        .cloned()
        .collect();
    let mut undeclared = Vec::new();
    for artifact in discovered {
        let path = artifact.path();
        // Native final-validation exemptions, not module-action admission.
        if [".pcm", ".gcm", ".ifc", "module.modulemap"]
            .iter()
            .any(|suffix| path.ends_with(suffix))
            || allowed.contains(artifact)
            || ignored.iter().any(|p| beneath(&path, p))
        {
            continue;
        }
        undeclared.push(path.into_owned());
    }
    if !undeclared.is_empty() {
        return Err(format!(
            "CppCompile undeclared inclusions: {}",
            undeclared.join(", ")
        ));
    }
    Ok(())
}

impl CppCompileSpec {
    /// Apply the named POSIX, case-sensitive, NOOP-mapped regular-file slice.
    /// `resolve_source` owns repository/package resolution legality and must return
    /// an existing authoritative source artifact. The type/path checks below are
    /// defensive, not authority. This method supplies no Core observation capability.
    pub fn reconcile_dotd_posix(
        &self,
        exec_root: &str,
        bytes: &[u8],
        mut resolve_source: impl FnMut(&str) -> Result<Option<AnalysisArtifact>, String>,
    ) -> Result<CppCompileDependencies, String> {
        let exec_root = normalize(exec_root)?;
        if !exec_root.starts_with('/') {
            return Err("CppCompile POSIX exec root must be absolute".into());
        }
        let sets = self.input_sets()?;
        let tokens = dotd::parse(bytes)?;
        let builtin = self
            .data()
            .builtin_include_directories
            .iter()
            .map(|p| normalize(p))
            .collect::<Result<Vec<_>, _>>()?;
        let mut derived = SmallMap::new();
        for artifact in sets.allowed_derived() {
            if matches!(artifact, AnalysisArtifact::Derived { .. }) {
                let path = artifact.path();
                if !derived.contains_key(path.as_ref()) {
                    derived.insert(CompactString::from(path.as_ref()), artifact);
                }
            }
        }
        let mut discovered = SmallSet::new();
        let mut outside = Vec::new();
        let mut unresolved = Vec::new();
        for token in tokens {
            // DependencySet resolves relative entries against execRoot before
            // HeaderDiscovery examines absolute builtin prefixes/root containment.
            let absolute = if token.starts_with('/') {
                normalize(&token)?
            } else {
                normalize(&format!("{exec_root}/{token}"))?
            };
            if builtin
                .iter()
                .any(|p| p.starts_with('/') && beneath(&absolute, p))
            {
                continue;
            }
            if !beneath(&absolute, &exec_root) {
                outside.push(absolute);
                continue;
            }
            let relative = if exec_root == "/" {
                &absolute[1..]
            } else {
                absolute
                    .strip_prefix(exec_root.as_str())
                    .unwrap()
                    .trim_start_matches('/')
            };
            let artifact = if let Some(artifact) = derived.get(relative) {
                Some((*artifact).clone())
            } else {
                let artifact = resolve_source(relative)?;
                if let Some(artifact) = &artifact {
                    if !matches!(artifact, AnalysisArtifact::Source(_))
                        || artifact.path() != relative
                    {
                        return Err(format!(
                            "CppCompile source resolver returned a non-source or mismatching artifact for {relative}"
                        ));
                    }
                    require_regular(artifact)?;
                }
                artifact
            };
            match artifact {
                Some(artifact) if &artifact != self.source() => {
                    discovered.insert(artifact);
                }
                Some(_) => {}
                // PathFragment.getFileExtension ignores a dot at index zero;
                // root `.cppmap` differs from `pkg/.cppmap` after relativization.
                None if relative.len() > ".cppmap".len() && relative.ends_with(".cppmap") => {}
                None => unresolved.push(CompactString::from(relative)),
            }
        }
        if self.data().needs_include_validation {
            if !outside.is_empty() {
                return Err(format!(
                    "CppCompile absolute path inclusions outside exec root: {}",
                    outside.join(", ")
                ));
            }
            if !unresolved.is_empty() {
                return Err(format!(
                    "CppCompile unresolved inclusions: {}",
                    unresolved.join(", ")
                ));
            }
            validate_inclusions(self, &sets, &discovered, builtin)?;
        }
        let post_discovery = sets
            .mandatory()
            .iter()
            .chain(discovered.iter())
            .cloned()
            .collect::<SmallSet<_>>();
        Ok(CppCompileDependencies {
            discovered: discovered.into_iter().collect(),
            post_discovery: post_discovery.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests;
