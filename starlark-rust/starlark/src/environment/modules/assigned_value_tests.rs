//! Freeze roots include private and unnamed slots, independent of public names.
use super::*;
#[test]
fn assigned_values_visit_private_anonymous_slots_and_propagate_consumer_failure() {
    let module = Module::new();
    module.set("visible", module.heap().alloc(1));
    module.set_private(
        module.frozen_heap().alloc_str_intern("private"),
        module.heap().alloc(2),
    );
    let anonymous = ModuleSlotId(2);
    module.slots().ensure_slot(anonymous);
    module.slots().set_slot(anonymous, module.heap().alloc(3));
    assert_eq!(
        module
            .names()
            .map(|n| n.as_str().to_owned())
            .collect::<Vec<_>>(),
        ["visible"]
    );
    let mut values = Vec::new();
    module
        .visit_assigned_values(&mut |value| {
            // The visitor can inspect the same original module's slots without a
            // live mutable slot borrow hiding inside the visitation API.
            assert_eq!(module.get("visible").unwrap().unpack_i32(), Some(1));
            values.push(value.unpack_i32().unwrap());
            Ok(())
        })
        .unwrap();
    assert_eq!(values, [1, 2, 3]);
    let mut calls = 0;
    let error = module
        .visit_assigned_values(&mut |_| {
            calls += 1;
            if calls == 2 {
                Err(crate::Error::new_other(std::io::Error::other(
                    "bounded consumer refusal",
                )))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(calls, 2);
    assert!(error.to_string().contains("bounded consumer refusal"));
    let frozen = module.freeze().unwrap();
    assert_eq!(
        frozen
            .module
            .get_slot(anonymous)
            .unwrap()
            .to_value()
            .unpack_i32(),
        Some(3)
    );
}
