/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Toolchain resolution algorithm.
//!
//! Implements Bazel's toolchain resolution: given a target's required toolchain
//! types and the current platform, select concrete toolchain implementations
//! from the set of registered toolchains.
//!
//! The algorithm:
//! 1. Collect available execution platforms (host platform for now)
//! 2. For each exec platform, find first compatible toolchain per required type
//! 3. Select first exec platform that satisfies ALL mandatory types
//!
//! Constraint matching: a toolchain's exec_compatible_with/target_compatible_with
//! matches a platform if every constraint_value in the list is present in the
//! platform's constraint set.

use std::collections::HashMap;
use std::collections::HashSet;

use super::native_rule_analysis::DeclaredToolchainInfo;
use super::native_rule_analysis::get_declared_toolchains;

/// A required toolchain type for a rule.
#[derive(Debug, Clone)]
pub struct RequiredToolchainType {
    /// The toolchain_type label (e.g., "@bazel_tools//tools/cpp:toolchain_type")
    pub type_label: String,
    /// Whether this toolchain is mandatory (true) or optional (false)
    pub mandatory: bool,
}

/// Result of toolchain resolution for a single target.
#[derive(Debug, Clone)]
pub struct ToolchainResolutionResult {
    /// The selected execution platform label
    pub exec_platform: String,
    /// Map of toolchain_type label → resolved toolchain implementation label.
    /// None means the toolchain type was optional and no match was found.
    pub resolved_toolchains: HashMap<String, Option<ResolvedToolchain>>,
}

/// A resolved toolchain — the concrete implementation to use.
#[derive(Debug, Clone)]
pub struct ResolvedToolchain {
    /// The toolchain() target label (the registration wrapper)
    pub toolchain_target: String,
    /// The actual toolchain implementation target (the `toolchain` attr value)
    pub toolchain_impl: String,
    /// The toolchain_type this satisfies
    pub toolchain_type: String,
}

/// A platform represented as a set of constraint_value labels.
#[derive(Debug, Clone)]
pub struct PlatformConstraints {
    /// The platform label (e.g., "@local_config_platform//:host")
    pub label: String,
    /// Set of constraint_value labels this platform has
    /// (e.g., {"@platforms//os:linux", "@platforms//cpu:x86_64"})
    pub constraint_values: HashSet<String>,
}

impl PlatformConstraints {
    /// Create platform constraints for the current host.
    pub fn host_platform() -> Self {
        let mut constraint_values = HashSet::new();

        // Detect host OS
        match std::env::consts::OS {
            "linux" => {
                constraint_values.insert("@platforms//os:linux".to_owned());
            }
            "macos" => {
                constraint_values.insert("@platforms//os:osx".to_owned());
            }
            "windows" => {
                constraint_values.insert("@platforms//os:windows".to_owned());
            }
            _ => {}
        }

        // Detect host CPU
        match std::env::consts::ARCH {
            "x86_64" => {
                constraint_values.insert("@platforms//cpu:x86_64".to_owned());
            }
            "aarch64" => {
                constraint_values.insert("@platforms//cpu:aarch64".to_owned());
            }
            "x86" => {
                constraint_values.insert("@platforms//cpu:x86_32".to_owned());
            }
            _ => {}
        }

        PlatformConstraints {
            label: "@local_config_platform//:host".to_owned(),
            constraint_values,
        }
    }

    /// Check if this platform satisfies all the given constraint requirements.
    ///
    /// Returns true if every constraint_value in `required` is present in this
    /// platform's constraint set. Empty requirements always match.
    pub fn satisfies(&self, required: &[String]) -> bool {
        for req in required {
            // Normalize the label for comparison (strip @@ prefix, handle aliases)
            let normalized = normalize_constraint_label(req);
            if !self.constraint_values.iter().any(|cv| {
                let norm_cv = normalize_constraint_label(cv);
                norm_cv == normalized
            }) {
                return false;
            }
        }
        true
    }
}

/// Normalize a label for comparison.
///
/// Handles:
/// - `@@platforms//os:linux` → `platforms//os:linux`
/// - `@platforms//os:linux` → `platforms//os:linux`
/// - `platforms//os:linux` → `platforms//os:linux` (already normalized)
/// - `//os:linux` → `//os:linux` (relative, kept as-is)
fn normalize_constraint_label(label: &str) -> String {
    // Strip all leading @ characters
    let label = label.trim_start_matches('@');
    label.to_owned()
}

/// Resolve toolchains for a target.
///
/// This is the core Bazel toolchain resolution algorithm adapted for kuro.
///
/// # Arguments
/// - `required_types`: The toolchain types the target's rule requires
/// - `target_platform`: The target platform's constraints
/// - `exec_platforms`: Available execution platforms (ordered by priority)
/// - `target_exec_constraints`: Additional exec constraints from the target
///
/// # Returns
/// A `ToolchainResolutionResult` with the selected exec platform and resolved
/// toolchains, or an error if mandatory toolchains can't be satisfied.
pub fn resolve_toolchains(
    required_types: &[RequiredToolchainType],
    target_platform: &PlatformConstraints,
    exec_platforms: &[PlatformConstraints],
    target_exec_constraints: &[String],
) -> Result<ToolchainResolutionResult, String> {
    if required_types.is_empty() {
        // No toolchains needed — use first exec platform
        let exec = exec_platforms
            .first()
            .map(|p| p.label.clone())
            .unwrap_or_else(|| "@local_config_platform//:host".to_owned());
        return Ok(ToolchainResolutionResult {
            exec_platform: exec,
            resolved_toolchains: HashMap::new(),
        });
    }

    // Get all declared toolchains from the global registry
    let declared = get_declared_toolchains();
    tracing::debug!(
        "Declared toolchains registry has {} entries. Required: {:?}",
        declared.len(),
        required_types
            .iter()
            .map(|r| &r.type_label)
            .collect::<Vec<_>>()
    );
    if !declared.is_empty() {
        for (label, info) in declared.iter().take(5) {
            tracing::debug!("  Declared: {} (type='{}')", label, info.toolchain_type);
        }
        if declared.len() > 5 {
            tracing::debug!("  ... and {} more", declared.len() - 5);
        }
    }

    // Step 1: Filter exec platforms by target's exec_compatible_with
    let eligible_exec_platforms: Vec<&PlatformConstraints> = exec_platforms
        .iter()
        .filter(|p| p.satisfies(target_exec_constraints))
        .collect();

    if eligible_exec_platforms.is_empty() {
        return Err(
            "No eligible execution platforms after filtering by target exec constraints".to_owned(),
        );
    }

    // Step 2: For each exec platform, find first compatible toolchain per type
    let mut platform_results: Vec<(
        &PlatformConstraints,
        HashMap<String, Option<ResolvedToolchain>>,
    )> = Vec::new();

    for exec_platform in &eligible_exec_platforms {
        let mut type_results: HashMap<String, Option<ResolvedToolchain>> = HashMap::new();

        for req in required_types {
            let mut found = None;

            // Search declared toolchains in registration order (priority)
            for (tc_label, tc_info) in &declared {
                // Check toolchain_type matches
                let tc_type_norm = normalize_constraint_label(&tc_info.toolchain_type);
                let req_type_norm = normalize_constraint_label(&req.type_label);
                if tc_type_norm != req_type_norm {
                    continue;
                }

                // Check exec platform compatibility
                if !exec_platform.satisfies(&tc_info.exec_compatible_with) {
                    continue;
                }

                // Check target platform compatibility
                if !target_platform.satisfies(&tc_info.target_compatible_with) {
                    continue;
                }

                // First match wins
                found = Some(ResolvedToolchain {
                    toolchain_target: tc_label.clone(),
                    toolchain_impl: tc_info.toolchain_impl.clone(),
                    toolchain_type: tc_info.toolchain_type.clone(),
                });
                break;
            }

            type_results.insert(req.type_label.clone(), found);
        }

        platform_results.push((exec_platform, type_results));
    }

    // Step 3: Select first exec platform that satisfies ALL mandatory types
    for (exec_platform, type_results) in &platform_results {
        let all_mandatory_satisfied = required_types.iter().all(|req| {
            if req.mandatory {
                type_results
                    .get(&req.type_label)
                    .map(|r| r.is_some())
                    .unwrap_or(false)
            } else {
                true // optional types don't block platform selection
            }
        });

        if all_mandatory_satisfied {
            return Ok(ToolchainResolutionResult {
                exec_platform: exec_platform.label.clone(),
                resolved_toolchains: type_results.clone(),
            });
        }
    }

    // No platform satisfies all mandatory types
    let missing_types: Vec<&str> = required_types
        .iter()
        .filter(|req| req.mandatory)
        .map(|req| req.type_label.as_str())
        .collect();
    Err(format!(
        "No execution platform found that provides all mandatory toolchain types: {:?}",
        missing_types
    ))
}

/// A request to resolve toolchains for one exec group.
#[derive(Debug, Clone)]
pub struct ExecGroupResolutionRequest {
    /// Group name ("default" for the rule-level toolchains).
    pub group_name: String,
    /// Toolchain types this group requires.
    pub required_types: Vec<RequiredToolchainType>,
    /// Additional exec constraints for this group.
    pub exec_constraints: Vec<String>,
}

/// Result of resolving all exec groups for a target.
#[derive(Debug, Clone)]
pub struct MultiGroupResolutionResult {
    /// Per-group results keyed by group name.
    pub groups: HashMap<String, ToolchainResolutionResult>,
}

/// Resolve toolchains for multiple exec groups independently.
///
/// Each exec group gets its own call to `resolve_toolchains()` with its own
/// required types and exec constraints. Different groups may select different
/// execution platforms.
pub fn resolve_toolchains_multi_group(
    requests: &[ExecGroupResolutionRequest],
    target_platform: &PlatformConstraints,
    exec_platforms: &[PlatformConstraints],
) -> Result<MultiGroupResolutionResult, String> {
    let mut groups = HashMap::new();
    for req in requests {
        let result = resolve_toolchains(
            &req.required_types,
            target_platform,
            exec_platforms,
            &req.exec_constraints,
        )?;
        groups.insert(req.group_name.clone(), result);
    }
    Ok(MultiGroupResolutionResult { groups })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_requirements() {
        let target_platform = PlatformConstraints::host_platform();
        let exec_platforms = vec![PlatformConstraints::host_platform()];

        let result = resolve_toolchains(&[], &target_platform, &exec_platforms, &[]).unwrap();

        assert!(result.resolved_toolchains.is_empty());
    }

    #[test]
    fn test_constraint_matching() {
        let platform = PlatformConstraints {
            label: "test".to_owned(),
            constraint_values: HashSet::from([
                "@platforms//os:linux".to_owned(),
                "@platforms//cpu:x86_64".to_owned(),
            ]),
        };

        // Empty requirements always match
        assert!(platform.satisfies(&[]));

        // Single matching constraint
        assert!(platform.satisfies(&["@platforms//os:linux".to_owned()]));

        // Multiple matching constraints
        assert!(platform.satisfies(&[
            "@platforms//os:linux".to_owned(),
            "@platforms//cpu:x86_64".to_owned(),
        ]));

        // Non-matching constraint
        assert!(!platform.satisfies(&["@platforms//os:windows".to_owned()]));

        // Partial match (one matches, one doesn't)
        assert!(!platform.satisfies(&[
            "@platforms//os:linux".to_owned(),
            "@platforms//cpu:aarch64".to_owned(),
        ]));
    }

    #[test]
    fn test_normalize_constraint_label() {
        assert_eq!(
            normalize_constraint_label("@@platforms//os:linux"),
            "platforms//os:linux"
        );
        assert_eq!(
            normalize_constraint_label("@platforms//os:linux"),
            "platforms//os:linux"
        );
        assert_eq!(
            normalize_constraint_label("platforms//os:linux"),
            "platforms//os:linux"
        );
    }

    #[test]
    fn test_multi_group_resolution_empty() {
        let target_platform = PlatformConstraints::host_platform();
        let exec_platforms = vec![PlatformConstraints::host_platform()];

        let requests = vec![
            ExecGroupResolutionRequest {
                group_name: "default".to_owned(),
                required_types: vec![],
                exec_constraints: vec![],
            },
            ExecGroupResolutionRequest {
                group_name: "link".to_owned(),
                required_types: vec![],
                exec_constraints: vec![],
            },
        ];

        let result =
            resolve_toolchains_multi_group(&requests, &target_platform, &exec_platforms).unwrap();
        assert_eq!(result.groups.len(), 2);
        assert!(result.groups.contains_key("default"));
        assert!(result.groups.contains_key("link"));
    }
}
