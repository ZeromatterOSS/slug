//! Pure no-scanning preparation. Success is not an execution-readiness certificate.
use compact_str::CompactString;
use starlark_map::small_set::SmallSet;

use super::CppCompileInputSets;
use super::CppCompileSpec;
use super::dependencies::beneath;
use super::dependencies::context_directories;
use crate::cc_features::CcVariableAccess;
use crate::cc_features::normalize_cc_path_posix;
use crate::cc_variables::OwnedCcVariableAccess;

#[derive(Debug, Eq, PartialEq)]
pub struct CppCompileInputPreparation {
    inputs: CppCompileInputSets,
    warnings: Vec<CompactString>,
}
impl CppCompileInputPreparation {
    pub fn inputs(&self) -> &CppCompileInputSets {
        &self.inputs
    }
    pub fn warnings(&self) -> &[CompactString] {
        &self.warnings
    }
}

fn normalize(path: &str) -> Result<CompactString, String> {
    normalize_cc_path_posix(path).map_err(|e| e.to_string())
}
fn msvc_suffix(path: &str) -> bool {
    path.get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("msvc"))
}
fn msvc_flag(option: &str) -> bool {
    let bytes = option.as_bytes();
    bytes.len() >= 6
        && matches!(bytes[0], b'-' | b'/')
        && matches!(bytes[1], b'i' | b'I')
        && msvc_suffix(&option[2..])
}

#[derive(Clone, Copy)]
enum FlagKind {
    Quote,
    System,
}

fn option_paths(
    options: &[CompactString],
    kind: FlagKind,
    warnings: &mut Vec<CompactString>,
) -> Result<Vec<CompactString>, String> {
    let mut paths = Vec::new();
    let mut index = 0;
    while let Some(option) = options.get(index) {
        index += 1;
        let flag = match kind {
            FlagKind::Quote if option.starts_with("-iquote") => "-iquote",
            FlagKind::System if option.starts_with("-isystem") => "-isystem",
            FlagKind::System if msvc_flag(option) => &option[..6],
            _ => continue,
        };
        if option.len() > flag.len() {
            // java.lang.String.trim strips only characters <= U+0020.
            paths.push(normalize(
                option[flag.len()..].trim_matches(|c| c <= '\u{20}'),
            )?);
        } else if let Some(path) = options.get(index) {
            index += 1;
            paths.push(normalize(path)?);
        } else {
            warnings.push(format!("dangling {flag} include option").into());
        }
    }
    Ok(paths)
}

fn raw_copts(variables: &mut OwnedCcVariableAccess<'_>) -> Result<Vec<CompactString>, String> {
    let Some(value) = variables
        .lookup("user_compile_flags", false)
        .map_err(|e| e.to_string())?
    else {
        return Ok(Vec::new());
    };
    variables
        .sequence(&value, "user_compile_flags")
        .map_err(|e| e.to_string())?
        .map(|value| variables.string(&value?, "user_compile_flags"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

impl CppCompileSpec {
    /// Observe original-cache discovery options and validate admitted POSIX include
    /// paths. No filesystem access, discovered-input state or execution admission.
    pub fn prepare_inputs_posix(&self) -> Result<CppCompileInputPreparation, String> {
        let data = self.data();
        // discoverInputs/getCompilerOptions passes null overwrittenVariables:
        // unlike execution argv, this does NOT create the empty overwrite child.
        let mut variables = OwnedCcVariableAccess::original(&data.variables);
        let options = data
            .feature_configuration
            .get_command_line(&data.action_name, &mut variables)
            .map_err(|e| e.to_string())?;
        let mut warnings = Vec::new();
        let system = option_paths(&options, FlagKind::System, &mut warnings)?;
        if data.needs_include_validation {
            // Pre-execution ignores declared system dirs even under strict-system
            // policy. The stricter post-dotd validation is a separate operation.
            let mut ignored = data
                .builtin_include_directories
                .iter()
                .map(|p| normalize(p))
                .collect::<Result<SmallSet<_>, _>>()?;
            ignored.extend(context_directories(self, "system_includes")?);
            ignored.extend(context_directories(self, "external_includes")?);
            let mut ordinary = context_directories(self, "includes")?;
            let copts = raw_copts(&mut variables)?;
            for option in &copts {
                if let Some(path) = option
                    .strip_prefix("-I")
                    .or_else(|| option.strip_prefix("/I"))
                {
                    if !path.is_empty() && !msvc_suffix(path) {
                        ordinary.push(normalize(path)?);
                    }
                }
            }
            let mut quote = context_directories(self, "quote_includes")?;
            quote.extend(option_paths(&copts, FlagKind::Quote, &mut warnings)?);
            for path in ordinary.iter().chain(&quote).chain(&system) {
                if ignored.contains(path) || ignored.iter().any(|prefix| beneath(path, prefix)) {
                    continue;
                }
                // Nonsibling external repository coordinates share the execroot.
                let checked = if path == "external" {
                    ""
                } else {
                    path.strip_prefix("external/").unwrap_or(path)
                };
                if checked.starts_with('/') || checked.split('/').any(|part| part == "..") {
                    return Err(format!(
                        "The include path '{checked}' references a path outside of the execution root."
                    ));
                }
            }
        }
        Ok(CppCompileInputPreparation {
            inputs: self.input_sets()?,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests;
