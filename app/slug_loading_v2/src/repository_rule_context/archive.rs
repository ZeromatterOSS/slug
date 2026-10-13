//! Caller-neutral argument validation and replay for repository archive calls.

use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use slug_bzlmod_v2::generated_repository_archive_parse_sha256;

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn parse_archive_effect<'v>(
    url: Value<'v>,
    output: &str,
    sha256: &str,
    archive_type: &str,
    strip_prefix: &str,
    old_strip_prefix: &str,
    allow_fail: bool,
    canonical_id: &str,
    auth: Option<Value<'v>>,
    headers: Option<Value<'v>>,
    integrity: &str,
    rename_files: Option<Value<'v>>,
    strip_components: i32,
    heap: Heap<'v>,
) -> Result<GeneratedRepositoryArchiveEffect, RepositoryRuleInvocationError> {
    if !matches!(output, "" | ".") || allow_fail || strip_components != 0 {
        return Err(RepositoryRuleInvocationError::ArchiveOptions);
    }
    for (option, nonempty_error) in [
        (auth, RepositoryRuleInvocationError::ArchiveAuth),
        (headers, RepositoryRuleInvocationError::ArchiveOptions),
        (rename_files, RepositoryRuleInvocationError::ArchiveOptions),
    ] {
        if let Some(option) = option {
            let dict = DictRef::from_value(option)
                .ok_or(RepositoryRuleInvocationError::ArchiveArgument)?;
            if !dict.is_empty() {
                return Err(nonempty_error);
            }
        }
    }
    if !strip_prefix.is_empty() && !old_strip_prefix.is_empty() {
        return Err(RepositoryRuleInvocationError::ArchiveArgument);
    }
    let prefix = if strip_prefix.is_empty() {
        old_strip_prefix
    } else {
        strip_prefix
    };
    let digest = generated_repository_archive_parse_sha256(sha256, integrity)
        .map_err(RepositoryRuleInvocationError::Plan)?;
    let mut urls = Vec::new();
    if let Some(url) = url.unpack_str() {
        urls.push(CompactString::new(url));
    } else {
        for value in url
            .iterate(heap)
            .map_err(|_| RepositoryRuleInvocationError::ArchiveArgument)?
        {
            if urls.len() == 8 {
                return Err(RepositoryRuleInvocationError::FilesystemLimit);
            }
            urls.push(CompactString::new(
                value
                    .unpack_str()
                    .ok_or(RepositoryRuleInvocationError::ArchiveArgument)?,
            ));
        }
    }
    let first = urls
        .first()
        .ok_or(RepositoryRuleInvocationError::ArchiveArgument)?;
    let format = match archive_type {
        "tar.xz" | "txz" => GeneratedRepositoryArchiveFormat::TarXz,
        "tar.gz" | "tgz" => GeneratedRepositoryArchiveFormat::TarGz,
        "" => {
            let first_path = first
                .split(['?', '#'])
                .next()
                .expect("URL text has a first component");
            if first_path.ends_with(".tar.xz") || first_path.ends_with(".txz") {
                GeneratedRepositoryArchiveFormat::TarXz
            } else if first_path.ends_with(".tar.gz") || first_path.ends_with(".tgz") {
                GeneratedRepositoryArchiveFormat::TarGz
            } else {
                return Err(RepositoryRuleInvocationError::ArchiveOptions);
            }
        }
        _ => return Err(RepositoryRuleInvocationError::ArchiveOptions),
    };
    GeneratedRepositoryArchiveEffect::new(
        urls.into(),
        digest,
        prefix.into(),
        format,
        canonical_id.into(),
        integrity.into(),
    )
    .map_err(RepositoryRuleInvocationError::Plan)
}

pub(super) fn replay_archive<'v>(
    state: &RepositoryRuleInvocationState,
    effect: GeneratedRepositoryArchiveEffect,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let result =
        filesystem::replay_operation(state, RepositoryFilesystemOperation::Archive(effect))?;
    state
        .generated_present
        .lock()
        .expect("generated path state mutex poisoned")
        .archive_seen = true;
    Ok(heap.alloc(AllocStruct([
        ("success", heap.alloc(true)),
        (
            "sha256",
            heap.alloc(result.sha256_hex().expect("validated archive result")),
        ),
        (
            "integrity",
            heap.alloc(result.sha256_sri().expect("validated archive result")),
        ),
    ])))
}

#[cfg(test)]
mod tests {
    use starlark::values::tuple::AllocTuple;

    use super::*;

    fn parse<'v>(
        url: Value<'v>,
        heap: Heap<'v>,
        format: &str,
        prefix: &str,
        old_prefix: &str,
    ) -> Result<GeneratedRepositoryArchiveEffect, RepositoryRuleInvocationError> {
        parse_archive_effect(
            url,
            "",
            &"a1".repeat(32),
            format,
            prefix,
            old_prefix,
            false,
            "",
            None,
            None,
            "",
            None,
            0,
            heap,
        )
    }

    #[test]
    fn archive_arguments_accept_independent_mirrors_prefixes_and_explicit_type() {
        let module = Module::new();
        let heap = module.heap();
        let urls = heap.alloc(AllocTuple([
            "https://independent.test/a.tar.xz",
            "file:///tmp/different-name.payload",
        ]));
        let effect = parse(urls, heap, "", "arbitrary/subtree", "").unwrap();
        assert_eq!(effect.urls().len(), 2);
        assert_eq!(effect.strip_prefix(), "arbitrary/subtree");
        assert_eq!(effect.format(), GeneratedRepositoryArchiveFormat::TarXz);
        assert_eq!(effect.sha256(), [0xa1; 32]);
        let crate_url = heap.alloc("https://independent.test/tinyjson.crate?download=1");
        assert_eq!(
            parse(crate_url, heap, "tar.gz", "", "").unwrap().format(),
            GeneratedRepositoryArchiveFormat::TarGz
        );
        assert!(parse(crate_url, heap, "", "", "").is_err());
        assert!(parse(urls, heap, "", "same", "same").is_err());
        assert!(parse(urls, heap, "", "../outside", "").is_err());
        assert!(
            parse(
                heap.alloc(AllocList(std::iter::empty::<&str>())),
                heap,
                "",
                "",
                ""
            )
            .is_err()
        );
    }

    #[test]
    fn archive_observation_refusal_survives_swallowed_attribute_lookup() {
        let module = Module::new();
        let facts = Arc::new(Mutex::new(GeneratedPathFacts {
            present: SmallSet::from_iter(["".into(), "authored".into()]),
            archive_seen: true,
            refused: false,
        }));
        let path = RepositoryStarlarkPath::Generated {
            repository: "independent".into(),
            relative: "unknown".into(),
            present: facts.clone(),
        };
        assert!(path.get_attr("exists", module.heap()).is_none());
        assert!(facts.lock().unwrap().refused);
        let known = RepositoryStarlarkPath::Generated {
            repository: "independent".into(),
            relative: "authored".into(),
            present: facts,
        };
        assert_eq!(
            known
                .get_attr("exists", module.heap())
                .unwrap()
                .unpack_bool(),
            Some(true)
        );
    }
}
