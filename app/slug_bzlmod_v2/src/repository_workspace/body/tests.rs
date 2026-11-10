//! Pure logical-content/storage tests; no native resource or authority claim.
use starlark_map::Hashed;
use starlark_map::small_map::SmallMap;

use super::*;

fn packed(bytes: &[u8]) -> RepositoryWorkspaceBody {
    let mut builder = RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    for chunk in bytes.chunks(CHUNK) {
        builder.append(chunk, &mut |_| Ok::<_, ()>(())).unwrap();
    }
    builder.finish().unwrap()
}
fn noise(length: usize) -> Vec<u8> {
    let mut n = 0x1234_5678u32;
    (0..length)
        .map(|_| {
            n ^= n << 13;
            n ^= n >> 17;
            n ^= n << 5;
            n as u8
        })
        .collect()
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Writes(Vec<(u8, Vec<u8>)>);
impl Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, b: &[u8]) {
        self.0.push((0, b.to_vec()));
    }
    fn write_usize(&mut self, v: usize) {
        self.0.push((1, v.to_ne_bytes().to_vec()));
    }
}
fn writes(body: &RepositoryWorkspaceBody) -> Writes {
    let mut h = Writes::default();
    body.hash(&mut h);
    h
}
#[test]
fn raw_packed_all_boundaries_preserve_full_content_and_exact_writer() {
    for length in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK + 17] {
        for bytes in [vec![7; length], noise(length)] {
            let arc: Arc<[u8]> = Arc::from(bytes.as_slice());
            let raw = RepositoryWorkspaceBody::from_arc(arc.clone());
            assert!(Arc::ptr_eq(raw.raw_arc().unwrap(), &arc));
            let encoded = packed(&bytes);
            let independent = packed(&bytes);
            assert_eq!(raw, encoded);
            assert_eq!(encoded, independent);
            assert!(!encoded.ptr_eq(&independent));
            assert!(encoded.raw_arc().is_none());
            assert_eq!(raw.routing_hash(), StarlarkHashValue::new(&raw));
            assert_eq!(encoded.routing_hash(), StarlarkHashValue::new(&encoded));
            let mut reference = Writes::default();
            length.hash(&mut reference);
            for chunk in bytes.chunks(CHUNK) {
                reference.write(chunk);
            }
            assert_eq!(writes(&raw), reference);
            assert_eq!(writes(&encoded), reference);
            let mut observed = Vec::new();
            encoded.for_each_chunk(&mut |b| {
                assert!(b.len() <= CHUNK);
                observed.extend_from_slice(b)
            });
            assert_eq!(observed, bytes);
            assert!(encoded.equals_slice(&bytes));
            if length != 0 {
                let mut changed = bytes.clone();
                changed[length - 1] ^= 1;
                assert_ne!(encoded, packed(&changed));
                assert!(!encoded.equals_slice(&changed));
            }
        }
    }
}
#[test]
fn range_copy_refusal_short_circuit_and_weak_storage_lifetime() {
    let bytes = noise(2 * CHUNK + 13);
    let body = packed(&bytes);
    for (start, len) in [
        (0, 0),
        (0, 1),
        (CHUNK - 3, 10),
        (CHUNK + 2, CHUNK),
        (bytes.len(), 0),
    ] {
        let mut output = vec![0; len];
        assert!(body.copy_into(start, &mut output));
        assert_eq!(output, &bytes[start..start + len]);
        assert!(body.range_eq(start, &output));
    }
    let mut untouched = [9; 8];
    assert!(!body.copy_into(usize::MAX, &mut untouched));
    assert_eq!(untouched, [9; 8]);
    assert!(!body.range_eq(bytes.len(), &[1]));
    let mut calls = 0;
    assert_eq!(
        body.try_for_each_chunk(&mut |_| {
            calls += 1;
            Err::<(), _>(17)
        }),
        Err(17)
    );
    assert_eq!(calls, 1);
    assert!(body.control_storage_bytes() > size_of::<PackedData>());
    let stored = match &body.0 {
        Repr::Packed(d) => d.blocks.iter().map(|b| b.bytes.len()).sum::<usize>(),
        _ => unreachable!(),
    };
    assert_eq!(
        body.native_storage_bytes(),
        body.control_storage_bytes() + stored
    );
    assert_eq!(
        body.retained_storage_bytes(),
        body.control_storage_bytes() + bytes.len()
    );
    let weak = body.downgrade();
    let clone = body.clone();
    assert!(body.ptr_eq(&clone));
    assert_eq!(body.allocation_identity(), clone.allocation_identity());
    drop(body);
    let upgraded = weak.upgrade().unwrap();
    assert!(clone.ptr_eq(&upgraded));
    drop(upgraded);
    drop(clone);
    assert!(weak.upgrade().is_none());
    let raw = RepositoryWorkspaceBody::from_arc(Arc::from([]));
    assert!(raw.control_storage_bytes() > 0);
    let weak = raw.downgrade();
    drop(raw);
    assert!(weak.upgrade().is_none());
}
#[test]
fn builder_rejects_bad_framing_and_admission_before_retained_block_allocation() {
    let mut builder = RepositoryWorkspaceBodyBuilder::new(CHUNK + 1).unwrap();
    let mut admissions = 0;
    assert!(matches!(
        builder.append(&[1], &mut |_| {
            admissions += 1;
            Ok::<_, ()>(())
        }),
        Err(RepositoryWorkspaceBodyBuildError::Body(Error::Length))
    ));
    assert_eq!(admissions, 0);
    assert!(matches!(
        builder.append(&vec![1; CHUNK], &mut |_| Err(7)),
        Err(RepositoryWorkspaceBodyBuildError::Admission(7))
    ));
    // Retrying the identical first block succeeds: refusal retained no block
    // and did not advance the framing cursor.
    builder
        .append(&vec![1; CHUNK], &mut |n| {
            assert!(n > 0);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(matches!(builder.finish(), Err(Error::Length)));
    let mut builder = RepositoryWorkspaceBodyBuilder::new(1).unwrap();
    builder.append(&[5], &mut |_| Ok::<_, ()>(())).unwrap();
    assert!(matches!(
        builder.append(&[5], &mut |_| Ok::<_, ()>(())),
        Err(RepositoryWorkspaceBodyBuildError::Body(Error::Length))
    ));
    assert!(builder.finish().unwrap().equals_slice(&[5]));
    assert!(matches!(
        RepositoryWorkspaceBodyBuilder::storage_bytes(usize::MAX),
        Err(Error::Storage)
    ));
}
#[test]
fn codec_checks_lz4_grammar_full_bytes_and_bounded_lengths() {
    for length in [0, 1, 12, 13, CHUNK - 2, CHUNK - 1, CHUNK] {
        for bytes in [vec![3; length], noise(length)] {
            let encoded = codec::encode(&bytes).unwrap();
            assert!(encoded.len() <= 72_109);
            let mut output = vec![0; bytes.len()];
            codec::decode(&encoded, &mut output).unwrap();
            assert_eq!(output, bytes);
            // A truncated/extended stream can sometimes be a legal equivalent
            // encoding. Every successful decode must still match ALL bytes.
            let mut trailing = encoded.clone();
            trailing.push(0);
            for candidate in [&encoded[..encoded.len() - 1], trailing.as_slice()] {
                if codec::decode(candidate, &mut output).is_ok() {
                    assert_eq!(output, bytes);
                }
            }
            let mut wrong = vec![0; bytes.len() + 1];
            assert!(codec::decode(&encoded, &mut wrong).is_err());
        }
    }
    // Literal token and equivalent ignored final low nibble, independent of
    // the encoder. Legal byte-different streams do not define content identity.
    for encoded in [&[0x10, b'a'][..], &[0x11, b'a'][..]] {
        let mut output = [0];
        codec::decode(encoded, &mut output).unwrap();
        assert_eq!(output, *b"a");
    }
    let mut changed = [0];
    codec::decode(&[0x10, b'b'], &mut changed).unwrap();
    assert_ne!(changed, *b"a");
    assert!(codec::decode(&[0x20, b'a'], &mut [0; 2]).is_err()); // truncated literal
    assert!(codec::decode(&[0xf0, 0xff], &mut [0]).is_err()); // missing extended length
    assert!(codec::decode(&[0x10, b'a', 0, 0, 0], &mut [0; 5]).is_err()); // zero offset
    assert!(codec::decode(&[0x00, 1, 0, 0], &mut [0; 4]).is_err()); // before output
    assert!(codec::decode(&[0x20, b'a', b'b'], &mut [0]).is_err()); // output short
    assert!(matches!(
        codec::encode(&vec![0; CHUNK + 1]),
        Err(Error::Length)
    ));
    assert!(matches!(
        codec::decode(&vec![0; 72_110], &mut []),
        Err(Error::Length)
    ));
    assert!(matches!(
        codec::decode(&[0], &mut vec![0; CHUNK + 1]),
        Err(Error::Length)
    ));
}
#[test]
fn cached_routing_collisions_keep_full_equality_and_physical_storage_positive() {
    let mut first = packed(&vec![1; CHUNK]);
    let mut second = packed(&vec![2; CHUNK]);
    let collision = StarlarkHashValue::new_unchecked(0);
    for body in [&mut first, &mut second] {
        let Repr::Packed(data) = &mut body.0 else {
            unreachable!()
        };
        Arc::get_mut(data).unwrap().routing = collision;
    }
    assert_ne!(first, second);
    let mut pool = SmallMap::new();
    pool.insert_hashed(Hashed::new_unchecked(collision, first.clone()), 1);
    pool.insert_hashed(Hashed::new_unchecked(collision, second.clone()), 2);
    assert_eq!(pool.len(), 2);
    assert_eq!(
        pool.get_full_hashed(Hashed::new_unchecked(collision, &first))
            .unwrap()
            .2,
        &1
    );
    assert_eq!(
        pool.get_full_hashed(Hashed::new_unchecked(collision, &second))
            .unwrap()
            .2,
        &2
    );
    let compressible = packed(&vec![0; 3 * CHUNK]);
    assert!(compressible.native_storage_bytes() < compressible.len());
    let random = packed(&noise(3 * CHUNK));
    assert!(random.native_storage_bytes() >= random.len());
    assert!(
        RepositoryWorkspaceBodyBuilder::storage_bytes(3 * CHUNK).unwrap()
            >= 2 * table_bytes(3).unwrap() + size_of::<RepositoryWorkspaceBodyBuilder>()
    );
}
#[test]
fn codec_workspace_bounds_preserve_admission_and_cover_lz4_tables() {
    // Pinned0.14.0 compress.rs chooses a 4096-entry u16 table for <65535
    // bytes and a 4096-entry u32 table otherwise; both fit the old allowance.
    assert_eq!(lz4_flex::block::get_maximum_output_size(CHUNK), 72_109);
    assert!(
        codec::scratch_components() <= RepositoryWorkspaceBodyBuilder::encoding_scratch_bytes()
    );
    assert_eq!(
        RepositoryWorkspaceBodyBuilder::encoding_scratch_bytes(),
        2 * 1024 * 1024
    );
    assert_eq!(RepositoryWorkspaceBody::decode_scratch_bytes(), 291_344);
    // Four nested logical block buffers plus conservative scalar/slice decoder
    // controls still fit; no dictionary, decoder allocation or decoded cache.
    assert!(4 * CHUNK + 8192 <= RepositoryWorkspaceBody::decode_scratch_bytes());
}

#[test]
fn independent_valid_block_storage_flags_do_not_enter_content_equality_or_hash() {
    let bytes = vec![0; CHUNK];
    let compressed = packed(&bytes);
    let mut raw_block = packed(&noise(CHUNK));
    let Repr::Packed(data) = &mut raw_block.0 else {
        unreachable!()
    };
    let data = Arc::get_mut(data).unwrap();
    // Private valid representation fixture: same decoded bytes stored as a raw
    // block, not a public encoded factory or a corruption fallback.
    data.blocks[0] = Block {
        bytes: Arc::from(bytes.as_slice()),
        compressed: false,
    };
    data.routing = compressed.routing_hash();
    data.control = arc_layout(Layout::new::<PackedData>()).unwrap()
        + table_bytes(1).unwrap()
        + arc_bytes(CHUNK).unwrap()
        - CHUNK;
    data.native = data.control + CHUNK;
    data.retained = data.control + CHUNK;
    assert_eq!(compressed, raw_block);
    assert_eq!(writes(&compressed), writes(&raw_block));
    assert!(compressed.native_storage_bytes() < raw_block.native_storage_bytes());
}

#[inline(never)]
fn native_like_stack_caller_exercises_body_content() {
    // A live caller read block models the nested buffer of an ordinary native
    // FD callback. This is a fixed-stack content regression, not native custody.
    let mut read_buffer = [0xa5u8; CHUNK];
    std::hint::black_box(&mut read_buffer);
    // Exercise both fresh table regimes (<65535 and >=65535), full blocks
    // and nested multi-block operations with the caller buffer live throughout.
    for length in [CHUNK - 2, CHUNK - 1, CHUNK, 2 * CHUNK + 17] {
        for bytes in [vec![0x6a; length], noise(length)] {
            let first = packed(&bytes); // encode, roundtrip validation and finish
            let second = packed(&bytes);
            let raw = RepositoryWorkspaceBody::from_arc(Arc::from(bytes.as_slice()));
            assert!(!first.ptr_eq(&second));
            assert_eq!(first, second);
            assert_eq!(raw, first);
            assert_eq!(first, raw);
            let mut hash = StarlarkHasher::new();
            first.hash(&mut hash);
            assert_eq!(hash.finish_small(), raw.routing_hash());
            let start = if length > CHUNK {
                CHUNK - 7
            } else {
                length / 2
            };
            let end = (start + CHUNK + 13).min(length);
            assert!(first.range_eq(start, &bytes[start..end]));
            let mut output = vec![0u8; end - start];
            assert!(first.copy_into(start, &mut output));
            assert_eq!(output, bytes[start..end]);
            let mut changed = bytes.clone();
            changed[length / 2] ^= 1;
            assert_ne!(first, packed(&changed));
            assert!(!first.range_eq(0, &changed));
            std::hint::black_box(&read_buffer);
        }
    }
    assert_eq!(
        read_buffer
            .iter()
            .map(|byte| usize::from(*byte))
            .sum::<usize>(),
        CHUNK * 0xa5
    );
}

#[test]
fn body_build_equality_hash_and_ranges_fit_fixed_256k_thread_with_live_read_block() {
    std::thread::Builder::new()
        .name("workspace-body-fixed-stack".into())
        .stack_size(256 * 1024)
        .spawn(native_like_stack_caller_exercises_body_content)
        .unwrap()
        .join()
        .unwrap();
}
