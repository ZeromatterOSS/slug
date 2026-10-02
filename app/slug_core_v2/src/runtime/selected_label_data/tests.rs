//! Genuine normal lower plans and whole-batch exact-file admission.
use std::fs;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use selected_tool_source::tests::Fixture;
use selected_tool_source::tests::discover_owner;
use selected_tool_source::tests::literal;
use selected_tool_source::tests::wire;
use slug_bzlmod_v2::GuestLabelDataPolicy;
use slug_bzlmod_v2::GuestNamespaceLimits;
use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::RuntimeSourceDeclarationInput;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_bzlmod_v2::RuntimeSourceSelectorInput;
use slug_bzlmod_v2::SelectedScratchElfMetadataNeeds;
use slug_bzlmod_v2::SelectedScratchPolicy;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_loading_v2::HostSelectedExtensionScratchPrefixKey;
use slug_loading_v2::SelectedGuestNamespaceFactsKey;
use slug_workspace_v2::DirectoryObservationLimits;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::ImageObservationRequest;
use slug_workspace_v2::NeedPathObservations;

use super::*;

pub(in crate::runtime::dice) fn enabled(max: u8) -> GuestNamespacePolicy {
    GuestNamespacePolicy::new(
        1001,
        1002,
        1001,
        1002,
        9,
        7,
        8,
        0o755,
        GuestNamespaceLimits::default(),
    )
    .unwrap()
    .with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: max })
    .unwrap()
}
pub(in crate::runtime::dice) fn fixture(
    root_label: &str,
) -> (
    Fixture,
    tempfile::TempDir,
    slug_bzlmod_v2::RuntimeSourceDeclarationPolicy,
    SelectedGuestNamespaceFactsKey,
) {
    let source = format!(
        r#"
def _repo(ctx):
    ctx.file('REPO.bazel','')
    ctx.file('BUILD.bazel',"exports_files(['data'])\n")
    ctx.file('data','generated-data',executable=False)
    ctx.file('unrelated','not selected',executable=False)
assets_repo=repository_rule(implementation=_repo)
def _producer(ctx):
    assets_repo(name='assets')
producer=module_extension(implementation=_producer)
cfg=tag_class(attrs={{'root':attr.label(),'repo':attr.label()}})
def _consumer(ctx):
    for module in ctx.modules:
        for tag in module.tags.cfg:
            root=ctx.path(tag.root)
            repo=ctx.path(tag.repo)
            ctx.file('config.json',str(root)+'\n'+str(repo),executable=False)
            generator=ctx.path('generator')
            ctx.file(generator,{},executable=True)
            ctx.execute([generator,'query',ctx.path('config.json')])
    fail('no root cfg')
ext=module_extension(implementation=_consumer,tag_classes={{'cfg':cfg}})
"#,
        literal(&wire(1))
    );
    let mut f = Fixture::new(&source);
    fs::write(f.workspace.path().join("MODULE.bazel"),format!("module(name='bazel_tools')\np=use_extension('//:ext.bzl','producer')\nuse_repo(p,assets='assets')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e,proof='proof')\ne.cfg(root='{root_label}',repo='@assets//:data')\nbazel_dep(name='platforms',version='1.0.0')\nlocal_path_override(module_name='platforms',path='platforms')\n")).unwrap();
    fs::write(
        f.workspace.path().join("BUILD.bazel"),
        "exports_files(['marker','data'])\n",
    )
    .unwrap();
    fs::write(f.workspace.path().join("data"), "root-A").unwrap();
    f.key = HostSelectedExtensionScratchPrefixKey::new(
        f.runtime.repository_materializer.workspace().dupe(),
        discover_owner(&f.runtime),
        SelectedScratchPolicy::plain(),
    );
    let host = tempfile::tempdir().unwrap();
    fs::write(host.path().join("asset"), "host").unwrap();
    let b = f.runtime.image_resources.descriptor_budget();
    let workspace =
        ImageChargedPath::pin_existing(f.runtime.repository_materializer.workspace().dupe(), b)
            .unwrap();
    let p = RuntimeSourceDeclarationPolicy::new(
        &workspace,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: host.path().to_str().unwrap(),
            },
            guest_prefix: "/external",
        }],
        RuntimeSourceLimits::default(),
        b,
    )
    .unwrap();
    let key = SelectedGuestNamespaceFactsKey::new(RuntimeSourceProfileKey::new(
        selected_tool_source::tests::key(&f).requests_key().clone(),
    ));
    (f, host, p, key)
}

// No completed facts are made here: drive RuntimeRequests, then install the
// real plan's selector-only transaction and compute normal DataRequests anew.
fn current_data<'a>(
    f: &'a Fixture,
    p: &RuntimeSourceDeclarationPolicy,
    key: &RuntimeSourceProfileKey,
    deadline: Instant,
) -> (
    NativeDemandAbortGuard<'a>,
    ObservedSelectedDataLabelRequests,
    SelectedWorkspaceDemands,
) {
    let (mut guard, runtime_plan, _) = runtime_sources::tests::current_plan(f, p, key);
    let mut inputs = guard.command().inputs.clone();
    inputs.request.guest_namespace = Some(enabled(2));
    guard.command.as_mut().unwrap().inputs = inputs;
    guard.command.as_mut().unwrap().scratch_metadata = Some(
        selected_tool_source::ScratchMetadataState::new(
            runtime_plan.tool_requests(),
            &f.runtime.scratch_resources,
        )
        .unwrap(),
    );
    for _ in 0..128 {
        guard.begin_attempt().unwrap();
        let result = f
            .runtime
            .runtime
            .block_on(super::plan(&f.runtime, &mut guard, key, deadline))
            .unwrap();
        f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
        match result {
            selected_tool_source::attempt::Attempt::Need(n) => {
                selected_tool_source::progress_baseline(&mut guard, &n, deadline).unwrap()
            }
            selected_tool_source::attempt::Attempt::Complete(plan, selected) => {
                assert_eq!(plan.runtime_requests(), &runtime_plan);
                let (_, reachable, terminal) = selected.sidecars.into_parts();
                terminal.reset_to_idle().unwrap();
                validate_requests(guard.command(), key, &plan, &reachable, deadline).unwrap();
                return (guard, plan, reachable);
            }
        }
    }
    panic!("genuine data plan must progress")
}
struct Restore {
    original: PathBuf,
    hidden: PathBuf,
    armed: bool,
}
impl Restore {
    fn new(path: &Path) -> Self {
        let hidden = path.with_extension("data-hidden");
        fs::rename(path, &hidden).unwrap();
        Self {
            original: path.to_owned(),
            hidden,
            armed: true,
        }
    }
    fn restore(mut self) {
        fs::rename(&self.hidden, &self.original).unwrap();
        self.armed = false
    }
}
impl Drop for Restore {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::rename(&self.hidden, &self.original);
        }
    }
}

#[test]
fn data_mixed_need_invalid_second_denies_before_proof_or_source_io_and_repairs() {
    use crate::runtime::input_image::elf_metadata;
    let (f, _host, p, key) = fixture("//:data");
    let _storage = prepare::Storage::new(&f.runtime, 2).unwrap();
    let _runtime = runtime_sources::prepare::Storage::new(&f.runtime).unwrap();
    let _scratch = selected_scratch_source::PreparedStage::new(&f.runtime).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let (mut guard, plan, reachable) = current_data(&f, &p, key.profile_key(), deadline);
    let r = &f.runtime.scratch_resources;
    let scratch = plan
        .runtime_requests()
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap();
    let first = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(scratch, r).unwrap(),
    );
    let leaf = plan.requests().next().unwrap().leaf_image_key();
    let anchor =
        ImageChargedPath::pin_existing(leaf.anchor_root().dupe(), r.descriptor_budget()).unwrap();
    let path =
        ImageChargedPath::pin_existing(leaf.source_root().dupe(), r.descriptor_budget()).unwrap();
    let valid = PathObservationDemand::input_image(
        leaf.namespace(),
        path.dupe(),
        ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat),
    )
    .unwrap();
    let tools = selected_tool_source::declarations::Catalog::new(
        &f.runtime,
        plan.runtime_requests().tool_requests(),
        deadline,
    )
    .unwrap();
    let runtime = runtime_sources::catalogue::Catalogue::new(
        &f.runtime,
        key.profile_key(),
        plan.runtime_requests(),
        &reachable,
        deadline,
    )
    .unwrap();
    let data =
        catalogue::Catalogue::new(&f.runtime, key.profile_key(), &plan, &reachable, deadline)
            .unwrap();
    for op in [
        ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
            max_entries: 10,
            max_name_bytes_total: 1024,
            max_component_bytes: 255,
        }),
        ImageObservationOperation::BoundedReadLink {
            max_target_bytes: 255,
        },
    ] {
        let bad = PathObservationDemand::input_image(
            leaf.namespace(),
            path.dupe(),
            ImageObservationRequest::new(anchor.dupe(), op),
        )
        .unwrap();
        let rows = SourcePreparationNeeds::path(NeedPathObservations::singleton(valid.clone()))
            .try_union(&SourcePreparationNeeds::path(
                NeedPathObservations::singleton(bad),
            ))
            .unwrap();
        let need = first.try_union(&rows).unwrap();
        elf_metadata::test_reset_borrowed_read_count();
        input_image::test_reset_anchor_open_count();
        let hidden = Restore::new(f.workspace.path());
        let error = runtime_sources::prepare::progress_sources_with_data(
            &mut guard,
            &need,
            key.profile_key(),
            plan.runtime_requests(),
            &reachable,
            &tools,
            &runtime,
            Some(&data),
            deadline,
        )
        .unwrap_err();
        hidden.restore();
        assert!(error.to_string().contains("undeclared image observation"));
        assert_eq!(elf_metadata::test_borrowed_read_count(), 0);
        assert_eq!(input_image::test_anchor_open_count(), 0);
        assert!(
            guard
                .command()
                .scratch_metadata
                .as_ref()
                .unwrap()
                .transaction()
                .unwrap()
                .get(scratch)
                .is_none()
        );
    }
    guard.discard().unwrap();
    f.restored();
    // Rename alters genuine ancestor facts. Recompute rather than patch them.
    let (mut guard, plan, reachable) = current_data(&f, &p, key.profile_key(), deadline);
    let scratch = plan
        .runtime_requests()
        .tool_requests()
        .requests()
        .next()
        .unwrap()
        .scratch_request()
        .unwrap();
    let need = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(scratch, r).unwrap(),
    );
    let tools = selected_tool_source::declarations::Catalog::new(
        &f.runtime,
        plan.runtime_requests().tool_requests(),
        deadline,
    )
    .unwrap();
    let runtime = runtime_sources::catalogue::Catalogue::new(
        &f.runtime,
        key.profile_key(),
        plan.runtime_requests(),
        &reachable,
        deadline,
    )
    .unwrap();
    let data =
        catalogue::Catalogue::new(&f.runtime, key.profile_key(), &plan, &reachable, deadline)
            .unwrap();
    elf_metadata::test_reset_borrowed_read_count();
    runtime_sources::prepare::progress_sources_with_data(
        &mut guard,
        &need,
        key.profile_key(),
        plan.runtime_requests(),
        &reachable,
        &tools,
        &runtime,
        Some(&data),
        deadline,
    )
    .unwrap();
    assert!(elf_metadata::test_borrowed_read_count() > 0);
    assert!(
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .unwrap()
            .transaction()
            .unwrap()
            .get(scratch)
            .is_some()
    );
    // A genuine admitted data observation reaches the same source-open boundary.
    input_image::test_reset_anchor_open_count();
    let observed = data.observe(guard.command(), vec![valid]).unwrap();
    assert_eq!(observed.observations().len(), 1);
    assert!(input_image::test_anchor_open_count() > 0);
    guard.discard().unwrap();
    f.restored();
}
