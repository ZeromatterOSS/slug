//! Pure closed-body construction evidence; no native admission or allocator claim.
use std::mem::size_of;

use super::construction_work_for_test as work;
use super::*;
use crate::repository_workspace::*;

const OUTPUT: usize = 72_109;
pub(super) fn noise(length: usize) -> Vec<u8> {
    let mut state = 0x12345678u32;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}
pub(super) fn build(bytes: &[u8]) -> RepositoryWorkspaceBody {
    let mut builder = RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    for raw in bytes.chunks(CHUNK) {
        builder.append(raw, &mut |_| Ok::<(), ()>(())).unwrap();
    }
    builder.finish().unwrap()
}
// Independent original public compressor and fresh initialized buffers, never
// codec::encode or the proposed reusable workspace implementation.
pub(super) fn fresh_selected(raw: &[u8]) -> (bool, Vec<u8>) {
    let mut encoded = vec![0; OUTPUT];
    let written = lz4_flex::block::compress_into(raw, &mut encoded).unwrap();
    encoded.truncate(written);
    let mut decoded = vec![0; raw.len()];
    assert_eq!(
        lz4_flex::block::decompress_into(&encoded, &mut decoded).unwrap(),
        raw.len()
    );
    assert_eq!(decoded, raw);
    let compressed = written < raw.len();
    (compressed, if compressed { encoded } else { raw.to_vec() })
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Writes(Vec<Vec<u8>>);
impl Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.push(bytes.to_vec());
    }
}
fn writes(body: &RepositoryWorkspaceBody) -> Writes {
    let mut writer = Writes::default();
    body.hash(&mut writer);
    writer
}
fn canonical(body: RepositoryWorkspaceBody) -> Arc<str> {
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationInstanceId;
    use starlark_map::small_map::SmallMap;
    let absolute = |unix: &str| {
        if cfg!(windows) {
            format!("C:{unix}")
        } else {
            unix.to_owned()
        }
    };
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            NormalizedAbsolutePath::new(absolute("/workspace")).unwrap(),
            Arc::new(crate::HostSelectedExtensionOwner::testing(
                "body-construction",
            )),
            CanonicalRepoName::new("body+construction").unwrap(),
            3,
            Arc::new(crate::RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse("@@//:repository.bzl").unwrap(),
                    rule_name: "body".into(),
                },
                attributes: Arc::new(SmallMap::new()),
            }),
            NormalizedAbsolutePath::new(absolute("/logical/body")).unwrap(),
            PathObservationInstanceId::new(17),
            7,
            RepositoryWorkspaceRetentionLimits::new(8, 16 * 1024 * 1024, 1024 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap(),
    );
    let initial = RepositoryWorkspaceTranscript::new(reservation, 0o755).unwrap();
    let request = initial
        .request(RepositoryWorkspaceOperation::File {
            path: "file".into(),
            body: body.clone(),
            mode: 0o644,
        })
        .unwrap();
    let terminal = initial
        .complete(
            request,
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            vec![
                RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory)
                    .unwrap(),
                RepositoryWorkspaceEntry::new(
                    "file".into(),
                    0o644,
                    RepositoryWorkspaceNode::File(body),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .realize(1)
        .unwrap();
    crate::source_preparation::general_workspace_source_identity(&terminal)
}
pub(super) fn closed_semantics(body: &RepositoryWorkspaceBody, bytes: &[u8], admissions: &[usize]) {
    let Repr::Packed(data) = &body.0 else {
        panic!("closed builder must return Packed");
    };
    assert_eq!(data.blocks.len(), bytes.len().div_ceil(CHUNK));
    let mut payload = 0usize;
    let mut control =
        arc_layout(Layout::new::<PackedData>()).unwrap() + table_bytes(data.blocks.len()).unwrap();
    let mut model_admissions = Vec::new();
    for (block, raw) in data.blocks.iter().zip(bytes.chunks(CHUNK)) {
        let (compressed, selected) = fresh_selected(raw);
        assert_eq!(block.compressed, compressed);
        assert_eq!(block.bytes.as_ref(), selected);
        let allocation = arc_bytes(selected.len()).unwrap();
        model_admissions.push(allocation);
        payload += selected.len();
        control += allocation - selected.len();
    }
    assert_eq!(admissions, model_admissions);
    assert_eq!(body.native_storage_bytes(), control + payload);
    assert_eq!(body.control_storage_bytes(), control);
    assert_eq!(body.retained_storage_bytes(), control + bytes.len());
    assert!(
        RepositoryWorkspaceBodyBuilder::storage_bytes(bytes.len()).unwrap()
            >= size_of::<RepositoryWorkspaceBodyBuilder>()
                + 2 * table_bytes(data.blocks.len()).unwrap()
    );
    let raw = RepositoryWorkspaceBody::from_arc(Arc::from(bytes));
    let independent = build(bytes);
    assert_eq!(body, &raw);
    assert_eq!(body, &independent);
    assert!(!body.ptr_eq(&independent));
    let Repr::Packed(independent_data) = &independent.0 else {
        unreachable!()
    };
    for (left, right) in data.blocks.iter().zip(independent_data.blocks.iter()) {
        assert!(!Arc::ptr_eq(&left.bytes, &right.bytes));
    }
    assert!(body.equals_slice(bytes));
    let mut restored = Vec::new();
    body.try_for_each_chunk(&mut |chunk| {
        restored.extend_from_slice(chunk);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(restored, bytes);
    let mut reference = Writes::default();
    bytes.len().hash(&mut reference);
    for chunk in bytes.chunks(CHUNK) {
        reference.write(chunk);
    }
    assert_eq!(writes(body), reference);
    assert_eq!(writes(&raw), reference);
    assert_eq!(body.routing_hash(), StarlarkHashValue::new(&raw));
    assert_eq!(canonical(body.clone()), canonical(raw));
}
fn required_work(count: work::Snapshot, calls: usize, bytes: usize) {
    for (actual_calls, actual_bytes) in [
        (count.encode_calls, count.encode_bytes),
        (count.decode_calls, count.decode_bytes),
        (count.comparison_calls, count.comparison_bytes),
    ] {
        assert_eq!((actual_calls, actual_bytes), (calls as u64, bytes as u64));
    }
}
fn bounded_workspace(count: work::Snapshot, maximum_raw: usize) {
    assert_eq!(
        (count.output_reserve_calls, count.validation_reserve_calls),
        (1, 1),
        "two construction-local transient reserve requests, not two per append"
    );
    assert_eq!(
        (count.output_capacity_bytes, count.validation_capacity_bytes),
        (OUTPUT as u64, CHUNK as u64)
    );
    assert_eq!(
        (count.output_init_calls, count.validation_init_calls),
        (1, 1)
    );
    assert_eq!(
        (count.output_init_bytes, count.validation_init_bytes),
        (OUTPUT as u64, maximum_raw as u64)
    );
}

#[test]
fn construction_workspace_preserves_closed_blocks_and_bounds_transient_work() {
    let mut bytes = vec![b'c'; CHUNK];
    bytes.extend(noise(CHUNK));
    bytes.extend(vec![b'z'; 37]);
    let witness = work::begin();
    let mut builder = RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    let mut admissions = Vec::new();
    for raw in bytes.chunks(CHUNK) {
        builder
            .append(raw, &mut |n| {
                admissions.push(n);
                Ok::<(), ()>(())
            })
            .unwrap();
    }
    let body = builder.finish().unwrap();
    let count = witness.snapshot();
    drop(witness);
    required_work(count, 3, bytes.len());
    closed_semantics(&body, &bytes, &admissions);
    let weak = body.downgrade();
    let clone = body.clone();
    drop(body);
    assert!(weak.upgrade().is_some());
    drop(clone);
    assert!(weak.upgrade().is_none());
    // The independent full semantic/storage/identity/lifetime oracle precedes RED.
    bounded_workspace(count, CHUNK);
}

#[test]
fn construction_workspace_refusal_retry_and_empty_paths_preserve_full_work() {
    let witness = work::begin();
    let empty = RepositoryWorkspaceBodyBuilder::new(0)
        .unwrap()
        .finish()
        .unwrap();
    let mut invalid = RepositoryWorkspaceBodyBuilder::new(CHUNK).unwrap();
    assert!(matches!(
        invalid.append(&[1], &mut |_| Ok::<(), ()>(())),
        Err(RepositoryWorkspaceBodyBuildError::Body(Error::Length))
    ));
    assert!(matches!(invalid.finish(), Err(Error::Length)));
    let empty_work = witness.snapshot();
    drop(witness);
    assert_eq!(empty_work, work::Snapshot::default());
    closed_semantics(&empty, &[], &[]);
    let empty_weak = empty.downgrade();
    drop(empty);
    assert!(empty_weak.upgrade().is_none());

    let first = vec![b'a'; CHUNK];
    let changed = noise(CHUNK);
    let witness = work::begin();
    let mut builder = RepositoryWorkspaceBodyBuilder::new(CHUNK).unwrap();
    let mut refused = None;
    assert!(matches!(
        builder.append(&first, &mut |n| {
            refused = Some(n);
            Err(17u8)
        }),
        Err(RepositoryWorkspaceBodyBuildError::Admission(17))
    ));
    let mut admitted = Vec::new();
    builder
        .append(&changed, &mut |n| {
            admitted.push(n);
            Ok::<(), u8>(())
        })
        .unwrap();
    let body = builder.finish().unwrap();
    let count = witness.snapshot();
    drop(witness);
    required_work(count, 2, 2 * CHUNK);
    let (_, first_selected) = fresh_selected(&first);
    assert_eq!(refused, Some(arc_bytes(first_selected.len()).unwrap()));
    closed_semantics(&body, &changed, &admitted);
    assert!(!body.equals_slice(&first));
    let weak = body.downgrade();
    drop(body);
    assert!(weak.upgrade().is_none());
    // A refused attempt never advances cursor/routing or publishes first bytes.
    bounded_workspace(count, CHUNK);
}

#[test]
fn construction_workspace_single_short_initializes_only_its_logical_length() {
    let bytes = vec![b's'; 37];
    let witness = work::begin();
    let mut builder = RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    let mut admissions = Vec::new();
    builder
        .append(&bytes, &mut |n| {
            admissions.push(n);
            Ok::<(), ()>(())
        })
        .unwrap();
    let body = builder.finish().unwrap();
    let count = witness.snapshot();
    drop(witness);
    required_work(count, 1, bytes.len());
    closed_semantics(&body, &bytes, &admissions);
    let weak = body.downgrade();
    let clone = body.clone();
    drop(body);
    assert!(weak.upgrade().is_some());
    drop(clone);
    assert!(weak.upgrade().is_none());
    bounded_workspace(count, bytes.len());
}
