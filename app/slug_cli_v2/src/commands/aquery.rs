/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_commands_v2::CommandKind;
use slug_commands_v2::aquery::AqueryRequest;

pub fn run(argv: Vec<String>) -> i32 {
    let result = AqueryRequest::parse(&argv).map(|request| request.placeholder_error());
    super::emit_result(CommandKind::Aquery, argv, result)
}
