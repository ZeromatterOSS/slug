//! Independent typed field oracle. No producer cache or forecast helper is read.
use super::*;
pub(super) fn fields(s: PublicationShape) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
}
fn add(a: &mut PublicationShape, b: PublicationShape) {
    a.nodes += b.nodes;
    a.edges += b.edges;
    a.bytes += b.bytes;
    a.visits += b.visits;
    a.depth = a.depth.max(b.depth);
}
pub(super) fn mapping(raw: &mapping::Raw) -> PublicationShape {
    let allowance = 64 + size_of::<usize>();
    let mut bytes = size_of::<mapping::Body>()
        + 4 * size_of::<usize>()
        + raw.len() * size_of::<(ApparentRepoName, CanonicalRepoName)>();
    for (a, c) in &**raw {
        bytes += size_of::<(ApparentRepoName, CanonicalRepoName)>()
            + 3 * allowance
            + a.retained_capacity_bytes()
            + c.retained_capacity_bytes();
    }
    PublicationShape {
        nodes: 0,
        edges: 2 + 3 * raw.len(),
        visits: 4 + 2 * raw.len(),
        bytes,
        depth: 4,
    }
}
pub(super) fn shapes(raw: &Rows) -> (PublicationShape, PublicationShape, PublicationShape) {
    let allowance = 64 + size_of::<usize>();
    let mut intrinsic = PublicationShape {
        nodes: 0,
        edges: 2,
        visits: 4,
        bytes: size_of::<SourceIdentityTable>()
            + 4 * size_of::<usize>()
            + raw.len() * size_of::<(CompactString, BzlModuleSourceProvenance)>(),
        depth: 5,
    };
    let mut unique = Vec::<&mapping::Raw>::new();
    for (name, p) in &**raw {
        let id = &p.identity;
        let label = &id.label;
        let mut scalars = vec![
            size_of::<(CompactString, BzlModuleSourceProvenance)>(),
            name.capacity(),
            size_of::<BzlModuleIdentity>(),
            size_of::<CanonicalLabel>(),
            label.package().repo().retained_capacity_bytes(),
            label.package().package().retained_capacity_bytes(),
            label.target().retained_capacity_bytes(),
            id.source.retained_text_bytes().unwrap(),
        ];
        if let Some(m) = label.mapping_id() {
            scalars.push(m.retained_capacity_bytes());
        }
        if let slug_build_api_v2::ModuleSourceOrigin::Workspace(path) = &id.source {
            intrinsic.bytes += path.capacity() - path.as_os_str().len();
        }
        intrinsic.bytes += scalars.iter().sum::<usize>() + scalars.len() * allowance;
        intrinsic.edges += scalars.len();
        intrinsic.visits += 6;
        if !unique
            .iter()
            .any(|a| Arc::ptr_eq(a, &id.repository_mapping))
        {
            unique.push(&id.repository_mapping);
        }
    }
    let count = unique.len();
    intrinsic.bytes += count * size_of::<mapping::MappingPair>()
        + 2 * size_of::<usize>()
        + raw.len() * size_of::<SharedPublicationInput>()
        + 2 * size_of::<usize>();
    intrinsic.edges += 2 * count + raw.len();
    // Raw/guard handles plus the two newly retained COPY scalar fields.
    intrinsic.visits += 4 * count + 1 + raw.len() + 1;
    let mut full = intrinsic;
    for (_, p) in &**raw {
        add(&mut full, mapping(&p.identity.repository_mapping));
    }
    let mut union = intrinsic;
    for m in unique {
        add(&mut union, mapping(m));
    }
    (intrinsic, full, union)
}
