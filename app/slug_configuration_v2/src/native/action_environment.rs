use std::collections::BTreeMap;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use super::configuration::OptionValue;
use super::configuration::SlugConfiguration;
use super::configuration::SlugConfigurationError;
use super::host::ActionEnvironmentHost;
use super::host::ActionEnvironmentHostOs;
use super::host::HostPathFlavor;
use super::path::NormalizedBazelPath;
use super::value::EnvValue;
use super::value::NativeOccurrence;
use super::value::NativeValue;
use super::value::TriState;

const CORE_OPTIONS: &str = "com.google.devtools.build.lib.analysis.config.CoreOptions";
const SHELL_OPTIONS: &str = "com.google.devtools.build.lib.analysis.ShellConfiguration.Options";
const STRICT_OPTIONS: &str =
    "com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions";

#[derive(
    Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe
)]
pub struct CanonicalStringMap(Arc<[(CompactString, CompactString)]>);

impl CanonicalStringMap {
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<CompactString>,
        V: Into<CompactString>,
    {
        let mut ordered = BTreeMap::new();
        for (key, value) in pairs {
            ordered.insert(key.into(), value.into());
        }
        Self(Arc::from(ordered.into_iter().collect::<Vec<_>>()))
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(key))
            .ok()
            .map(|index| self.0[index].1.as_str())
    }
}

#[derive(
    Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe
)]
pub struct CanonicalStringSet(Arc<[CompactString]>);

impl CanonicalStringSet {
    pub fn from_values<I, V>(values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<CompactString>,
    {
        let mut values = values.into_iter().map(Into::into).collect::<Vec<_>>();
        values.sort_unstable();
        values.dedup();
        Self(Arc::from(values))
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(CompactString::as_str)
    }
    pub fn contains(&self, value: &str) -> bool {
        self.0
            .binary_search_by(|candidate| candidate.as_str().cmp(value))
            .is_ok()
    }
}

/// Immutable action environment before client-inherited values are resolved.
#[derive(
    Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe
)]
pub struct RetainedActionEnvironment {
    fixed: CanonicalStringMap,
    inherited: CanonicalStringSet,
}

impl RetainedActionEnvironment {
    fn from_entries(entries: BTreeMap<CompactString, Option<CompactString>>) -> Self {
        let mut fixed = Vec::new();
        let mut inherited = Vec::new();
        for (name, value) in entries {
            match value {
                Some(value) => fixed.push((name, value)),
                None => inherited.push(name),
            }
        }
        Self {
            fixed: CanonicalStringMap(Arc::from(fixed)),
            inherited: CanonicalStringSet(Arc::from(inherited)),
        }
    }
    pub fn fixed(&self) -> &CanonicalStringMap {
        &self.fixed
    }
    pub fn inherited(&self) -> &CanonicalStringSet {
        &self.inherited
    }
    pub fn for_action<I, K, V>(
        &self,
        use_default_shell_environment: bool,
        action_environment: I,
    ) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<CompactString>,
        V: Into<CompactString>,
    {
        let action = CanonicalStringMap::from_pairs(action_environment);
        if !use_default_shell_environment {
            return Self {
                fixed: action,
                inherited: CanonicalStringSet::default(),
            };
        }
        if action.0.is_empty() {
            return self.dupe();
        }
        Self {
            fixed: CanonicalStringMap::from_pairs(self.fixed.iter().chain(action.iter())),
            inherited: CanonicalStringSet::from_values(
                self.inherited
                    .iter()
                    .filter(|name| action.get(name).is_none()),
            ),
        }
    }
}

impl SlugConfiguration {
    pub fn configured_action_path_flavor(&self) -> Result<HostPathFlavor, SlugConfigurationError> {
        let host = self
            .action_environment_host()
            .ok_or(action_environment_error("missing retained Host facts"))?;
        Ok(if host.os() == ActionEnvironmentHostOs::Windows {
            HostPathFlavor::Windows
        } else {
            HostPathFlavor::Unix
        })
    }

    pub fn configured_action_environment(
        &self,
    ) -> Result<RetainedActionEnvironment, SlugConfigurationError> {
        let host = self
            .action_environment_host()
            .ok_or(action_environment_error("missing retained Host facts"))?;
        ensure_deferred_options_are_default(self)?;
        let strict = strict_action_environment(self)?;
        let mut entries = BTreeMap::new();
        if !strict {
            entries.insert(CompactString::new("LD_LIBRARY_PATH"), None);
        }
        if strict {
            entries.insert(
                CompactString::new("PATH"),
                Some(CompactString::new(
                    if host.os() == ActionEnvironmentHostOs::Windows {
                        windows_path_with_client(host, None)?
                    } else {
                        "/bin:/usr/bin:/usr/local/bin".to_owned()
                    },
                )),
            );
        } else if host.os() == ActionEnvironmentHostOs::Windows {
            entries.insert(
                CompactString::new("PATH"),
                Some(CompactString::new(windows_path_with_client(
                    host,
                    host.path(),
                )?)),
            );
        } else {
            entries.insert(CompactString::new("PATH"), None);
        }
        apply_environment_rows(self, &mut entries)?;
        if host.os() == ActionEnvironmentHostOs::Windows {
            entries.insert(
                CompactString::new("RUNFILES_MANIFEST_ONLY"),
                Some(CompactString::new("1")),
            );
        }
        Ok(RetainedActionEnvironment::from_entries(entries))
    }
}
fn strict_action_environment(
    configuration: &SlugConfiguration,
) -> Result<bool, SlugConfigurationError> {
    match configuration.option_value(STRICT_OPTIONS, "incompatible_strict_action_env")? {
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(value))) => Ok(*value),
        _ => Err(action_environment_error("invalid strict option state")),
    }
}
fn ensure_deferred_options_are_default(
    configuration: &SlugConfiguration,
) -> Result<(), SlugConfigurationError> {
    if !matches!(
        configuration.option_value(SHELL_OPTIONS, "shell_executable")?,
        OptionValue::Native(NativeOccurrence::Absent)
    ) {
        return Err(action_environment_error(
            "explicit shell_executable is deferred",
        ));
    }
    if !matches!(
        configuration.option_value(CORE_OPTIONS, "enable_runfiles")?,
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Tri(TriState::Auto)))
    ) {
        return Err(action_environment_error(
            "explicit enable_runfiles is deferred",
        ));
    }
    Ok(())
}
fn apply_environment_rows(
    configuration: &SlugConfiguration,
    entries: &mut BTreeMap<CompactString, Option<CompactString>>,
) -> Result<(), SlugConfigurationError> {
    let values = match configuration.option_value(CORE_OPTIONS, "action_env")? {
        OptionValue::Native(NativeOccurrence::List(values))
        | OptionValue::Native(NativeOccurrence::Scalar(NativeValue::List(values))) => values,
        OptionValue::Native(NativeOccurrence::Absent) => return Ok(()),
        _ => return Err(action_environment_error("invalid action_env option state")),
    };
    for value in values.iter() {
        match value {
            NativeValue::Env(EnvValue::Set(name, value)) => {
                entries.insert(name.clone(), Some(value.clone()));
            }
            NativeValue::Env(EnvValue::Inherit(name)) => {
                entries.insert(name.clone(), None);
            }
            NativeValue::Env(EnvValue::Unset(name)) => {
                entries.remove(name.as_str());
            }
            _ => {
                return Err(action_environment_error("invalid action_env member"));
            }
        }
    }
    Ok(())
}
fn action_environment_error(reason: &'static str) -> SlugConfigurationError {
    SlugConfigurationError::ActionEnvironment { reason }
}
fn windows_path_with_client(
    host: &ActionEnvironmentHost,
    client_path: Option<&str>,
) -> Result<String, SlugConfigurationError> {
    let shell = host.bazel_sh().unwrap_or("c:/msys64/usr/bin/bash.exe");
    let shell = normalize_windows_path(shell)?;
    let prefix = windows_shell_prefix(&shell);
    let system_root = host
        .system_root()
        .filter(|value| !value.is_empty())
        .unwrap_or("C:\\Windows");
    let mut path = format!(
        "{prefix};{system_root};{system_root}\\System32;{system_root}\\System32\\WindowsPowerShell\\v1.0"
    );
    if let Some(client_path) = client_path {
        path.push(';');
        path.push_str(client_path);
    }
    Ok(path)
}

fn windows_shell_prefix(shell: &str) -> String {
    let Some(parent) = parent_directory(shell) else {
        return String::new();
    };
    let mut result = parent.replace('/', "\\");
    let sibling = if ends_with_fragment(parent, "usr/bin") {
        parent_directory(parent)
            .and_then(parent_directory)
            .map(|value| relative(value, "bin"))
    } else if ends_with_fragment(parent, "bin") {
        parent_directory(parent).map(|value| relative(&relative(value, "usr"), "bin"))
    } else {
        None
    };
    if let Some(sibling) = sibling {
        result.push(';');
        result.push_str(&sibling.replace('/', "\\"));
    }
    result
}

fn normalize_windows_path(path: &str) -> Result<String, SlugConfigurationError> {
    NormalizedBazelPath::new(HostPathFlavor::Windows, path)
        .map(|path| path.as_str().to_owned())
        .map_err(|_| {
            action_environment_error(
                "Windows BAZEL_SH short paths require a Host filesystem observation",
            )
        })
}

fn parent_directory(path: &str) -> Option<&str> {
    let drive = path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/';
    match path.rfind('/') {
        Some(0) => (path.len() > 1).then_some("/"),
        Some(2) if drive => (path.len() > 3).then_some(&path[..3]),
        Some(index) => Some(&path[..index]),
        None if path.is_empty() => None,
        None => Some(""),
    }
}

fn ends_with_fragment(path: &str, suffix: &str) -> bool {
    path == suffix
        || path
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('/'))
}

fn relative(path: &str, child: &str) -> String {
    if path.is_empty() {
        child.to_owned()
    } else if path.ends_with('/') {
        format!("{path}{child}")
    } else {
        format!("{path}/{child}")
    }
}

#[cfg(test)]
mod tests {
    use slug_identity_v2::CanonicalLabel;

    use super::*;
    use crate::CommandConfigurationOccurrence;
    use crate::CommandConfigurationOverlay;
    use crate::NativeCommandOption;
    use crate::native::StarlarkOptions;
    use crate::native::host::AutoCpuToken;
    use crate::native::host::HostConversionInputs;
    use crate::native::host::HostPathFlavor;

    fn configuration(
        host: ActionEnvironmentHost,
        occurrences: Vec<CommandConfigurationOccurrence>,
    ) -> SlugConfiguration {
        let path_flavor = if host.os() == ActionEnvironmentHostOs::Windows {
            HostPathFlavor::Windows
        } else {
            HostPathFlavor::Unix
        };
        let host = HostConversionInputs::new(
            Some(if path_flavor == HostPathFlavor::Windows {
                AutoCpuToken::X64Windows
            } else {
                AutoCpuToken::K8
            }),
            Some(path_flavor),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap()
        .with_action_environment_host(host);
        SlugConfiguration::default_target(&host)
            .unwrap()
            .with_command_configuration(
                StarlarkOptions::default(),
                &CommandConfigurationOverlay::from(occurrences),
            )
            .unwrap()
    }

    fn native(option: NativeCommandOption, value: &str) -> CommandConfigurationOccurrence {
        CommandConfigurationOccurrence::native(option, Some(value), false)
    }

    #[test]
    fn canonical_collections_ignore_input_order_and_keep_last_duplicate() {
        let first = CanonicalStringMap::from_pairs([("b", "2"), ("a", "1"), ("b", "3")]);
        let second = CanonicalStringMap::from_pairs([("a", "1"), ("b", "3")]);
        assert_eq!(first, second);
        assert_eq!(first.iter().collect::<Vec<_>>(), [("a", "1"), ("b", "3")]);
        assert_eq!(first.get("b"), Some("3"));
        assert_eq!(first.get("missing"), None);

        let first = CanonicalStringSet::from_values(["b", "a", "b"]);
        let second = CanonicalStringSet::from_values(["a", "b"]);
        assert_eq!(first, second);
        assert_eq!(first.iter().collect::<Vec<_>>(), ["a", "b"]);
        assert!(first.contains("b"));
    }

    #[test]
    fn action_composition_matches_default_and_explicit_precedence() {
        let configured = RetainedActionEnvironment {
            fixed: CanonicalStringMap::from_pairs([("FIXED", "configured"), ("BOTH", "fixed")]),
            inherited: CanonicalStringSet::from_values(["DYNAMIC", "BOTH"]),
        };
        let isolated = configured.for_action(false, [("ONLY", "action")]);
        assert_eq!(
            isolated.fixed().iter().collect::<Vec<_>>(),
            [("ONLY", "action")]
        );
        assert_eq!(isolated.inherited().iter().next(), None);

        let composed = configured.for_action(
            true,
            [
                ("FIXED", "action"),
                ("DYNAMIC", "override"),
                ("NEW", "value"),
            ],
        );
        assert_eq!(composed.fixed().get("FIXED"), Some("action"));
        assert_eq!(composed.fixed().get("DYNAMIC"), Some("override"));
        assert_eq!(composed.fixed().get("BOTH"), Some("fixed"));
        assert_eq!(composed.fixed().get("NEW"), Some("value"));
        assert!(!composed.inherited().contains("DYNAMIC"));
        assert!(composed.inherited().contains("BOTH"));
    }

    #[test]
    fn windows_path_normalization_and_shell_siblings_match_bazel_shapes() {
        assert_eq!(
            normalize_windows_path("c:/plain/path").unwrap(),
            "c:/plain/path"
        );
        assert_eq!(
            normalize_windows_path("d:\\foo\\.\\bar\\..\\shell").unwrap(),
            "D:/foo/shell"
        );
        assert_eq!(
            windows_shell_prefix("c:/msys64/usr/bin/bash.exe"),
            "c:\\msys64\\usr\\bin;c:\\msys64\\bin"
        );
        assert_eq!(
            windows_shell_prefix("D:/tools/bin/bash.exe"),
            "D:\\tools\\bin;D:\\tools\\usr\\bin"
        );
        assert_eq!(windows_shell_prefix("D:/foo/shell"), "D:\\foo");
        assert_eq!(
            normalize_windows_path("C:/PROGRA~1/bash.exe"),
            Err(SlugConfigurationError::ActionEnvironment {
                reason: "Windows BAZEL_SH short paths require a Host filesystem observation",
            })
        );
    }

    #[test]
    fn retained_types_are_allocative_and_cheap_to_clone() {
        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<CanonicalStringMap>();
        allocative::<CanonicalStringSet>();
        allocative::<RetainedActionEnvironment>();
        dupe::<CanonicalStringMap>();
        dupe::<CanonicalStringSet>();
        dupe::<RetainedActionEnvironment>();
    }

    #[test]
    fn configured_environment_applies_strict_runfiles_and_option_rows() {
        let linux = configuration(
            ActionEnvironmentHost::without_environment(ActionEnvironmentHostOs::Linux),
            vec![
                native(NativeCommandOption::ActionEnv, "SET=first"),
                native(NativeCommandOption::ActionEnv, "DYNAMIC"),
                native(NativeCommandOption::ActionEnv, "=SET"),
                native(NativeCommandOption::ActionEnv, "SET="),
                native(NativeCommandOption::ActionEnv, "DYNAMIC=fixed"),
            ],
        );
        let environment = linux.configured_action_environment().unwrap();
        assert_eq!(
            environment.fixed().get("PATH"),
            Some("/bin:/usr/bin:/usr/local/bin")
        );
        assert_eq!(environment.fixed().get("SET"), Some(""));
        assert_eq!(environment.fixed().get("DYNAMIC"), Some("fixed"));
        assert!(!environment.inherited().contains("DYNAMIC"));
        assert_eq!(environment.fixed().get("RUNFILES_MANIFEST_ONLY"), None);

        let windows = configuration(ActionEnvironmentHost::windows(None, None, None), vec![]);
        let environment = windows.configured_action_environment().unwrap();
        assert_eq!(
            environment.fixed().get("PATH"),
            Some(
                "c:\\msys64\\usr\\bin;c:\\msys64\\bin;C:\\Windows;C:\\Windows\\System32;C:\\Windows\\System32\\WindowsPowerShell\\v1.0"
            )
        );
        assert_eq!(environment.fixed().get("RUNFILES_MANIFEST_ONLY"), Some("1"));
    }

    #[test]
    fn non_strict_environment_inherits_unix_and_uses_windows_server_path() {
        let strict_off = CommandConfigurationOccurrence::native(
            NativeCommandOption::IncompatibleStrictActionEnv,
            Some("false"),
            false,
        );
        for os in [
            ActionEnvironmentHostOs::Linux,
            ActionEnvironmentHostOs::Macos,
            ActionEnvironmentHostOs::Freebsd,
            ActionEnvironmentHostOs::Openbsd,
            ActionEnvironmentHostOs::Unknown,
        ] {
            let strict = configuration(ActionEnvironmentHost::without_environment(os), vec![])
                .configured_action_environment()
                .unwrap();
            assert_eq!(
                strict.fixed().get("PATH"),
                Some("/bin:/usr/bin:/usr/local/bin")
            );
            assert!(!strict.inherited().contains("LD_LIBRARY_PATH"));

            let inherited = configuration(
                ActionEnvironmentHost::without_environment(os),
                vec![strict_off.clone()],
            )
            .configured_action_environment()
            .unwrap();
            assert!(inherited.inherited().contains("PATH"));
            assert!(inherited.inherited().contains("LD_LIBRARY_PATH"));
            assert_eq!(inherited.fixed().iter().next(), None);
        }

        let windows = configuration(
            ActionEnvironmentHost::windows(
                Some("D:/tools/bin/bash.exe"),
                Some("C:/client/bin"),
                Some("D:\\Windows"),
            ),
            vec![strict_off],
        )
        .configured_action_environment()
        .unwrap();
        assert_eq!(
            windows.fixed().get("PATH"),
            Some(
                "D:\\tools\\bin;D:\\tools\\usr\\bin;D:\\Windows;D:\\Windows\\System32;D:\\Windows\\System32\\WindowsPowerShell\\v1.0;C:/client/bin"
            )
        );
        assert!(windows.inherited().contains("LD_LIBRARY_PATH"));
    }

    #[test]
    fn exec_projection_uses_host_action_environment_and_preserves_host_identity() {
        let target = configuration(
            ActionEnvironmentHost::without_environment(ActionEnvironmentHostOs::Linux),
            vec![
                native(NativeCommandOption::ActionEnv, "MODE=target"),
                native(NativeCommandOption::HostActionEnv, "MODE=exec"),
                native(NativeCommandOption::HostActionEnv, "HOST_ONLY"),
            ],
        );
        let exec = target
            .to_exec_for_platform(&CanonicalLabel::parse("@@//:platform").unwrap())
            .unwrap();
        assert_eq!(
            target
                .configured_action_environment()
                .unwrap()
                .fixed()
                .get("MODE"),
            Some("target")
        );
        let environment = exec.configured_action_environment().unwrap();
        assert_eq!(environment.fixed().get("MODE"), Some("exec"));
        assert!(environment.inherited().contains("HOST_ONLY"));
    }

    #[test]
    fn host_fact_is_structural_restores_and_missing_fails_closed() {
        let linux_host = ActionEnvironmentHost::without_environment(ActionEnvironmentHostOs::Linux);
        let first = configuration(linux_host.dupe(), vec![]);
        let changed = configuration(
            ActionEnvironmentHost::without_environment(ActionEnvironmentHostOs::Macos),
            vec![],
        );
        let restored = configuration(linux_host.dupe(), vec![]);
        assert_ne!(first, changed);
        assert_ne!(first.canonical_bytes(), changed.canonical_bytes());
        assert_eq!(first, restored);
        assert_eq!(first.canonical_bytes(), restored.canonical_bytes());

        let option_first = configuration(
            linux_host.dupe(),
            vec![native(NativeCommandOption::ActionEnv, "MODE=first")],
        );
        let option_changed = configuration(
            linux_host.dupe(),
            vec![native(NativeCommandOption::ActionEnv, "MODE=changed")],
        );
        let option_restored = configuration(
            linux_host,
            vec![native(NativeCommandOption::ActionEnv, "MODE=first")],
        );
        assert_ne!(option_first, option_changed);
        assert_ne!(
            option_first.configured_action_environment().unwrap(),
            option_changed.configured_action_environment().unwrap()
        );
        assert_eq!(option_first, option_restored);
        assert_eq!(
            option_first.configured_action_environment().unwrap(),
            option_restored.configured_action_environment().unwrap()
        );

        let missing = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap();
        assert_eq!(
            SlugConfiguration::default_target(&missing)
                .unwrap()
                .configured_action_environment(),
            Err(SlugConfigurationError::ActionEnvironment {
                reason: "missing retained Host facts",
            })
        );
    }
}
