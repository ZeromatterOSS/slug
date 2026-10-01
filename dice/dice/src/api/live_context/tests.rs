/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::hash::Hash;
use std::hash::Hasher;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;

use crate::DetectCycles;
use crate::Dice;
use crate::DiceLiveContextId;
use crate::InjectedKey;

fn hash(id: DiceLiveContextId) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

#[tokio::test]
async fn clones_and_simultaneous_transactions_share_live_context() {
    let dice = Dice::builder().build(DetectCycles::Disabled);
    let first = dice.updater().commit().await;
    let clone = first.dupe();
    let simultaneous = dice.updater().commit().await;
    let id = first.live_context_id();
    assert_eq!(id, clone.live_context_id());
    assert_eq!(id, simultaneous.live_context_id());
    assert_eq!(hash(id), hash(simultaneous.live_context_id()));
    assert_eq!(id, id.dupe());
}

#[derive(Allocative, Clone, Debug, Display, Eq, PartialEq, Hash)]
struct Input;

impl InjectedKey for Input {
    type Value = usize;

    fn equality(x: &usize, y: &usize) -> bool {
        x == y
    }
}

#[tokio::test]
async fn semantic_versions_and_foreign_engines_have_distinct_live_contexts() {
    let dice = Dice::builder().build(DetectCycles::Disabled);
    let mut updater = dice.updater();
    updater.changed_to([(Input, 1)]).unwrap();
    let mut first = updater.commit().await;
    assert_eq!(first.compute(&Input).await.unwrap(), 1);
    let mut updater = dice.updater();
    updater.changed_to([(Input, 2)]).unwrap();
    let mut changed = updater.commit().await;
    assert_eq!(changed.compute(&Input).await.unwrap(), 2);
    assert_ne!(first.version(), changed.version());
    assert_ne!(first.live_context_id(), changed.live_context_id());

    let foreign = Dice::builder().build(DetectCycles::Disabled);
    let mut updater = foreign.updater();
    updater.changed_to([(Input, 1)]).unwrap();
    let other = updater.commit().await;
    assert_eq!(first.version(), other.version());
    assert_ne!(first.live_context_id(), other.live_context_id());
}

#[tokio::test]
async fn copied_token_does_not_keep_live_context_alive() {
    let dice = Dice::builder().build(DetectCycles::Disabled);
    let first = dice.updater().commit().await;
    let version = first.version();
    let retained_token = first.live_context_id();
    drop(first);
    let reopened = dice.updater().commit().await;
    assert_eq!(version, reopened.version());
    assert_ne!(retained_token, reopened.live_context_id());
}
