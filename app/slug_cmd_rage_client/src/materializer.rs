/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::future::Shared;
use slug_cli_proto::ClientContext;
use slug_client_ctx::command_outcome::CommandOutcome;
use slug_client_ctx::daemon::client::connect::BootstrapBuckdClient;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::events_ctx::PartialResultCtx;
use slug_client_ctx::events_ctx::PartialResultHandler;
use slug_client_ctx::subscribers::subscriber::EventSubscriber;
use slug_cmd_audit_client::AuditCommand;
use slug_cmd_audit_client::deferred_materializer::DeferredMaterializerCommand;
use slug_cmd_audit_client::deferred_materializer::DeferredMaterializerSubcommand;
use slug_common::manifold::ManifoldClient;
use slug_error::slug_error;

use crate::manifold::buf_to_manifold;
use crate::rage::MaterializerRageUploadData;

pub async fn upload_materializer_data(
    buckd: Shared<BoxFuture<'_, slug_error::Result<BootstrapBuckdClient>>>,
    client_context: &ClientContext,
    manifold: &ManifoldClient,
    manifold_id: &String,
    materializer_data: MaterializerRageUploadData,
) -> slug_error::Result<String> {
    let mut buckd = buckd.await?.to_connector();

    let mut events_ctx = EventsCtx::new(None, vec![Box::new(TracingSubscriber) as _]);

    let mut capture = CaptureStdout::new();

    let outcome = buckd
        .with_flushing()
        .audit(
            slug_cli_proto::GenericRequest {
                context: Some(client_context.clone()),
                serialized_opts: serde_json::to_string(&AuditCommand::DeferredMaterializer(
                    DeferredMaterializerCommand {
                        common_opts: Default::default(),
                        _target_cfg: Default::default(),
                        subcommand: match materializer_data {
                            MaterializerRageUploadData::State => {
                                DeferredMaterializerSubcommand::List
                            }
                            MaterializerRageUploadData::Fsck => {
                                DeferredMaterializerSubcommand::Fsck
                            }
                        },
                    },
                ))?,
            },
            &mut events_ctx,
            None,
            &mut capture,
        )
        .await?;

    match outcome {
        CommandOutcome::Success(..) => {}
        CommandOutcome::Failure(..) => {
            return Err(slug_error!(slug_error::ErrorTag::Tier0, "Command failed"));
        }
    }

    let manifold_filename = format!("flat/{manifold_id}_materializer_{materializer_data}");
    buf_to_manifold(manifold, &capture.buf, manifold_filename).await
}

/// Receive StdoutBytes, just capture them.
struct CaptureStdout {
    buf: Vec<u8>,
}

impl CaptureStdout {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
}

#[async_trait]
impl PartialResultHandler for CaptureStdout {
    type PartialResult = slug_cli_proto::StdoutBytes;

    async fn handle_partial_result(
        &mut self,
        _ctx: PartialResultCtx<'_>,
        partial_res: Self::PartialResult,
    ) -> slug_error::Result<()> {
        self.buf.extend(partial_res.data);
        Ok(())
    }
}

struct TracingSubscriber;

#[async_trait]
impl EventSubscriber for TracingSubscriber {
    async fn handle_tailer_stderr(&mut self, stderr: &str) -> slug_error::Result<()> {
        tracing::info!("{}", stderr);
        Ok(())
    }

    async fn handle_error(&mut self, error: &slug_error::Error) -> slug_error::Result<()> {
        tracing::info!("{:#}", error);
        Ok(())
    }

    async fn handle_command_result(
        &mut self,
        result: &slug_cli_proto::CommandResult,
    ) -> slug_error::Result<()> {
        tracing::info!("{:?}", result);
        Ok(())
    }
}
