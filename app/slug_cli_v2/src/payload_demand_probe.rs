//! Opt-in native demand diagnostic; run only through the bounded driver.
#[cfg(feature = "native-probe-observer")]
use std::os::fd::FromRawFd;

use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_bzlmod_inputs;

#[test]
#[ignore = "requires run_payload_demand_probe.sh isolation and authentic inputs"]
#[cfg(not(feature = "native-probe-observer"))]
fn authentic_sentinel_demand() {
    panic!("compile this ignored test through the bounded observer driver");
}

#[test]
#[ignore = "requires run_payload_demand_probe.sh isolation and authentic inputs"]
#[cfg(feature = "native-probe-observer")]
fn authentic_sentinel_demand() {
    let observer_fd: i32 = std::env::var("SLUG_SENTINEL_OBSERVER_FD")
        .expect("bounded driver transfers the observer fd")
        .parse()
        .expect("observer fd is decimal");
    // SAFETY: the driver transfers one fresh inherited descriptor to this test
    // process. No Rust owner or concurrent closer exists here; this is its sole
    // adoption, immediately transferred to the safe Core validator.
    let observer_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(observer_fd) };
    let observer_guard =
        slug_core_v2::runtime::probe_observer::ProbeGuard::install_from_fd(observer_fd)
            .expect("bounded driver supplies a sealed zeroed observer channel");
    let observer = observer_guard.observer();
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
    let projection =
        observer.phase(slug_core_v2::runtime::probe_observer::Phase::DiagnosticProjection);
    let projected = accepted.project(|terminal| match terminal.as_ref().as_ref() {
        Ok(_) => TerminalOutput::new(0, String::new(), String::new()),
        Err(error) => TerminalOutput::new(1, String::new(), format!("{error:?}")),
    });
    projection.finish();
    let publication =
        observer.phase(slug_core_v2::runtime::probe_observer::Phase::EventPublication);
    let published = projected.publish();
    publication.finish();
    let terminal_release =
        observer.phase(slug_core_v2::runtime::probe_observer::Phase::TerminalRelease);
    let (_, exit_code, stdout, stderr) = published.into_parts();
    terminal_release.finish();
    println!("native publication exit={exit_code}\n{stdout}");
    eprintln!("{stderr}");
    assert_eq!(exit_code, 0, "native evaluation/publication failed");
    println!("SLUG_SENTINEL_NATIVE_SUCCESS_PUBLISHED_0");
}
