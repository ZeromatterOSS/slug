/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use slug_common::cas_digest::CasDigestConfig;
use slug_common::io::IoProvider;
use slug_common::io::fs::FsIoProvider;
use slug_common::io::trace::TracingIoProvider;
use slug_common::legacy_configs::configs::LegacyBuckConfig;
use slug_core::fs::project::ProjectRoot;

pub async fn create_io_provider(
    fb: fbinit::FacebookInit,
    project_fs: ProjectRoot,
    root_config: &LegacyBuckConfig,
    cas_digest_config: CasDigestConfig,
    trace_io: bool,
    _use_eden_thrift_read: bool,
) -> slug_error::Result<Arc<dyn IoProvider>> {
    #[cfg(fbcode_build)]
    {
        if false {
            if let Some(eden) = slug_eden::io_provider::EdenIoProvider::new(
                fb,
                &project_fs,
                cas_digest_config,
                _use_eden_thrift_read,
            )
            .await?
            {
                return if trace_io {
                    Ok(Arc::new(TracingIoProvider::new(Box::new(eden))))
                } else {
                    Ok(Arc::new(eden))
                };
            }
        }
    }

    let _allow_unused = (fb, root_config);

    if trace_io {
        Ok(Arc::new(TracingIoProvider::new(Box::new(
            FsIoProvider::new(project_fs, cas_digest_config),
        ))))
    } else {
        Ok(Arc::new(FsIoProvider::new(project_fs, cas_digest_config)))
    }
}
