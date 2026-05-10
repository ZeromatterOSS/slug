/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use kuro_core::cells::cell_path::CellPathRef;
use kuro_core::package::PackageLabel;
use kuro_events::dispatch::async_record_root_spans;
use kuro_events::span::SpanId;
use kuro_util::time_span::TimeSpan;
use smallvec::SmallVec;

use crate::package_listing::interpreter::InterpreterPackageListingResolver;
use crate::package_listing::listing::PackageListing;
use crate::package_listing::resolver::PackageListingResolver;

static PACKAGE_LISTING_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static PACKAGE_LISTING_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static PACKAGE_LISTING_MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);

fn record_max_active(max: &AtomicUsize, active: usize) {
    let mut current = max.load(Ordering::Relaxed);
    while active > current {
        match max.compare_exchange_weak(current, active, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

#[derive(
    Clone,
    Dupe,
    derive_more::Display,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Allocative
)]
pub struct PackageListingKey(pub PackageLabel);

pub struct PackageListingKeyActivationData {
    pub time_span: TimeSpan,
    pub spans: SmallVec<[SpanId; 1]>,
}

#[async_trait]
impl Key for PackageListingKey {
    type Value = kuro_error::Result<PackageListing>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let now = TimeSpan::start_now();
        let memory_checkpoints = kuro_util::memory_checkpoint::enabled();
        let active = PACKAGE_LISTING_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
        record_max_active(&PACKAGE_LISTING_MAX_ACTIVE, active);

        let (result, spans) = async_record_root_spans(
            InterpreterPackageListingResolver::new(ctx).resolve(self.0.dupe()),
        )
        .await;
        let active = PACKAGE_LISTING_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
        let completed = PACKAGE_LISTING_COMPLETED.fetch_add(1, Ordering::Relaxed) + 1;

        if memory_checkpoints {
            let (files, dirs, subpackages, path_bytes, ok) = match &result {
                Ok(listing) => (
                    listing.file_count(),
                    listing.directory_count(),
                    listing.subpackage_count(),
                    listing.approximate_path_bytes(),
                    1,
                ),
                Err(_) => (0, 0, 0, 0, 0),
            };
            kuro_util::memory_checkpoint::checkpoint(
                "package_listing_key",
                [
                    ("active", active),
                    ("completed", completed),
                    (
                        "max_active",
                        PACKAGE_LISTING_MAX_ACTIVE.load(Ordering::Relaxed),
                    ),
                    ("ok", ok),
                    ("files", files),
                    ("dirs", dirs),
                    ("subpackages", subpackages),
                    ("path_bytes", path_bytes),
                    (
                        "package_path_len",
                        self.0.as_cell_path().path().as_str().len(),
                    ),
                ],
            );
        }

        ctx.store_evaluation_data(PackageListingKeyActivationData {
            time_span: now.end_now(),
            spans,
        })?;

        result
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

pub struct DicePackageListingResolver<'compute, 'dice>(pub &'compute mut DiceComputations<'dice>);

#[async_trait]
impl PackageListingResolver for DicePackageListingResolver<'_, '_> {
    async fn resolve(&mut self, package: PackageLabel) -> kuro_error::Result<PackageListing> {
        self.0.compute(&PackageListingKey(package)).await?
    }

    async fn get_enclosing_package(
        &mut self,
        path: CellPathRef<'async_trait>,
    ) -> kuro_error::Result<PackageLabel> {
        InterpreterPackageListingResolver::new(self.0)
            .get_enclosing_package(path)
            .await
    }

    async fn get_enclosing_packages(
        &mut self,
        path: CellPathRef<'async_trait>,
        enclosing_violation_path: CellPathRef<'async_trait>,
    ) -> kuro_error::Result<Vec<PackageLabel>> {
        InterpreterPackageListingResolver::new(self.0)
            .get_enclosing_packages(path, enclosing_violation_path)
            .await
    }
}

impl DicePackageListingResolver<'_, '_> {
    pub async fn resolve_package_listing(
        &mut self,
        package: PackageLabel,
    ) -> kuro_error::Result<PackageListing> {
        self.resolve(package).await.map_err(kuro_error::Error::from)
    }
}
