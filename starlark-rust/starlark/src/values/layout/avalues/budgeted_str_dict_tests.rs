use super::*;
use crate::values::dict::DictRef;

#[test]
fn bounded_string_dictionary_preserves_key_identity_order_and_starlark_hashes() {
    Heap::temp(|heap| {
        let long = "雪".repeat(171);
        let first = heap.alloc_str(&long);
        let second = heap.alloc_str("other");
        let duplicate = heap.alloc_str(&long);
        assert!(!first.to_value().ptr_eq(duplicate.to_value()));
        assert_ne!(
            Hashed::new(long.as_str()).hash(),
            Hashed::new("other").hash()
        );

        let original = heap.alloc("original value");
        let replacement = heap.alloc("replacement value");
        let other = heap.alloc("other value");
        let rows = [(first, original), (second, other), (duplicate, replacement)];
        let bytes = SmallMap::<StringValue<'_>, Value<'_>>::with_capacity(rows.len())
            .retained_storage_bytes_upper_bound()
            .unwrap()
            + AValueComplex::<MutableDict<'_>>::alloc_size_for_extra_len(0).bytes() as usize;
        // Three incoming rows and end check (4), three actual hashes (10),
        // linear lookup scans (3), one equal-length candidate comparison (4),
        // and the dictionary arena object (1).
        let exact = ExecutionBudget::new(22, bytes, 8);
        let output = heap
            .alloc_budgeted_str_dict_from_rows(rows.len(), rows.map(Ok), &exact)
            .unwrap();
        assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        let output = DictRef::from_value(output).unwrap();
        assert_eq!(output.len(), 2);
        for ((key, value), (expected_key, expected_value)) in output
            .iter_hashed()
            .zip([(first, replacement), (second, other)])
        {
            assert!(key.key().ptr_eq(expected_key.to_value()));
            assert_eq!(
                key.hash(),
                expected_key.to_value().get_hashed().unwrap().hash()
            );
            assert!(value.ptr_eq(expected_value));
        }
        assert!(
            output
                .get(duplicate.to_value())
                .unwrap()
                .unwrap()
                .ptr_eq(replacement)
        );

        for (work, allocation, limit, admitted_rows) in [
            // Refuse one unit before computing the first 513-byte hash.
            (4, bytes, ExecutionLimit::Work, 1),
            // Refuse the equal-length byte comparison after reaching the duplicate.
            (19, bytes, ExecutionLimit::Work, 3),
            // Refuse the final arena object after successful map construction.
            (22, bytes - 1, ExecutionLimit::AllocationBytes, 3),
        ] {
            let fresh_rows = [
                (heap.alloc_str(&long), original),
                (second, other),
                (heap.alloc_str(&long), replacement),
            ];
            let before = (heap.allocated_bytes(), heap.available_bytes());
            let budget = ExecutionBudget::new(work, allocation, 8);
            let mut produced = 0;
            let input = fresh_rows.into_iter().map(|row| {
                produced += 1;
                Ok(row)
            });
            let error = heap
                .alloc_budgeted_str_dict_from_rows(fresh_rows.len(), input, &budget)
                .unwrap_err();
            assert!(matches!(error.kind(), crate::ErrorKind::Other(cause)
                if cause.downcast_ref::<ExecutionLimit>() == Some(&limit)));
            assert_eq!(budget.check(), Err(limit));
            assert_eq!(produced, admitted_rows);
            assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);
        }
    });
}
