//! Smoke test: build a `SlugOssReConfiguration` from CLI args + the BuildBuddy
//! API key, hand it to `REClientBuilder::build_and_connect`, and call
//! `Capabilities`. If this returns ok, the wire path between slug's REv2
//! client and BuildBuddy is healthy; any failures from subsequent
//! action-level integration are higher up the stack.
//!
//! Usage:
//!   cargo run --example probe_re -- <engine_uri> <header_kv...>
//! Example:
//!   cargo run --example probe_re -- \
//!     grpcs://remote.buildbuddy.io \
//!     "x-buildbuddy-api-key=<KEY>"

use std::env;

use remote_execution::REClientBuilder;
use slug_re_configuration::HttpHeader;
use slug_re_configuration::SlugOssReConfiguration;

fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut args = env::args().skip(1);
    let engine = args
        .next()
        .expect("usage: probe_re <engine_uri> [HEADER_KV...]");
    let headers: Vec<HttpHeader> = args
        .map(|kv| {
            let (key, value) = kv
                .split_once('=')
                .unwrap_or_else(|| panic!("header arg not KEY=VALUE: {kv}"));
            HttpHeader {
                key: key.to_owned(),
                value: value.to_owned(),
            }
        })
        .collect();

    let config = SlugOssReConfiguration {
        cas_address: Some(engine.clone()),
        engine_address: Some(engine.clone()),
        action_cache_address: Some(engine.clone()),
        tls: engine.starts_with("grpcs://") || engine.starts_with("https://"),
        http_headers: headers,
        capabilities: Some(true),
        ..Default::default()
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime.block_on(async move {
        match REClientBuilder::build_and_connect(&config).await {
            Ok(_client) => println!("RE wire OK against {engine}"),
            Err(e) => {
                eprintln!("RE wire FAILED against {engine}: {e:#}");
                std::process::exit(1);
            }
        }
    });
}
