//! Exact scoped offline admission over the original selected BCR request.
use super::*;
use crate::runtime::dice::source_service::OfflineArchiveBacking;

pub(super) const PROTOBUF_URL: &str = "https://github.com/protocolbuffers/protobuf/releases/download/v33.4/protobuf-33.4.bazel.tar.gz";
pub(super) const PROTOBUF_SRI: &str = "sha256-aH6YpHGXO1xf1xF1DEC4uCwK3jP2Sdtl4AspDyk0Wis=";
pub(super) const MODULE_SRI: &str = "sha256-EUd1uBazi20MpiBFDWsCVQxgzu39yNmiKYM7NKIj3EI=";
pub(super) const PROTOBUF_DIGEST: [u8; 32] = [
    0x68, 0x7e, 0x98, 0xa4, 0x71, 0x97, 0x3b, 0x5c, 0x5f, 0xd7, 0x11, 0x75, 0x0c, 0x40, 0xb8, 0xb8,
    0x2c, 0x0a, 0xde, 0x33, 0xf6, 0x49, 0xdb, 0x65, 0xe0, 0x0b, 0x29, 0x0f, 0x29, 0x34, 0x5a, 0x2b,
];

pub(super) const PROTOBUF_NATIVE_BYTE_BUDGET: u64 = 64 << 20;
const ARCHIVE_MIRROR: &str = "tests/v2_oracle/fixtures/configured-cli-authentic/mirror";
const PREPARED_MODULE: &str = "target/wp954/registry/modules/protobuf/33.4/MODULE.bazel";

pub(super) fn native_byte_budget_backing(bodies: &Bodies) -> bool {
    let Some(project) = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
    else {
        return false;
    };
    let (Some(backing), Some(module)) = (&bodies.offline_archive, &bodies.offline_archive_module)
    else {
        return false;
    };
    backing.matches(PROTOBUF_URL, &PROTOBUF_DIGEST)
        && backing
            .path()
            .as_path()
            .strip_prefix(project)
            .and_then(|relative| relative.strip_prefix(ARCHIVE_MIRROR))
            .is_ok_and(|relative| {
                Some(relative) == PROTOBUF_URL.strip_prefix("https://").map(Path::new)
            })
        && module
            .as_path()
            .strip_prefix(project)
            .is_ok_and(|relative| relative == Path::new(PREPARED_MODULE))
}

fn string_is(value: Option<&OverrideAttributeValue>, expected: &str) -> bool {
    matches!(value, Some(OverrideAttributeValue::String(value)) if value.as_str() == expected)
}
fn singleton(value: Option<&OverrideAttributeValue>) -> Option<&str> {
    match value {
        Some(OverrideAttributeValue::Iterable(values)) => match values.as_ref() {
            [OverrideAttributeValue::String(value)] => Some(value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn matches(
    bodies: &Bodies,
    request: &slug_bzlmod_v2::RepositoryMaterializationRequest,
) -> bool {
    let (Some(backing), Some(module)) = (&bodies.offline_archive, &bodies.offline_archive_module)
    else {
        return false;
    };
    let label = &request.repo_spec.rule_id.bzl_file;
    let attrs = &request.repo_spec.attributes;
    request.id.workspace == bodies.roots[0]
        && request.id.canonical_repo.as_str() == "protobuf+"
        && matches!(&request.kind, RepositoryMaterializationKind::Immutable)
        && label.mapping_id().is_none()
        && label.package().repo().as_str() == "bazel_tools"
        && label.package().package().as_str() == "tools/build_defs/repo"
        && label.target().as_str() == "http.bzl"
        && request.repo_spec.rule_id.rule_name.as_str() == "http_archive"
        && backing.matches(PROTOBUF_URL, &PROTOBUF_DIGEST)
        && attrs.len() == 9
        && singleton(attrs.get("urls")) == Some(PROTOBUF_URL)
        && string_is(attrs.get("integrity"), PROTOBUF_SRI)
        && string_is(attrs.get("strip_prefix"), "protobuf-33.4")
        && ["remote_patches", "remote_file_urls", "remote_file_integrity"].into_iter().all(|key| {
            matches!(attrs.get(key), Some(OverrideAttributeValue::Map(values)) if values.is_empty())
        })
        && singleton(attrs.get("remote_module_file_urls")).and_then(|url| url.strip_prefix("file://"))
            .is_some_and(|path| Some(path) == module.as_path().to_str())
        && string_is(attrs.get("remote_module_file_integrity"), MODULE_SRI)
        && matches!(attrs.get("remote_patch_strip"), Some(OverrideAttributeValue::Int(0)))
}

pub(super) fn configure(
    mut bodies: Bodies,
    registry: RegistryChoice,
    command: CommandChoice,
    backing: Arc<OfflineArchiveBacking>,
    module: NormalizedAbsolutePath,
) -> Bodies {
    if public_probe_preparation_tests::offline_archive_native_role(registry, command) {
        bodies.offline_archive = Some(backing);
        bodies.offline_archive_module = Some(module);
    }
    bodies
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use starlark_map::small_map::SmallMap;

    use super::*;
    use crate::runtime::repository_io::source_service::SourceCounter;

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }
    fn configured(registry: RegistryChoice, command: CommandChoice) -> Bodies {
        configure(
            Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]),
            registry,
            command,
            OfflineArchiveBacking::new(PROTOBUF_URL, PROTOBUF_DIGEST, path("/archive.tar.gz")),
            path("/prepared/MODULE.bazel"),
        )
    }
    fn request() -> RepositoryMaterializationRequest {
        let string = |s: &str| OverrideAttributeValue::String(s.into());
        let list = |s: &str| OverrideAttributeValue::Iterable(Arc::new([string(s)]));
        let empty = || OverrideAttributeValue::Map(Arc::new(SmallMap::new()));
        let fields: [(CompactString, OverrideAttributeValue); 9] = [
            ("urls".into(), list(PROTOBUF_URL)),
            ("integrity".into(), string(PROTOBUF_SRI)),
            ("strip_prefix".into(), string("protobuf-33.4")),
            ("remote_patches".into(), empty()),
            ("remote_file_urls".into(), empty()),
            ("remote_file_integrity".into(), empty()),
            (
                "remote_module_file_urls".into(),
                list("file:///prepared/MODULE.bazel"),
            ),
            ("remote_module_file_integrity".into(), string(MODULE_SRI)),
            ("remote_patch_strip".into(), OverrideAttributeValue::Int(0)),
        ];
        RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: path("/fixture"),
                canonical_repo: CanonicalRepoName::new("protobuf+").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::new(SmallMap::from_iter(fields)),
            },
            kind: RepositoryMaterializationKind::Immutable,
        }
    }

    #[test]
    fn immutable_archive_guard_preserves_original_spec_and_exact_fields() {
        let bodies = configured(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        let original = request();
        let attrs = original.repo_spec.attributes.clone();
        assert!(matches(&bodies, &original));
        assert!(Arc::ptr_eq(&attrs, &original.repo_spec.attributes));
        assert!(
            bodies
                .admit(&SourcePreparationNeeds::repository(original.clone()))
                .is_ok()
        );
        for key in attrs.keys() {
            let mut wrong = original.clone();
            Arc::make_mut(&mut wrong.repo_spec.attributes)
                .insert(key.clone(), OverrideAttributeValue::Bool(false));
            assert!(!matches(&bodies, &wrong), "{key}");
            let mut missing = original.clone();
            Arc::make_mut(&mut missing.repo_spec.attributes).shift_remove(key);
            assert!(!matches(&bodies, &missing), "missing {key}");
        }
        for (key, value) in [
            (
                "urls",
                OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String(
                    "https://other.test/protobuf.tar.gz".into(),
                )])),
            ),
            (
                "integrity",
                OverrideAttributeValue::String(MODULE_SRI.into()),
            ),
            (
                "strip_prefix",
                OverrideAttributeValue::String("protobuf-33.4/".into()),
            ),
            (
                "remote_module_file_urls",
                OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String(
                    "file:////prepared/MODULE.bazel".into(),
                )])),
            ),
            (
                "remote_module_file_integrity",
                OverrideAttributeValue::String(PROTOBUF_SRI.into()),
            ),
            ("remote_patch_strip", OverrideAttributeValue::Int(1)),
        ] {
            let mut wrong = original.clone();
            Arc::make_mut(&mut wrong.repo_spec.attributes).insert(key.into(), value);
            assert!(!matches(&bodies, &wrong), "wrong typed {key}");
        }
        for key in [
            "remote_patches",
            "remote_file_urls",
            "remote_file_integrity",
        ] {
            let mut wrong = original.clone();
            Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
                key.into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    slug_bzlmod_v2::OverrideAttributeKey::String("unexpected".into()),
                    OverrideAttributeValue::None,
                )]))),
            );
            assert!(!matches(&bodies, &wrong), "nonempty {key}");
        }
        let mut mapping = slug_identity_v2::RepositoryMapping::new(
            slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
        );
        mapping.insert(
            slug_identity_v2::ApparentRepoName::new("bazel_tools").unwrap(),
            CanonicalRepoName::new("bazel_tools").unwrap(),
        );
        let mut mapped = original.clone();
        mapped.repo_spec.rule_id.bzl_file =
            slug_identity_v2::ApparentLabel::parse("@bazel_tools//tools/build_defs/repo:http.bzl")
                .unwrap()
                .resolve(&mapping);
        assert!(!matches(&bodies, &mapped));
        let mut missing_module =
            configured(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        missing_module.offline_archive_module = None;
        assert!(!matches(&missing_module, &original));
        missing_module.offline_archive_module = Some(path("/foreign/MODULE.bazel"));
        assert!(!matches(&missing_module, &original));
        let mut missing_backing =
            configured(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        missing_backing.offline_archive = None;
        assert!(!matches(&missing_backing, &original));
        missing_backing.offline_archive = Some(OfflineArchiveBacking::new(
            PROTOBUF_URL,
            [0; 32],
            path("/archive.tar.gz"),
        ));
        assert!(!matches(&missing_backing, &original));
        let mut extra = original.clone();
        Arc::make_mut(&mut extra.repo_spec.attributes).insert(
            "type".into(),
            OverrideAttributeValue::String("tar.gz".into()),
        );
        assert!(!matches(&bodies, &extra));
        let mut reordered = original.clone();
        reordered.repo_spec.attributes = Arc::new(SmallMap::from_iter(
            attrs
                .iter()
                .skip(1)
                .chain(attrs.iter().take(1))
                .map(|(k, v)| (k.clone(), v.clone())),
        ));
        assert!(matches(&bodies, &reordered));
        for label in [
            "@@other//tools/build_defs/repo:http.bzl",
            "@@bazel_tools//other:http.bzl",
            "@@bazel_tools//tools/build_defs/repo:Http.bzl",
        ] {
            let mut wrong = original.clone();
            wrong.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(label).unwrap();
            assert!(!matches(&bodies, &wrong));
        }
        let mut wrong = original.clone();
        wrong.id.workspace = path("/foreign");
        assert!(!matches(&bodies, &wrong));
        wrong = original.clone();
        wrong.id.canonical_repo = CanonicalRepoName::new("protobuf").unwrap();
        assert!(!matches(&bodies, &wrong));
        wrong = original.clone();
        wrong.repo_spec.rule_id.rule_name = "other".into();
        assert!(!matches(&bodies, &wrong));
        wrong = original.clone();
        wrong.kind = RepositoryMaterializationKind::Local {
            logical_root: path("/archive"),
        };
        assert!(!matches(&bodies, &wrong));
        assert!(Arc::ptr_eq(&attrs, &original.repo_spec.attributes));
    }

    #[test]
    fn immutable_archive_guard_preserves_roles_order_and_last_drop() {
        let original = request();
        for (registry, command) in [
            (RegistryChoice::DefaultBcr, CommandChoice::Modules),
            (RegistryChoice::DefaultBcr, CommandChoice::PublicProbe),
            (RegistryChoice::PreparedFile, CommandChoice::Modules),
        ] {
            let bodies = configured(registry, command);
            assert!(bodies.offline_archive.is_none());
            assert!(!matches(&bodies, &original));
            assert_eq!(
                bodies
                    .admit(&SourcePreparationNeeds::repository(original.clone()))
                    .unwrap_err()
                    .family,
                "materialization"
            );
        }
        let bodies = configured(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        let backing_weak = Arc::downgrade(bodies.offline_archive.as_ref().unwrap());
        let attrs_weak = Arc::downgrade(&original.repo_spec.attributes);
        for first in [true, false] {
            let mut other = original.clone();
            other.id.canonical_repo = CanonicalRepoName::new("foreign+").unwrap();
            let good = SourcePreparationNeeds::repository(original.clone());
            let bad = SourcePreparationNeeds::repository(other);
            let needs = if first {
                good.try_union(&bad)
            } else {
                bad.try_union(&good)
            }
            .unwrap();
            let (denied, reason) = bodies.first_denied_materialization(&needs).unwrap();
            assert_eq!(denied.id.canonical_repo.as_str(), "foreign+");
            assert_eq!(reason, "immutable_kind");
            let calls = std::cell::Cell::new(0);
            assert!(
                bodies
                    .admit_with_plan_matcher(&needs, &|_, _| {
                        calls.set(calls.get() + 1);
                        false
                    })
                    .is_err()
            );
            assert_eq!(calls.get(), 0);
        }
        let limits = bodies.source_limits(RegistryChoice::PreparedFile);
        let start = Instant::now();
        let scope = bodies
            .source_scope(
                bodies.roots[0].dupe(),
                start + Duration::from_secs(1),
                start + Duration::from_secs(2),
                limits,
            )
            .unwrap();
        let scope_weak = Arc::downgrade(&scope);
        drop(original);
        assert!(attrs_weak.upgrade().is_none());
        drop(bodies);
        assert!(backing_weak.upgrade().is_some());
        drop(scope);
        assert!(backing_weak.upgrade().is_none());
        assert!(scope_weak.upgrade().is_none());
        println!(
            "WP1080_OFFLINE_LAYOUT backing={} backing_align={} bodies={} bodies_align={}",
            std::mem::size_of::<OfflineArchiveBacking>(),
            std::mem::align_of::<OfflineArchiveBacking>(),
            std::mem::size_of::<Bodies>(),
            std::mem::align_of::<Bodies>()
        );
    }

    #[test]
    fn immutable_archive_native_byte_profile_is_exact_and_cumulative() {
        use CommandChoice::Modules;
        use CommandChoice::PublicProbe;
        use RegistryChoice::DefaultBcr;
        use RegistryChoice::PreparedFile;
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let archive = NormalizedAbsolutePath::new(
            project
                .join(ARCHIVE_MIRROR)
                .join(PROTOBUF_URL.strip_prefix("https://").unwrap()),
        )
        .unwrap();
        let module = NormalizedAbsolutePath::new(project.join(PREPARED_MODULE)).unwrap();
        let backing =
            |url: &'static str, digest, path| OfflineArchiveBacking::new(url, digest, path);
        let make = || {
            let mut bodies = configured(PreparedFile, PublicProbe);
            bodies.rules_rust_defs_import_sources = true;
            bodies.offline_archive = Some(backing(PROTOBUF_URL, PROTOBUF_DIGEST, archive.dupe()));
            bodies.offline_archive_module = Some(module.dupe());
            bodies
        };
        let native = |b: &Bodies| b.native_source_limits(PreparedFile, PublicProbe);
        let bodies = make();
        let owner = bodies.offline_archive.as_ref().unwrap();
        let owners = Arc::strong_count(owner);
        let weak = Arc::downgrade(owner);
        let expected = SourceServiceLimits {
            source_attempts: 256,
            decoded_bytes: PROTOBUF_NATIVE_BYTE_BUDGET,
            written_bytes: PROTOBUF_NATIVE_BYTE_BUDGET,
            ..bodies.source_limits(PreparedFile)
        };
        assert_eq!(native(&bodies), expected);
        assert_eq!(Arc::strong_count(owner), owners);
        assert!(std::ptr::eq(
            bodies.offline_archive_module.as_ref().unwrap().as_path(),
            module.as_path()
        ));
        let ordinary = bodies.source_limits(PreparedFile);
        assert_eq!(ordinary.decoded_bytes, 16 << 20);
        assert_eq!(ordinary.written_bytes, 16 << 20);
        for case in 0..8 {
            let mut b = make();
            match case {
                0 => b.offline_archive = None,
                1 => b.offline_archive_module = None,
                2 => {
                    b.offline_archive = Some(backing(
                        "https://other.test/archive.tar.gz",
                        PROTOBUF_DIGEST,
                        archive.dupe(),
                    ))
                }
                3 => b.offline_archive = Some(backing(PROTOBUF_URL, [0; 32], archive.dupe())),
                4 => {
                    b.offline_archive = Some(backing(
                        PROTOBUF_URL,
                        PROTOBUF_DIGEST,
                        path("/foreign/archive.tar.gz"),
                    ))
                }
                5 => b.offline_archive_module = Some(path("/foreign/MODULE.bazel")),
                6 => {
                    b = configured(PreparedFile, PublicProbe);
                    b.rules_rust_defs_import_sources = true;
                }
                _ => b.rules_rust_defs_import_sources = false,
            }
            assert_eq!(
                native(&b),
                SourceServiceLimits {
                    source_attempts: if b.rules_rust_defs_import_sources {
                        256
                    } else {
                        16
                    },
                    ..b.source_limits(PreparedFile)
                },
                "case {case}"
            );
        }
        for (r, c) in [
            (DefaultBcr, Modules),
            (DefaultBcr, PublicProbe),
            (PreparedFile, Modules),
        ] {
            assert_eq!(bodies.native_source_limits(r, c), bodies.source_limits(r));
        }
        for counter in [SourceCounter::DecodedBytes, SourceCounter::WrittenBytes] {
            let start = Instant::now();
            let scope = bodies
                .source_scope(
                    bodies.roots[0].dupe(),
                    start + Duration::from_secs(2),
                    start + Duration::from_secs(3),
                    native(&bodies),
                )
                .unwrap();
            let weak_scope = Arc::downgrade(&scope);
            assert_eq!(scope.limits(), expected);
            scope
                .charge(counter, PROTOBUF_NATIVE_BYTE_BUDGET / 2)
                .unwrap();
            scope
                .charge(counter, PROTOBUF_NATIVE_BYTE_BUDGET / 2)
                .unwrap();
            let refusal = Err(SourceServiceError::Limit(counter));
            assert_eq!(scope.charge(counter, 1), refusal);
            assert_eq!(
                scope.first_error_for_test(),
                Some(SourceServiceError::Limit(counter))
            );
            assert_eq!(scope.charge(SourceCounter::SourceAttempts, 1), refusal);
            let counters = scope.snapshot();
            assert_eq!(
                counters.decoded_bytes,
                if counter == SourceCounter::DecodedBytes {
                    PROTOBUF_NATIVE_BYTE_BUDGET
                } else {
                    0
                }
            );
            assert_eq!(
                counters.written_bytes,
                if counter == SourceCounter::WrittenBytes {
                    PROTOBUF_NATIVE_BYTE_BUDGET
                } else {
                    0
                }
            );
            assert_eq!(counters.source_attempts, 0);
            assert_eq!(counters.live_buffer_bytes, 0);
            drop(scope);
            assert!(weak_scope.upgrade().is_none());
        }
        assert_eq!(Arc::strong_count(owner), owners);
        drop(bodies);
        assert!(weak.upgrade().is_none());
    }
}
