use std::hash::Hash;
use std::hash::Hasher;

use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::ActionKind;
use crate::ActionRegistry;
use crate::ActionSpec;
use crate::CtxActions;
use crate::ReapiCommandProjection;
use crate::WorkspaceStatusArtifacts;
use crate::artifact_root::test_support as roots;

// Typed descriptor/header proof only. Analysis owns real Starlark rendering.
#[derive(Debug)]
struct Originals;
impl crate::published_value::PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        1
    }
    fn imported_arenas(&self) -> &[crate::PublishedValueArena] {
        &[]
    }
}
#[derive(Debug)]
struct NeverRender;
impl BuildInfoTransformRuntime for NeverRender {
    fn render(
        &self,
        _: &[u8],
        _: &[u8],
        _: &dyn BuildInfoEvaluationContext,
    ) -> Result<Vec<u8>, BuildInfoTransformError> {
        panic!("metadata must never invoke BuildInfo callback")
    }
}
fn original(slot: bool) -> Result<crate::PublishedValueRef, crate::PublishedValueError> {
    use crate::published_value::*;
    let mut builder = PublishedValueArenaBuilder::new();
    let node = builder.reserve().unwrap();
    builder
        .set_node(
            node,
            PublishedNode::Intrinsic(PublishedIntrinsic {
                api: crate::NativeCallableApiId::new("unrelated.build_info", 1).unwrap(),
                parameters: Arc::from([]),
                receiver: None,
                return_annotation: None,
            }),
        )
        .unwrap();
    if slot {
        builder
            .set_original_slot(node, PublishedRootSlot::new(0).unwrap())
            .unwrap();
    }
    builder.seal(Arc::new(Originals))?.value(node)
}
fn descriptor() -> RetainedBuildInfoTransform {
    RetainedBuildInfoTransform::from_published(original(true).unwrap(), Arc::new(NeverRender))
        .unwrap()
}
fn checked_spec(
    status: crate::WorkspaceStatusArtifact,
    template: AnalysisArtifact,
    output: ActionOutput,
    kind: BuildInfoTransformKind,
) -> Result<BuildInfoTransformSpec, String> {
    Ok(BuildInfoTransformSpec::new(
        BuildInfoTransformHeader::new(kind, status, template, output)?,
        descriptor(),
    ))
}
fn template(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@bazel_tools//templates:{name}")).unwrap(),
    )
}
fn spec(
    kind: BuildInfoTransformKind,
    workspace: &str,
    name: &str,
    root: crate::DerivedArtifactRoot,
    output: &str,
) -> BuildInfoTransformSpec {
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new(workspace).unwrap());
    let (status, output) = match kind {
        BuildInfoTransformKind::Info => (
            status.stable(),
            ActionOutput::new(root, output, ActionOutputKind::File),
        ),
        BuildInfoTransformKind::Version => {
            (status.volatile(), ActionOutput::new_constant(root, output))
        }
    };
    checked_spec(status.dupe(), template(name), output, kind).unwrap()
}

#[test]
fn build_info_specs_preserve_all_owned_inputs_and_metadata() {
    let a = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "volatile",
        roots::root(),
        "pkg/v.h",
    );
    assert_eq!(
        a.inputs(),
        &[
            AnalysisArtifact::WorkspaceStatus(a.status().dupe()),
            template("volatile")
        ]
    );
    assert!(a.is_volatile() && a.execute_unconditionally());
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (template, count) in [(status.volatile(), 1), (status.stable(), 2)] {
        let input = AnalysisArtifact::WorkspaceStatus(template.dupe());
        let same_role = checked_spec(
            status.volatile().dupe(),
            input.clone(),
            a.output().dupe(),
            BuildInfoTransformKind::Version,
        )
        .unwrap();
        assert_eq!(same_role.inputs().len(), count);
        assert_eq!(
            same_role.template(),
            &input,
            "role retention is independent of input deduplication"
        );
        assert_eq!(
            same_role.inputs()[0],
            AnalysisArtifact::WorkspaceStatus(status.volatile().dupe())
        );
    }
    assert_eq!(a.output().metadata(), ActionOutputMetadata::Constant);
    let action = ActionSpec::build_info_transform(a.dupe());
    assert_eq!(action.kind(), &ActionKind::BuildInfoTransform);
    assert_eq!(action.mnemonic(), "TranslateBuildInfo");
    assert_eq!(action.outputs(), &[a.output().dupe()]);
    assert_eq!(
        action.progress_message(),
        Some("Translating volatile BuildInfo file")
    );
    assert!(action.render_argv().is_err());
    assert!(ReapiCommandProjection::from_action(&action).is_err());
    for other in [
        spec(
            BuildInfoTransformKind::Info,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/other",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "different",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root_for(b"B"),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/other.h",
        ),
    ] {
        assert_ne!(a, other);
    }
    assert_eq!(
        a,
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h"
        )
    );
    let callback = descriptor();
    let same = callback.clone();
    assert!(
        callback
            .published_callable()
            .same_original(same.published_callable())
    );
    // The genuine builder refuses the missing binding at seal, before a
    // descriptor can exist. Never fabricate an invalid sealed arena.
    assert!(matches!(
        original(false),
        Err(crate::PublishedValueError::Invalid("missing original slot"))
    ));
}

#[test]
fn constant_metadata_is_orthogonal_and_cannot_enter_other_action_families() {
    let ordinary = ActionOutput::new(roots::root(), "out", ActionOutputKind::File);
    let constant = ActionOutput::new_constant(roots::root(), "out");
    assert_eq!(ordinary.path(), constant.path());
    assert_eq!(ordinary.kind(), constant.kind());
    assert_ne!(ordinary, constant);
    assert_ne!(ordinary.cmp(&constant), std::cmp::Ordering::Equal);
    let hash = |output: &ActionOutput| {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        output.hash(&mut hash);
        hash.finish()
    };
    assert_ne!(hash(&ordinary), hash(&constant));
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (status, output, kind) in [
        (
            status.volatile(),
            ordinary.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "dir", ActionOutputKind::Directory),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "../escape", ActionOutputKind::File),
            BuildInfoTransformKind::Info,
        ),
    ] {
        assert!(checked_spec(status.dupe(), template("t"), output, kind).is_err());
    }
    for action in [
        ActionSpec::new(
            ActionKind::BuildInfoTransform,
            "TranslateBuildInfo",
            vec![ordinary],
        ),
        ActionSpec::new(
            ActionKind::Write {
                content: "fake".into(),
                is_executable: false,
            },
            "FileWrite",
            vec![constant],
        ),
    ] {
        assert!(ActionRegistry::new().register(action.clone()).is_err());
        assert!(ReapiCommandProjection::from_action(&action).is_err());
    }
}

#[test]
fn build_info_registration_is_ordered_atomic_and_conflicts_across_metadata() {
    let mut actions = CtxActions::new(roots::root());
    let volatile = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "v",
        roots::root(),
        "pkg/v.h",
    );
    let stable = spec(
        BuildInfoTransformKind::Info,
        "/workspace",
        "i",
        roots::root(),
        "pkg/i.h",
    );
    assert_eq!(
        actions
            .register_build_info_transform(volatile.dupe())
            .unwrap(),
        0
    );
    assert_eq!(
        actions
            .register_build_info_transform(stable.dupe())
            .unwrap(),
        1
    );
    let before = actions.clone();
    assert!(
        actions
            .write(
                ActionOutput::new(roots::root(), "pkg/v.h", ActionOutputKind::File),
                "collision",
                false
            )
            .is_err()
    );
    assert_eq!(before, actions);
    assert!(actions.register_build_info_transform(volatile).is_err());
    assert_eq!(before, actions);
    let mut registry = ActionRegistry::new();
    let stable = ActionSpec::build_info_transform(stable);
    assert!(registry.register_batch([stable.clone(), stable]).is_err());
    assert!(registry.actions().is_empty());
    let malformed = ActionSpec::new(
        ActionKind::Write {
            content: "x".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new_constant(roots::root(), "bad")],
    );
    assert!(actions.replace_actions(vec![malformed]).is_err());
    assert_eq!(before, actions);
}

#[test]
fn build_info_descriptor_dispatch_lifetime_and_shared_alias_state_are_explicit() {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    #[derive(Debug)]
    struct Dispatch(Arc<AtomicUsize>);
    impl BuildInfoTransformRuntime for Dispatch {
        fn render(
            &self,
            status: &[u8],
            template: &[u8],
            context: &dyn BuildInfoEvaluationContext,
        ) -> Result<Vec<u8>, BuildInfoTransformError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            assert_eq!(status, b"actual status");
            assert_eq!(context.mnemonic(), "ConsumerMnemonic");
            assert!(!context.is_cancelled());
            context.print("actual current sink")?;
            Ok(template.to_vec())
        }
    }
    struct Consumer(std::cell::RefCell<Vec<String>>);
    impl BuildInfoEvaluationContext for Consumer {
        fn is_cancelled(&self) -> bool {
            false
        }
        fn print(&self, text: &str) -> Result<(), BuildInfoTransformError> {
            self.0.borrow_mut().push(text.to_owned());
            Ok(())
        }
        fn mnemonic(&self) -> &str {
            "ConsumerMnemonic"
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let backend = Arc::new(Dispatch(calls.clone()));
    let weak = Arc::downgrade(&backend);
    let a = RetainedBuildInfoTransform::from_published(original(true).unwrap(), backend.clone())
        .unwrap();
    let b =
        RetainedBuildInfoTransform::from_published(original(true).unwrap(), Arc::new(NeverRender))
            .unwrap();
    let foreign =
        RetainedBuildInfoTransform::from_published(original(true).unwrap(), Arc::new(NeverRender))
            .unwrap();
    assert_eq!(a, b);
    assert!(!a.published_callable().same_original(b.published_callable()));
    let mut state = crate::analysis_value::PublicationEqState::default();
    assert!(a.publication_eq_with(&b, &mut state));
    assert!(
        a.published_callable()
            .publication_eq_with(b.published_callable(), &mut state)
    );
    assert!(
        !a.published_callable()
            .publication_eq_with(foreign.published_callable(), &mut state)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "metadata and Eq never invoke"
    );
    let copy = a.clone();
    drop(backend);
    drop(a);
    assert!(weak.upgrade().is_some());
    let context = Consumer(std::cell::RefCell::new(Vec::new()));
    for expected in [1, 2] {
        assert_eq!(
            copy.render(b"actual status", b"actual template", &context)
                .unwrap(),
            b"actual template"
        );
        assert_eq!(calls.load(Ordering::SeqCst), expected);
    }
    assert_eq!(
        *context.0.borrow(),
        vec!["actual current sink", "actual current sink"]
    );
    drop(copy);
    assert!(weak.upgrade().is_none());
}
