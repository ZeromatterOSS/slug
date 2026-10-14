use super::*;
#[derive(Debug, Allocative)]
struct Bytes(usize);
impl StructuralFunctionOwnerPart for Bytes {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, o: &dyn StructuralFunctionOwnerPart) -> bool {
        o.as_any()
            .downcast_ref::<Self>()
            .is_some_and(|o| self.0 == o.0)
    }
    fn hash_part(&self, h: &mut dyn Hasher) {
        h.write_usize(self.0);
    }
    fn retained_bytes(&self) -> usize {
        self.0
    }
}
#[test]
fn native_owner_and_part_arc_headers_are_forecast_before_allocation() {
    let part = Arc::new(Bytes(8));
    let owner =
        FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, part.clone(), part).unwrap();
    assert_eq!(
        owner.intrinsic_shape().bytes,
        16 + std::mem::size_of::<OwnerData>() + 6 * std::mem::size_of::<usize>()
    );
    assert!(
        FunctionEvaluationOwner::new(
            FunctionOwnerDomain::BzlLoad,
            Arc::new(Bytes(super::super::BYTE_LIMIT - 1)),
            Arc::new(Bytes(1))
        )
        .is_err()
    );
}
