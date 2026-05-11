/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

import React from 'react';
import clsx from 'clsx';
import styles from './HomepageFeatures.module.css';

const FeatureList = [
  {
    icon: '🚀',
    title: 'Fast',
    description: (
      <>
        Kuro targets Bazel 9 compatibility while reusing Buck2's fast Rust
        internals. It is experimental software for exploring build-system design
        and agentic programming.
      </>
    ),
  },
  {
    icon: '🎯',
    title: 'Reliable',
    description: (
      <>
        Kuro aims to preserve Bazel-compatible hermeticity and dependency
        semantics. Missing dependencies should be surfaced as errors rather than
        hidden by local machine state.
      </>
    ),
  },
  {
    icon: '🧩',
    title: 'Extensible',
    description: (
      <>
        Kuro builds on Starlark, DICE, Superconsole, and remote execution
        architecture inherited from Buck2 while evolving as a separate
        Zeromatter Inc project.
      </>
    ),
  },
];

function Feature({icon, title, description}) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center" style={{fontSize: '400%'}}>
        {icon}
      </div>
      <div className="text--center padding-horiz--md">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures() {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
