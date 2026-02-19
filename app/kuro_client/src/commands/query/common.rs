/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use kuro_cli_proto::QueryOutputFormat;
use kuro_client_ctx::query_args::CommonAttributeArgs;
use kuro_query_parser::placeholder::QUERY_PERCENT_SS_PLACEHOLDER;

#[derive(
    Debug,
    Clone,
    Dupe,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize
)]
#[clap(rename_all = "snake_case")]
enum QueryOutputFormatArg {
    Dot,
    Json,
    DotCompact,
    Starlark,
    Html,
}

/// Bazel-compatible --output flag values.
/// Maps to the subset of Bazel query output formats that kuro supports.
#[derive(
    Debug,
    Clone,
    Dupe,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize
)]
#[clap(rename_all = "snake_case")]
enum BazelQueryOutputArg {
    /// One label per line (default)
    Label,
    /// Label followed by rule kind
    LabelKind,
    /// JSON format
    Json,
    /// Starlark-like BUILD file representation
    Build,
    /// Graphviz dot graph format
    Graph,
}

/// Args common to all the query commands
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
#[clap(group = clap::ArgGroup::new("output_attribute_flags").multiple(false))]
pub(crate) struct CommonQueryOptions {
    #[clap(name = "QUERY", help = "the query to evaluate")]
    query: String,

    #[clap(flatten)]
    pub attributes: CommonAttributeArgs,

    #[clap(long, help = "Output in JSON format")]
    json: bool,

    #[clap(long, help = "Output in Graphviz Dot format")]
    dot: bool,

    #[clap(long, help = "Output in a more compact format than Graphviz Dot")]
    dot_compact: bool,

    #[clap(
        long,
        ignore_case = true,
        help = "Output format (default: list).",
        long_help = "Output format (default: list). \n
           dot -  dot graph format. \n
           dot_compact - compact alternative to dot format. \n
           json - JSON format. \n
           starlark - targets are printed like starlark code that would produce them.
           html - html file containing interactive target graph.
         ",
        value_name = "dot|dot_compact|json|starlark|html",
        value_enum
    )]
    output_format: Option<QueryOutputFormatArg>,

    /// Bazel-compatible output format flag.
    /// Supported values: label (default), label_kind, json, build, graph.
    #[clap(
        long = "output",
        ignore_case = true,
        help = "Bazel-compatible output format (label, label_kind, json, build, graph).",
        value_name = "label|label_kind|json|build|graph",
        value_enum
    )]
    bazel_output: Option<BazelQueryOutputArg>,

    #[clap(
        name = "QUERY_ARGS",
        help = "list of literals for a multi-query (one containing `%s` or `%Ss`)"
    )]
    query_args: Vec<String>,
}

impl CommonQueryOptions {
    fn args_as_set(args: &[String]) -> String {
        let mut s = "set(".to_owned();
        for (i, v) in args.iter().enumerate() {
            if i != 0 {
                s += " ";
            }
            s += "'";
            s += v;
            s += "'";
        }
        s += ")";
        s
    }

    pub fn output_format(&self) -> QueryOutputFormat {
        // Check --output-format first, then --output (Bazel-compatible alias), then individual flags
        match self.output_format {
            Some(QueryOutputFormatArg::Json) => return QueryOutputFormat::Json,
            Some(QueryOutputFormatArg::Dot) => return QueryOutputFormat::Dot,
            Some(QueryOutputFormatArg::DotCompact) => return QueryOutputFormat::DotCompact,
            Some(QueryOutputFormatArg::Starlark) => return QueryOutputFormat::Starlark,
            Some(QueryOutputFormatArg::Html) => return QueryOutputFormat::Html,
            None => {}
        }
        // Map Bazel --output flag to our format
        match self.bazel_output {
            Some(BazelQueryOutputArg::Json) => return QueryOutputFormat::Json,
            Some(BazelQueryOutputArg::Build) => return QueryOutputFormat::Starlark,
            Some(BazelQueryOutputArg::Graph) => return QueryOutputFormat::Dot,
            // label and label_kind both use default list format
            Some(BazelQueryOutputArg::Label) | Some(BazelQueryOutputArg::LabelKind) | None => {}
        }
        if self.json {
            QueryOutputFormat::Json
        } else if self.dot {
            QueryOutputFormat::Dot
        } else if self.dot_compact {
            QueryOutputFormat::DotCompact
        } else {
            QueryOutputFormat::Default
        }
    }

    pub fn get_query(&self) -> (String, Vec<String>) {
        if self.query.contains(QUERY_PERCENT_SS_PLACEHOLDER) {
            let replacement = Self::args_as_set(&self.query_args);
            (
                self.query
                    .replace(QUERY_PERCENT_SS_PLACEHOLDER, &replacement),
                vec![],
            )
        } else {
            (self.query.clone(), self.query_args.clone())
        }
    }
}
