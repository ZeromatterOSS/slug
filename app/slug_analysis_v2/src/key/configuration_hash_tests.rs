//! Operational bucket work; canonical and complete identity remain independent.
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::discriminant;
use std::mem::size_of;

use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;

use super::*;

#[derive(Default)]
struct Work {
    writes: usize,
    bytes: usize,
}
impl Hasher for Work {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.bytes += bytes.len();
    }
}
#[derive(Default)]
struct Stream(Vec<u8>);
impl Hasher for Stream {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

// Independently derived original enum field stream, including its compiler's
// actual enum discriminant. References retain the original fields' Hash calls.
#[derive(Hash)]
enum OriginalIdentity<'a> {
    Slug(&'a SlugConfiguration),
    Legacy {
        kind: &'a ConfigurationKind,
        checksum: &'a ConfigurationChecksum,
        starlark_options: &'a StarlarkOptions,
    },
}
fn original(identity: &ConfigurationIdentity) -> OriginalIdentity<'_> {
    match identity {
        ConfigurationIdentity::Slug(config) => OriginalIdentity::Slug(config),
        ConfigurationIdentity::Legacy {
            kind,
            checksum,
            starlark_options,
        } => OriginalIdentity::Legacy {
            kind,
            checksum,
            starlark_options,
        },
    }
}
fn expected_bucket(key: &ConfigurationKey, state: &mut impl Hasher) {
    match &key.identity {
        ConfigurationIdentity::Slug(config) => {
            discriminant(&key.identity).hash(state);
            state.write(config.projection().as_bytes());
        }
        ConfigurationIdentity::Legacy { .. } => original(&key.identity).hash(state),
    }
}
fn bytes(value: &impl Hash) -> Vec<u8> {
    let mut stream = Stream::default();
    value.hash(&mut stream);
    stream.0
}
fn configuration() -> SlugConfiguration {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    SlugConfiguration::default_target(&host)
        .unwrap()
        .with_host_platform_label(&CanonicalLabel::parse("@@//platform:host").unwrap())
}
fn option(value: &str) -> StarlarkOption {
    StarlarkOption::string(
        CanonicalLabel::parse("@@//settings:flag").unwrap(),
        value,
        StarlarkOptionScope::Default,
    )
}
fn variants() -> Vec<ConfigurationKey> {
    let base = configuration();
    let native = base
        .with_command_configuration(
            base.starlark_options().clone(),
            &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
                NativeCommandOption::CompilationMode,
                Some("dbg"),
                false,
            )]),
        )
        .unwrap();
    vec![
        ConfigurationKey::from_slug(base.clone()),
        ConfigurationKey::from_slug(base.to_exec().unwrap()),
        ConfigurationKey::from_slug(native),
        ConfigurationKey::from_slug(base.with_starlark_option(option("changed"))),
        ConfigurationKey::from_slug(
            base.with_transition_target_platform(Some(
                &CanonicalLabel::parse("@@//platform:target").unwrap(),
            ))
            .unwrap(),
        ),
        ConfigurationKey::from_slug(
            base.with_host_platform_label(
                &CanonicalLabel::parse("@@//platform:other_host").unwrap(),
            ),
        ),
    ]
}

#[test]
fn configuration_key_fresh_construction_hashes_existing_projection() {
    let config = configuration();
    assert_eq!(config.option_count(), 341);
    assert!(
        config.canonical_bytes().len() > 4096,
        "use a real sizeable canonical option vector"
    );
    let mut actual = Work::default();
    let mut expected = Work::default();
    for _ in 0..64 {
        // Fresh wrappers, not cached object-hash reuse, must have bounded work.
        let key = ConfigurationKey::from_slug(config.clone());
        key.hash(&mut actual);
        expected_bucket(&key, &mut expected);
    }
    assert_eq!(
        (actual.writes, actual.bytes),
        (expected.writes, expected.bytes)
    );
    let key = ConfigurationKey::from_slug(config.clone());
    let mut prefix = Stream::default();
    discriminant(&key.identity).hash(&mut prefix);
    assert_eq!(expected.bytes, 64 * (prefix.0.len() + 32));
    assert_eq!(
        bytes(&config),
        config.canonical_bytes(),
        "generic Slug Hash retains its complete canonical stream"
    );
}

#[test]
fn configuration_key_discriminant_and_legacy_stream_match_original() {
    let slug = ConfigurationKey::from_slug(configuration());
    let mut prefix = Stream::default();
    discriminant(&slug.identity).hash(&mut prefix);
    let mut full = prefix.0.clone();
    full.extend_from_slice(slug.slug_configuration().unwrap().canonical_bytes());
    assert_eq!(bytes(&slug.identity), full);
    assert_eq!(bytes(&original(&slug.identity)), full);
    for kind in [
        ConfigurationKind::Target,
        ConfigurationKind::Exec,
        ConfigurationKind::HostLike,
    ] {
        let bare =
            ConfigurationKey::new(kind, ConfigurationChecksum::new("legacy.value-1").unwrap());
        let changed = bare.with_starlark_option(option("quoted_'_%_雪"));
        let removed = changed.without_starlark_option(option("unused").label());
        for key in [&bare, &changed, &removed] {
            assert_eq!(bytes(key), bytes(&key.identity));
            assert_eq!(
                bytes(key),
                bytes(&original(&key.identity)),
                "Legacy preserves original discriminant and every field byte"
            );
        }
        assert_eq!(removed, bare);
        assert_ne!(discriminant(&bare.identity), discriminant(&slug.identity));
    }
}

#[test]
fn configuration_bucket_preserves_full_identity_and_layout() {
    let keys = variants();
    let independent = ConfigurationKey::from_slug(configuration());
    assert_eq!(keys[0], independent);
    assert_eq!(bytes(&keys[0]), bytes(&independent));
    assert_eq!(
        size_of::<ConfigurationKey>(),
        size_of::<ConfigurationIdentity>()
    );
    #[allow(dead_code)]
    struct OriginalTargetLayout {
        label: CanonicalLabel,
        configuration: ConfigurationKey,
        should_apply_rule_transition: bool,
        toolchain_execution_platform: Option<Arc<CanonicalLabel>>,
    }
    assert_eq!(
        size_of::<ConfiguredTargetKey>(),
        size_of::<OriginalTargetLayout>()
    );
    // Every bucket collides. Complete Eq, never a digest/bucket, selects entries.
    let mut map = HashMap::<ConfigurationKey, usize, BuildHasherDefault<Work>>::default();
    for (index, key) in keys.iter().enumerate() {
        assert!(map.insert(key.clone(), index).is_none());
        assert_eq!(key == &key.clone(), key.identity == key.clone().identity);
        assert_eq!(
            format!("{key:?}"),
            format!("ConfigurationKey {{ identity: {:?} }}", key.identity)
        );
        for other in &keys {
            assert_eq!(key.cmp(other), key.identity.cmp(&other.identity));
        }
    }
    assert_eq!(map.len(), keys.len());
    assert_eq!(map.get(&independent), Some(&0));
    for (index, key) in keys.iter().enumerate() {
        assert_eq!(map.get(key), Some(&index));
    }
    for key in keys.iter().skip(1) {
        assert_ne!(key, &keys[0]);
        assert_ne!(
            key.complete_identity_bytes(),
            keys[0].complete_identity_bytes()
        );
    }
}

#[test]
fn configured_target_hash_preserves_complete_artifact_identity() {
    for configuration in variants() {
        let label = CanonicalLabel::parse("@@//pkg:target").unwrap();
        for target in [
            ConfiguredTargetKey::new(label.clone(), configuration.clone()),
            ConfiguredTargetKey::without_rule_transition(label.clone(), configuration.clone()),
            ConfiguredTargetKey::new(label.clone(), configuration.clone())
                .with_toolchain_execution_platform(Arc::new(
                    CanonicalLabel::parse("@@//platform:exec").unwrap(),
                )),
        ] {
            let mut expected = Stream::default();
            target.label.hash(&mut expected);
            expected_bucket(target.configuration(), &mut expected);
            target.should_apply_rule_transition.hash(&mut expected);
            target.toolchain_execution_platform.hash(&mut expected);
            assert_eq!(bytes(&target), expected.0);
            let canonical = configuration
                .slug_configuration()
                .unwrap()
                .canonical_bytes();
            assert_eq!(configuration.complete_identity_bytes().as_ref(), canonical);
            let owner =
                slug_build_api_v2::AnalysisConfiguredTargetKey::new(label.clone(), canonical);
            let owner = match target.toolchain_execution_platform() {
                Some(platform) => owner.with_toolchain_execution_platform(platform.clone()),
                None => owner,
            };
            assert_eq!(target.artifact_owner(), owner);
        }
    }
}
