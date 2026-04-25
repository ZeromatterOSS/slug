//! Bazel Build Event Protocol (BEP) and Build Event Service (BES) schemas,
//! vendored from upstream sources for parity with Bazel's `--build_event_*`
//! and `--bes_*` flags.
//!
//! Sources:
//! - `build_event_stream.proto` from Bazel 9.x
//!   (`src/main/java/com/google/devtools/build/lib/buildeventstream/proto/`).
//! - `publish_build_event.proto` + transitively imported `build_events.proto`
//!   and `build_status.proto` from googleapis
//!   (commit `de157ca3`, matching Bazel 9.x's pinned version).
//! - Supporting `google/api/*` annotations from the same googleapis commit.
//! - Transitively imported Bazel protos (`action_cache`, `command_line`,
//!   `failure_details`, `invocation_policy`, `option_filters`,
//!   `strategy_policy`, `package_load_metrics`, and the
//!   `analysis_cache_service_metadata_status` proto).
//!
//! No schema modifications: we own the import, upstream owns evolution.

#![allow(clippy::all)]

pub mod file_sink;
pub mod grpc_sink;
pub mod translate;

pub mod build_event_stream {
    include!(concat!(env!("OUT_DIR"), "/build_event_stream.rs"));
}

pub mod google {
    pub mod api {
        include!(concat!(env!("OUT_DIR"), "/google.api.rs"));
    }
    pub mod devtools {
        pub mod build {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/google.devtools.build.v1.rs"));
            }
        }
    }
}

pub mod blaze {
    include!(concat!(env!("OUT_DIR"), "/blaze.rs"));

    pub mod invocation_policy {
        include!(concat!(env!("OUT_DIR"), "/blaze.invocation_policy.rs"));
    }

    pub mod strategy_policy {
        include!(concat!(env!("OUT_DIR"), "/blaze.strategy_policy.rs"));
    }
}

pub mod command_line {
    include!(concat!(env!("OUT_DIR"), "/command_line.rs"));
}

pub mod failure_details {
    include!(concat!(env!("OUT_DIR"), "/failure_details.rs"));
}

pub mod options {
    include!(concat!(env!("OUT_DIR"), "/options.rs"));
}

pub mod devtools {
    pub mod build {
        pub mod lib {
            pub mod packages {
                pub mod metrics {
                    include!(concat!(
                        env!("OUT_DIR"),
                        "/devtools.build.lib.packages.metrics.rs"
                    ));
                }
            }
        }
    }
}

pub mod devtools_blaze_proto {
    include!(concat!(env!("OUT_DIR"), "/devtools_blaze_proto.rs"));
}
