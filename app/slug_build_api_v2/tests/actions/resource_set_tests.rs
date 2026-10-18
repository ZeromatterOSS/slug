//! General native descriptor dispatch and cross-field publication identity.
use std::sync::atomic::Ordering;

use slug_build_api_v2::ArgsDirectoryContext;
use slug_build_api_v2::ArgsExpansionError;
use slug_build_api_v2::PublishedValueRef;
use slug_build_api_v2::RetainedArgsInputs;
use slug_build_api_v2::RetainedArgsMapper;
use slug_build_api_v2::RuntimeMapper;
use slug_build_api_v2::actions::ResourceSetInputs;
use slug_build_api_v2::actions::ResourceSetOs;

use super::*;
#[derive(Debug)]
struct EmptyMapper;
impl RuntimeMapper for EmptyMapper {
    fn map_batch(
        &self,
        values: &[AnalysisValue],
        _: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<compact_str::CompactString>, ArgsExpansionError> {
        assert!(values.is_empty());
        Ok(Vec::new())
    }
}
fn make(
    resource: slug_build_api_v2::actions::RetainedSpawnResourceSet,
    mapper: PublishedValueRef,
) -> SpawnSpec {
    let mapper = RetainedArgsMapper::from_published(mapper, Arc::new(EmptyMapper)).unwrap();
    let mut spawn = param_spawn(vec![], &["out"]);
    let args = RetainedVectorArg::new(
        RetainedVectorSource::Mapped {
            inputs: RetainedArgsInputs::Sequence(Arc::from([])),
            mapper,
            registration_location: None,
        },
        default_vector_options(),
    );
    // Retain the mapped callable despite empty output. The empty batch mapper
    // sees no elements; rendering must never invoke the resource runtime.
    spawn = SpawnSpec::new(
        spawn.invocation().clone(),
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
            RetainedSpawnArgsSnapshot::new(
                RetainedArgsRecipe::new(
                    vec![RetainedArgCall::AddAll(args)],
                    RetainedParamFileFormat::Multiline,
                ),
                None,
            ),
        )]),
        spawn.inputs().clone(),
        spawn.tools().clone(),
        spawn.outputs().to_vec(),
        None,
        spawn.environment().clone(),
        spawn.execution_requirements().clone(),
        spawn.mnemonic(),
        spawn.progress_message(),
    );
    spawn.with_resource_set(resource)
}
#[test]
fn resource_publication_shares_one_alias_state_with_empty_args_and_runtime_is_nonsemantic() {
    let (a, ca) = resource_support::descriptor("unrelated.shared");
    let (b, cb) = resource_support::descriptor("unrelated.shared");
    let (foreign, cf) = resource_support::descriptor("unrelated.shared");
    assert_eq!(a, b);
    assert!(!a.published_callable().same_original(b.published_callable()));
    let aliased = make(a.clone(), a.published_callable().clone());
    let independently_aliased = make(b.clone(), b.published_callable().clone());
    let split = make(b, foreign.published_callable().clone());
    assert_eq!(aliased, independently_aliased);
    assert_ne!(aliased, split);
    assert_eq!(aliased.render_argv().unwrap(), split.render_argv().unwrap());
    assert_eq!(
        (
            ca.load(Ordering::SeqCst),
            cb.load(Ordering::SeqCst),
            cf.load(Ordering::SeqCst)
        ),
        (0, 0, 0)
    );
}
#[test]
fn resource_explicit_runtime_is_uncached_and_descriptor_clone_keeps_original_root() {
    let (a, calls) = resource_support::descriptor("unrelated.failure");
    let clone = a.clone();
    assert!(
        a.published_callable()
            .same_original(clone.published_callable())
    );
    let context = resource_support::Context::default();
    for expected in [1, 2] {
        let error = clone
            .evaluate(
                ResourceSetInputs {
                    os: ResourceSetOs::Linux,
                    parent_input_count: 2,
                },
                &context,
            )
            .unwrap_err();
        assert!(error.to_string().contains("typed runtime would fail"));
        assert_eq!(calls.load(Ordering::SeqCst), expected);
    }
    drop(a);
    assert!(clone.published_callable().original_slot().is_some());
}
