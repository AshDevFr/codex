import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";
import apiSidebar from "./apiSidebar";

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    "intro",
    "showcase",
    {
      type: "category",
      label: "Getting Started",
      collapsed: false,
      items: ["getting-started", "configuration"],
    },
    {
      type: "category",
      label: "Libraries & Scanning",
      items: [
        "libraries",
        "library-jobs",
        {
          type: "category",
          label: "Scanning Strategies",
          link: { type: "doc", id: "scanning-strategies/index" },
          items: [
            "scanning-strategies/series-strategies",
            "scanning-strategies/book-strategies",
            "scanning-strategies/examples",
          ],
        },
        {
          type: "category",
          label: "Preprocessing Rules",
          link: { type: "doc", id: "preprocessing-rules" },
          items: ["examples/preprocessing-examples"],
        },
        "formats",
      ],
    },
    {
      type: "category",
      label: "Metadata",
      items: [
        "book-metadata",
        "series-metadata",
        "series-management",
        "custom-metadata",
        "release-tracking",
      ],
    },
    {
      type: "category",
      label: "Reading & Lists",
      items: [
        "reader-settings",
        "offline-reading",
        "filtering",
        "collections-readlists",
      ],
    },
    {
      type: "category",
      label: "Users & Permissions",
      link: { type: "doc", id: "users/index" },
      items: [
        "users/user-management",
        "users/permissions",
        "users/api-keys",
        "users/authentication",
        "users/oidc",
        "users/sharing-tags",
      ],
    },
    {
      type: "category",
      label: "Integrations & Apps",
      items: [
        {
          type: "category",
          label: "Plugins",
          link: { type: "doc", id: "plugins/index" },
          items: [
            "plugins/open-library",
            "plugins/mangabaka",
            "plugins/anilist-sync",
            "plugins/anilist-recommendations",
            "plugins/release-mangaupdates",
            "plugins/release-nyaa",
          ],
        },
        "opds",
        "third-party-apps",
        "tsundoku",
        "exports",
      ],
    },
    {
      type: "category",
      label: "Backup & Migration",
      items: [
        "backup-migration/export-import-copy",
        "backup-migration/migrate-postgres",
      ],
    },
    {
      type: "category",
      label: "Deployment",
      link: { type: "doc", id: "deployment/index" },
      items: [
        "deployment/docker",
        "deployment/kubernetes",
        "deployment/systemd",
        "deployment/reverse-proxy",
        "deployment/database",
        "deployment/performance",
        "deployment/operations",
      ],
    },
    "troubleshooting",
  ],
  apiSidebar,
};

export default sidebars;
