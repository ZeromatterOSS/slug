---
id: labels_and_nodes
title: Understanding Labels and Nodes in Slug
---

import useBaseUrl from '@docusaurus/useBaseUrl';

Slug's labels and nodes are fundamental components that work together to
represent and track build targets in the build graph. Understanding how these
different types of labels and nodes relate to each other is essential not only
for writing BXL but also for working effectively with Slug's architecture.

## Overview

Slug uses several types of labels and nodes, each serving a specific purpose:

|              | target label                                                      | providers label                                                                                                     | node                                                              |
| ------------ | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| unconfigured | [TargetLabel](../../../api/build/TargetLabel)                     | [ProvidersLabel](../../../api/build/ProvidersLabel)                                                                 | [UnconfiguredTargetNode](../../../api/bxl/UnconfiguredTargetNode) |
| configured   | [ConfiguredTargetLabel](../../../api/build/ConfiguredTargetLabel) | [Label](../../../api/build/Label) (same as [ConfiguredProvidersLabel](../../../api/build/ConfiguredProvidersLabel)) | [ConfiguredTargetNode](../../../api/bxl/ConfiguredTargetNode)     |

**Note:** As part of our ongoing improvements, we are migrating to more explicit
type names. TargetLabel and ProvidersLabel will be renamed to include the
`Unconfigured` prefix for consistency.

The following diagram illustrates the relationships between these components:

<img src={useBaseUrl('/img/target_node_label_relationship.png')}
alt='justifyContent'/>

## Key Distinctions

### Configured vs Unconfigured

In the targets build graph, Slug operates with two main perspectives on build
targets: unconfigured and configured. You can refer
[execution model](../../../developers/architecture/slug/#execution-model) to
see these two phase in a slug build.

**Unconfigured** components are configuration independent representations. Think
of them as the blueprint of your targets. For example, `//slug:slug` is the
representation of `slug`'s unconfigured target label.

**Configured** components, on the other hand, include all the platform-specific
details and other configurations needed for actual building. They have the
necessary information about how to build it for a specific platform or
configuration. For example, `//slug:slug (cfg:linux-x86_64-xxxxxx)` is the
representation of `slug`'s configured target label.

### Labels vs Nodes

**Labels** are identifiers that uniquely reference targets in your build graph.
They're like addresses that tell Slug which target you're talking about. For
example, `//slug:slug` is an unconfigured label that points to a specific
target.

**Nodes** contain the actual information about targets. They hold the data about
what a target is, what it depends on, what attributes it has, etc.

### Target Labels vs Provider Labels

**Target labels** (both configured and unconfigured) identify complete build
targets. For example, `//slug:slug` refers to an entire target.

**Provider labels** (both configured and unconfigured) represents a specific
part of a target. For example, `//slug:slug[llvm_ir]` represents `slug`
target's `llvm_ir` sub-target

## Label and Nodes Conversion

This diagram shows how different components transform to each other using api

<img src={useBaseUrl('/img/node_label_conversion.png')} alt='justifyContent'/>
