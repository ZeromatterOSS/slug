/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use dice::DiceTransaction;
use kuro_cli_proto::new_generic::DocsResponse;
use kuro_cli_proto::new_generic::DocsStarlarkBuiltinsRequest;
use kuro_error::BuckErrorContext;
use kuro_error::internal_error;
use kuro_fs::fs_util;
use kuro_fs::paths::abs_path::AbsPathBuf;
use kuro_fs::paths::forward_rel_path::ForwardRelativePath;
use kuro_interpreter_for_build::interpreter::globals::register_analysis_natives;
use kuro_interpreter_for_build::interpreter::globals::register_bxl_natives;
use kuro_interpreter_for_build::interpreter::globals::register_load_natives;
use kuro_interpreter_for_build::interpreter::globals::starlark_library_extensions_for_kuro;
use kuro_server_ctx::ctx::ServerCommandContextTrait;
use starlark::docs::DocItem;
use starlark::docs::multipage::DocModuleInfo;
use starlark::docs::multipage::render_markdown_multipage;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;

pub(crate) fn write_docs_to_subdir(
    modules_infos: Vec<DocModuleInfo<'_>>,
    base_path: &str,
    linked_ty_mapper: Option<fn(&str, &str) -> String>,
    render_signature_at_bottom: bool,
) -> kuro_error::Result<()> {
    let base_path = AbsPathBuf::new(base_path)?;
    let mut docs: BTreeMap<_, _> =
        render_markdown_multipage(modules_infos, linked_ty_mapper, render_signature_at_bottom)
            .into_iter()
            .collect();
    while let Some((mut doc_path, rendered)) = docs.pop_first() {
        let mut path = base_path.clone();
        // Map:
        // - "" -> "index.md"
        // - "bxl" -> "bxl/index.md"
        // - "bxl/typename" -> "bxl/typename.md"
        if doc_path.is_empty()
            || docs
                .first_key_value()
                .is_some_and(|(k, _)| k.starts_with(&format!("{doc_path}/")))
        {
            path.push(
                ForwardRelativePath::new(&doc_path)
                    .internal_error("Doc paths should be forward relative")?
                    .as_path(),
            );
            path.push(ForwardRelativePath::new("index.md").unwrap().as_path());
        } else {
            doc_path.push_str(".md");
            path.push(
                ForwardRelativePath::new(&doc_path)
                    .internal_error("Doc paths should be forward relative")?
                    .as_path(),
            );
        }

        if let Some(parent) = path.parent() {
            fs_util::create_dir_all(parent)?;
        }
        // Since we just <Link> to the docs, we need to import the Link component at the top of the file
        let final_rendered_conent = format!("import Link from '@docusaurus/Link';\n\n{rendered}");
        fs_util::write(path, &final_rendered_conent)?;
    }

    Ok(())
}

pub(crate) async fn docs_starlark_builtins(
    _server_ctx: &dyn ServerCommandContextTrait,
    _dice_ctx: DiceTransaction,
    request: &DocsStarlarkBuiltinsRequest,
) -> kuro_error::Result<DocsResponse> {
    let starlark = Globals::extended_by(starlark_library_extensions_for_kuro()).documentation();

    let mut build = GlobalsBuilder::new()
        .with(register_load_natives)
        .with(register_analysis_natives)
        .build()
        .documentation();
    // Remove the Attr type from build docs: it has no methods and its name collides
    // with the `attr` namespace on case-insensitive filesystems (Windows/macOS).
    build.members.shift_remove("Attr");

    let mut bxl = GlobalsBuilder::new()
        .with(register_bxl_natives)
        .build()
        .documentation();

    let Some(DocItem::Module(bxl)) = bxl.members.shift_remove("bxl") else {
        return Err(internal_error!("bxl namespace should exist"));
    };

    let modules_infos = vec![
        DocModuleInfo {
            module: &starlark,
            name: "Starlark APIs".to_owned(),
            page_path: "starlark".to_owned(),
        },
        DocModuleInfo {
            module: &build,
            name: "Build APIs".to_owned(),
            page_path: "build".to_owned(),
        },
        DocModuleInfo {
            module: &bxl,
            name: "Bxl APIs".to_owned(),
            page_path: "bxl".to_owned(),
        },
    ];

    fn linked_ty_mapper(path: &str, type_name: &str) -> String {
        format!("<Link to=\"/docs/api/{path}\">{type_name}</Link>")
    }

    write_docs_to_subdir(modules_infos, &request.path, Some(linked_ty_mapper), false)?;

    Ok(DocsResponse { json_output: None })
}
