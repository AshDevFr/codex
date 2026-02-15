import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import type * as OpenApiPlugin from 'docusaurus-plugin-openapi-docs';
import { themes as prismThemes } from 'prism-react-renderer';

// Read version from package.json
import packageJson from './package.json';
const appVersion = packageJson.version;

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Codex',
  tagline: 'A next-generation digital library server for comics, manga, and ebooks',
  favicon: 'img/codex-logo-color.svg',

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Set the production url of your site here
  url: 'https://codex.4sh.dev',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'codex', // Usually your GitHub org/user name.
  projectName: 'codex', // Usually your repo name.

  onBrokenLinks: 'throw',

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          // Enable "edit this page" links
          docItemComponent: '@theme/ApiItem',
        },
        blog: false,
        theme: {
          customCss: ['./src/css/custom.css', './src/css/openapi-highcontrast.css'],
        },
      } satisfies Preset.Options,
    ],
  ],

  plugins: [
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'dev',
        path: 'dev',
        routeBasePath: 'dev',
        sidebarPath: './devSidebar.ts',
      },
    ],
    [
      'docusaurus-plugin-openapi-docs',
      {
        id: 'api',
        docsPluginId: 'classic',
        config: {
          codex: {
            specPath: 'api/openapi.json',
            outputDir: 'docs/api',
            sidebarOptions: {
              groupPathsBy: 'tag',
              categoryLinkSource: 'tag',
            },
            showSchemas: true,
          } satisfies OpenApiPlugin.Options,
        },
      },
    ],
  ],

  themes: [
    'docusaurus-theme-openapi-docs',
    [
      require.resolve("@easyops-cn/docusaurus-search-local"),
      /** @type {import("@easyops-cn/docusaurus-search-local").PluginOptions} */
      ({
        // `hashed` is recommended as long-term-cache of index file is possible.
        hashed: true,

        language: ["en"],

        // Index both main docs and dev docs
        docsDir: ['docs', 'dev'],
        docsRouteBasePath: ['docs', 'dev'],

        // Exclude API docs from search indexing
        ignoreFiles: [/docs\/api\/.*/],

        // Customize the keyboard shortcut to focus search bar (default is "mod+k"):
        // searchBarShortcutKeymap: "s", // Use 'S' key
        // searchBarShortcutKeymap: "ctrl+shift+f", // Use Ctrl+Shift+F

        // If you're using `noIndex: true`, set `forceIgnoreNoIndex` to enable local index:
        // forceIgnoreNoIndex: true,
      }),
    ],
  ],

  themeConfig: {
    // Replace with your project's social card
    image: 'img/codex-social-card.jpg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Codex',
      logo: {
        alt: 'Codex Logo',
        src: 'img/codex-logo-color.svg',
      },
      items: [
        {
          type: 'html',
          position: 'left',
          value: `<span class="badge badge--primary" style="margin-left: 4px; font-size: 0.7rem; vertical-align: middle;">v${appVersion}</span>`,
        },
        {
          type: 'docSidebar',
          sidebarId: 'tutorialSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          to: '/dev/intro',
          position: 'left',
          label: 'Dev',
        },
        {
          to: '/docs/api/codex-api',
          position: 'left',
          label: 'API',
        },
        {
          href: 'https://codex.userjot.com',
          label: 'Feature Board',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {
              label: 'Introduction',
              to: '/docs/',
            },
            {
              label: 'Getting Started',
              to: '/docs/getting-started',
            },
            {
              label: 'API Reference',
              to: '/docs/api',
            },
          ],
        },
        {
          title: 'Developers',
          items: [
            {
              label: 'Plugin Development',
              to: '/dev/plugins/overview',
            },
            {
              label: 'Contributing',
              to: '/dev/contributing/development',
            },
          ],
        },
        {
          title: 'Project',
          items: [
            {
              label: 'Feature Board',
              href: 'https://codex.userjot.com',
            },
            {
              label: 'Documentation',
              href: 'https://codex.4sh.dev',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Codex. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
