/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use slug_commands_v2::CommandParseError;
use slug_commands_v2::RepositoryEnvironmentEntry;
use slug_commands_v2::RepositoryEnvironmentOverride;
use slug_commands_v2::RepositoryEnvironmentSnapshot;

pub(crate) fn capture_repository_environment(
    workspace: &Path,
    overrides: &[RepositoryEnvironmentOverride],
) -> Result<RepositoryEnvironmentSnapshot, CommandParseError> {
    // The iterator owns one process snapshot. Do not perform a second ambient
    // read while applying inherit operations or preparing daemon transport.
    capture_repository_environment_from(workspace, overrides, std::env::vars_os())
}

pub(crate) fn capture_repository_environment_from(
    workspace: &Path,
    overrides: &[RepositoryEnvironmentOverride],
    entries: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<RepositoryEnvironmentSnapshot, CommandParseError> {
    let mut original = BTreeMap::<String, Arc<str>>::new();
    for (index, (name, value)) in entries.into_iter().enumerate() {
        let name = name
            .into_string()
            .map_err(|_| non_unicode_entry(index, "name"))?;
        let value = value
            .into_string()
            .map_err(|_| non_unicode_entry(index, "value"))?;
        original.insert(name, Arc::from(value));
    }

    let mut effective = original.clone();
    let workspace = workspace.display().to_string();
    for operation in overrides {
        match operation {
            RepositoryEnvironmentOverride::Set { name, value } => {
                effective.insert(
                    name.clone(),
                    Arc::from(value.replace("%bazel_workspace%", &workspace)),
                );
            }
            RepositoryEnvironmentOverride::Inherit { name } => {
                if let Some(value) = original.get(name) {
                    effective.insert(name.clone(), value.clone());
                }
            }
            RepositoryEnvironmentOverride::Unset { name } => {
                effective.remove(name);
            }
        }
    }

    RepositoryEnvironmentSnapshot::from_canonical(Arc::from(
        effective
            .into_iter()
            .map(|(name, value)| RepositoryEnvironmentEntry::new(name, value))
            .collect::<Vec<_>>(),
    ))
    .map_err(|error| CommandParseError::InvalidFlagValue {
        flag: "client repository environment".to_owned(),
        message: error.to_string(),
    })
}

fn non_unicode_entry(index: usize, field: &str) -> CommandParseError {
    CommandParseError::InvalidFlagValue {
        flag: format!("client environment entry #{}", index + 1),
        message: format!("{field} is not valid Unicode"),
    }
}

pub(crate) fn redacted_repository_environment_argv(argv: &[String]) -> Vec<&str> {
    argv.iter()
        .map(|arg| slug_commands_v2::redact_repository_environment_arg(arg))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        entries
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect()
    }

    #[test]
    fn applies_set_inherit_unset_in_occurrence_order_against_original() {
        let operations = [
            RepositoryEnvironmentOverride::Set {
                name: "A".to_owned(),
                value: "overlay".to_owned(),
            },
            RepositoryEnvironmentOverride::Inherit {
                name: "A".to_owned(),
            },
            RepositoryEnvironmentOverride::Unset {
                name: "B".to_owned(),
            },
            RepositoryEnvironmentOverride::Set {
                name: "MISSING".to_owned(),
                value: "preserved".to_owned(),
            },
            RepositoryEnvironmentOverride::Inherit {
                name: "MISSING".to_owned(),
            },
            RepositoryEnvironmentOverride::Set {
                name: "EMPTY".to_owned(),
                value: String::new(),
            },
        ];
        let snapshot = capture_repository_environment_from(
            Path::new("/workspace"),
            &operations,
            environment(&[("B", "remove"), ("A", "original")]),
        )
        .unwrap();
        assert_eq!(snapshot.get("A").unwrap().as_ref(), "original");
        assert_eq!(snapshot.get("B"), None);
        assert_eq!(snapshot.get("EMPTY").unwrap().as_ref(), "");
        assert_eq!(snapshot.get("MISSING").unwrap().as_ref(), "preserved");
        assert_eq!(
            snapshot
                .iter()
                .map(|entry| entry.name())
                .collect::<Vec<_>>(),
            ["A", "EMPTY", "MISSING"]
        );
    }

    #[test]
    fn expands_every_workspace_placeholder_and_redacts_argv() {
        let secret = "sentinel-secret";
        let snapshot = capture_repository_environment_from(
            Path::new("/ws"),
            &[RepositoryEnvironmentOverride::Set {
                name: "A".to_owned(),
                value: "%bazel_workspace%/x/%bazel_workspace%".to_owned(),
            }],
            environment(&[("SECRET", secret)]),
        )
        .unwrap();
        assert_eq!(snapshot.get("A").unwrap().as_ref(), "/ws/x//ws");
        assert!(!format!("{snapshot:?}").contains(secret));
        assert_eq!(
            redacted_repository_environment_argv(&[
                "--repo_env=SECRET=sentinel-secret".to_owned(),
                "//pkg:target".to_owned(),
            ]),
            ["--repo_env=<redacted>", "//pkg:target"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_unicode_without_reproducing_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let secret = OsString::from_vec(vec![b's', 0xff, b'x']);
        let error = capture_repository_environment_from(
            Path::new("/workspace"),
            &[],
            [(OsString::from("A"), secret)],
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("entry #1"));
        assert!(message.contains("value is not valid Unicode"));
        assert!(!message.contains("sx"));
    }
}
