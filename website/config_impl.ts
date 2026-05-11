/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

import { themes } from 'prism-react-renderer';
import { fbContent } from 'docusaurus-plugin-internaldocs-fb/internal';
import type { ThemeConfig as ClassicPresetConfig, Options as ClassicPresetOptions } from '@docusaurus/preset-classic';
import type { DocusaurusConfig } from '@docusaurus/types';

import { postProcessItems } from './sidebars.js';
import { redirects } from './redirects';

const lightCodeTheme = themes.github;
const darkCodeTheme = themes.dracula;

const presetOptions: ClassicPresetOptions = ({
  docs: {
    path: '../docs',
    sidebarPath: require.resolve('./sidebars_generated.ts'),
    async sidebarItemsGenerator({ defaultSidebarItemsGenerator, ...args }) {
      const items = await defaultSidebarItemsGenerator({
        ...args
      });
      return postProcessItems(items);
    },
  },
  theme: {
    customCss: require.resolve('./src/css/custom.css'),
  },
  internSearch: true,
  staticDocsProject: 'kuro',
});

const themeConfig: ClassicPresetConfig = ({
  docs: {
    sidebar: {
      hideable: true,
    },
  },
  navbar: {
    title: 'Kuro',
    logo: {
      alt: 'Kuro Logo',
      src: 'img/logo.svg',
    },
    items: [
      {
        type: 'doc',
        docId: 'index',
        position: 'left',
        label: 'Docs',
      },
      {
        to: '/docs/api',
        position: 'left',
        label: 'API',
        activeBaseRegex: '/docs/api',
      },
      {
        to: '/docs/prelude/rules',
        position: 'left',
        label: 'Rules',
        activeBasePath: '/docs/prelude',
      },
      {
        href: fbContent({
          internal: 'https://github.com/ZeromatterOSS/kuro',
          external: 'https://github.com/ZeromatterOSS/kuro',
        }),
        // @ts-ignore : The type signature for `fbContent` incorrectly claims it might return a `[]`
        label: 'GitHub',
        position: 'right',
      },
    ],
  },
  footer: {
    style: 'dark',
    links: [
      {
        title: 'Docs',
        items: [
          {
            label: 'User guide',
            to: '/docs',
          },
        ],
      },
      {
        title: 'Community',
        items: [
          {
            label: 'GitHub issues',
            href: 'https://github.com/ZeromatterOSS/kuro/issues',
          },
        ],
      },
      {
        title: 'More',
        items: [
          {
            label: 'Code',
            href: fbContent({
              internal: 'https://github.com/ZeromatterOSS/kuro',
              external: 'https://github.com/ZeromatterOSS/kuro',
            }),
          },
          {
            label: 'Zeromatter',
            href: 'https://zeromatter.com',
          },
          {
            label: 'Buck2 upstream',
            href: 'https://github.com/facebook/buck2',
          },
        ],
      },
    ],
    copyright: `Copyright © ${new Date().getFullYear()} Zeromatter Inc. Portions derived from Buck2 by Meta Platforms, Inc. Built with Docusaurus.`,
  },
  prism: {
    additionalLanguages: ['bash', 'powershell', 'cpp', 'ini', 'mermaid'],
    theme: lightCodeTheme,
    darkTheme: darkCodeTheme,
  },
  algolia: fbContent({
    internal: undefined,
    external: {
      appId: '9RT0EWXQO8',
      apiKey: 'cf8a08e681e1e1d8a73a08d3f13948c7',
      indexName: 'kuro',
    }
  }),
});

const config: DocusaurusConfig = ({
  title: 'Kuro',
  // Kuro does not currently have a public docs website. This local URL avoids
  // advertising a nonexistent domain in generated metadata.
  url: 'http://localhost',
  baseUrl: '/',
  onBrokenLinks: 'throw',
  trailingSlash: true,
  onBrokenMarkdownLinks: 'warn',
  favicon: 'img/logo.png',
  organizationName: 'ZeromatterOSS',
  projectName: 'kuro',

  presets: [
    [
      require.resolve('docusaurus-plugin-internaldocs-fb/docusaurus-preset'),
      presetOptions,
    ],
  ],

  plugins: [
    [
      '@docusaurus/plugin-google-gtag',
      {
        trackingID: 'G-GEGGHE39PE',
        anonymizeIP: true,
      },
    ],
    [
      '@docusaurus/plugin-client-redirects',
      {
        redirects: redirects,
      },
    ],
  ],

  themeConfig,

  // @ts-ignore : Fields of this are not declared as optional, but they are
  markdown: ({
    // Use mdx for `.mdx` files and commonmark for `.md` files
    format: 'mdx',
    mermaid: true,
  }),
  themes: ['@docusaurus/theme-mermaid'],
});

module.exports = {
  config: config,
};
