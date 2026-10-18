//! Complete pre-change public headers, used only to check native row widths.
//! The old marker is layout-only; no former callback behavior is constructed.
#![allow(dead_code)]

use std::mem::align_of;
use std::mem::size_of;

use super::*;

// Every field follows the pre-resource-retirement SpawnSpec. The former
// resource descriptor held one single-variant ZST; its Option occupied a byte.
struct PreviousSpawnSpec {
    invocation: RetainedSpawnInvocation,
    command_line: RetainedCommandLine,
    inputs: ArtifactInputs,
    tools: ArtifactInputs,
    outputs: Arc<[ActionOutput]>,
    unused_inputs_list: Option<AnalysisArtifact>,
    environment: RetainedActionEnvironment,
    execution_requirements: CanonicalStringMap,
    mnemonic: CompactString,
    progress_message: Option<CompactString>,
    resource_set: Option<()>,
}

enum PreviousActionPayload {
    Legacy(LegacyActionSpec),
    Spawn(PreviousSpawnSpec),
    Symlink(SymlinkSpec),
    SolibSymlink(SolibSymlinkSpec),
    CppCompile(CppCompileSpec),
    BuildInfoTransform(BuildInfoTransformSpec),
    ArgsWrite(ArgsWriteSpec),
    RunfilesSupport(RunfilesSupportActionSpec),
}

struct PreviousActionSpec {
    payload: PreviousActionPayload,
    exec_properties: BTreeMap<String, String>,
    exec_group: Option<String>,
}

fn no_wider<Current, Previous>() {
    assert!(
        size_of::<Current>() <= size_of::<Previous>(),
        "{} width {} exceeds previous {}",
        std::any::type_name::<Current>(),
        size_of::<Current>(),
        size_of::<Previous>(),
    );
    assert!(
        align_of::<Current>() <= align_of::<Previous>(),
        "{} alignment {} exceeds previous {}",
        std::any::type_name::<Current>(),
        align_of::<Current>(),
        align_of::<Previous>(),
    );
}

#[test]
fn retained_resource_public_headers_fit_complete_previous_layouts() {
    assert_eq!(size_of::<Option<()>>(), 1);
    assert_eq!(align_of::<Option<()>>(), 1);
    no_wider::<SpawnSpec, PreviousSpawnSpec>();
    no_wider::<ActionPayload, PreviousActionPayload>();
    no_wider::<ActionSpec, PreviousActionSpec>();
}
