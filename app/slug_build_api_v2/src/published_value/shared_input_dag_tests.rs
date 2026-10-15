//! Composite accounting uses the existing leaf/owner/import fixture owners.
use super::*;

#[derive(Debug, Allocative)]
struct Composite {
    intrinsic: PublicationShape,
    full: PublicationShape,
    children: Vec<SharedPublicationInput>,
}
impl ImmutablePublicationInput for Composite {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.full
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.intrinsic
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.children
    }
}
fn composite(children: Vec<SharedPublicationInput>) -> Arc<Composite> {
    let intrinsic = PublicationShape {
        nodes: 1,
        edges: children.len(),
        bytes: std::mem::size_of::<Composite>()
            + 2 * std::mem::size_of::<usize>()
            + children.capacity() * std::mem::size_of::<SharedPublicationInput>(),
        visits: 5,
        depth: 0,
    };
    let mut full = intrinsic;
    for child in &children {
        let s = child.retained_shape();
        full.nodes = full.nodes.checked_add(s.nodes).unwrap();
        full.edges = full.edges.checked_add(s.edges).unwrap();
        full.bytes = full.bytes.checked_add(s.bytes).unwrap();
        full.visits = full.visits.checked_add(s.visits).unwrap();
        full.depth = full.depth.max(s.depth.checked_add(1).unwrap());
    }
    Arc::new(Composite {
        intrinsic,
        full,
        children,
    })
}
fn composite_owner(root: SharedPublicationInput) -> FunctionEvaluationOwner {
    let stable = Arc::new(Inputs {
        semantic_ids: vec![],
        inputs: vec![],
    });
    let validity = Arc::new(Inputs {
        semantic_ids: vec![99],
        inputs: vec![root],
    });
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, stable, validity).unwrap()
}

#[path = "shared_input_dag_tests/graph.rs"]
mod graph;
#[path = "shared_input_dag_tests/refusal.rs"]
mod refusal;
