/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::Error;

use slug_error::slug_error;
use winapi::shared::minwindef::BOOL;
use winapi::shared::minwindef::DWORD;
use winapi::shared::minwindef::FALSE;

pub(crate) fn result_bool(ret: BOOL) -> slug_error::Result<()> {
    if ret == FALSE {
        Err(slug_error!(
            slug_error::ErrorTag::Tier0,
            "{}",
            format!("{}", Error::last_os_error())
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn result_dword(ret: DWORD) -> slug_error::Result<()> {
    if ret == DWORD::MAX {
        Err(slug_error!(
            slug_error::ErrorTag::Tier0,
            "{}",
            format!("{}", Error::last_os_error())
        ))
    } else {
        Ok(())
    }
}
