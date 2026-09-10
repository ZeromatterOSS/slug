//! Opt-in native demand diagnostic; run only through the bounded driver.
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_bzlmod_inputs;

#[test]
#[ignore = "requires run_payload_demand_probe.sh isolation and authentic inputs"]
fn authentic_sentinel_demand() {
    let scratch = std::env::var("SLUG_SENTINEL_SCRATCH").expect("use the bounded driver");
    let workspace = std::path::Path::new(&scratch).join("workspace");
    let registry = format!("--registry=file://{scratch}/registry");
    let request = BuildRequest::parse(&[registry.as_str(), "//:root"]).unwrap();
    let accepted = match evaluate_workspace_build_command_with_bzlmod_inputs(
        &workspace,
        &request.targets,
        request.bzlmod_policy,
        normalize_bzlmod_environment_value(None).unwrap(),
        request.lockfile_mode,
        &request.registry_urls,
        Default::default(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => panic!("native evaluation failed before acceptance: {error:?}"),
    };
    let projected = accepted.project(|terminal| match terminal.as_ref().as_ref() {
        Ok(_) => TerminalOutput::new(0, String::new(), String::new()),
        Err(error) => TerminalOutput::new(1, String::new(), format!("{error:?}")),
    });
    let (_, exit_code, stdout, stderr) = projected.publish().into_parts();
    println!("native publication exit={exit_code}\n{stdout}");
    eprintln!("{stderr}");
    assert_eq!(exit_code, 0, "native evaluation/publication failed");
    println!("SLUG_SENTINEL_NATIVE_SUCCESS_PUBLISHED_0");
}
