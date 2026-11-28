//! Diagnostic identity for the BUILD evaluator's final local print batch.

use super::event_origin::EventOriginMemo;
use super::event_origin::EventOriginWriter;
use super::*;

pub(super) type PackageEvents = Option<LoadingEventBatch>;

pub(super) fn capture(
    input: &HostPackageAttemptInput<'_>,
    mapping: &RunfilesRepositoryMapping,
    requests: &SmallMap<HostGlobLoadingRequest, HostGlobPrepared>,
    batch: &EventBatch,
    input_error: Option<(&HostGlobLoadingRequest, &HostGlobRequestInputError)>,
) -> PackageEvents {
    if batch.events().is_empty() {
        return None;
    }

    let mut writer = EventOriginWriter::new(b"slug-diagnostic-build-inputs-v1");
    writer.text(input.package_identifier.repo().as_str());
    writer.text(input.package_identifier.package().as_str());
    writer.path(&input.build_file);
    writer.bytes(&input.source);
    writer.count(mapping.entries().len());
    for (apparent, canonical) in mapping.entries() {
        writer.text(apparent.as_str());
        writer.text(canonical.as_str());
    }
    writer.count(input.repository_features.len());
    for feature in &*input.repository_features {
        writer.text(feature);
    }
    writer.optional(
        input.associated_module_metadata.as_ref(),
        |writer, (name, version)| {
            writer.text(name);
            writer.text(version);
        },
    );

    let mut memo = EventOriginMemo::default();
    writer.count(input.loaded_modules.len());
    for (raw_load, module) in input.loaded_modules {
        writer.text(raw_load);
        let actual = module
            .semantic_input_facts()
            .expect("Host BUILD direct loads have prepared semantic inputs");
        writer.append_stamp(&actual.event_stamp(&mut memo));
    }

    writer.count(requests.len());
    for (request, result) in requests {
        request.write_event_origin(&mut writer);
        match result.as_ref() {
            Ok(matches) => {
                writer.tag(0);
                writer.count(matches.paths().len());
                for path in matches.paths() {
                    writer.bytes(path);
                }
            }
            // Traversal errors terminate the evaluator at this request. The
            // detailed diagnostic remains in the semantic error owner.
            Err(_) => writer.tag(1),
        }
    }
    writer.optional(input_error, |writer, (request, error)| {
        request.write_event_origin(writer);
        error.write_event_origin(writer);
    });

    Some(LoadingEventBatch {
        batch: batch.dupe(),
        origin: LoadingEventOrigin::Package(writer.finish()),
    })
}
