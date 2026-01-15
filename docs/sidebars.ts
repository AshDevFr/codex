import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";
import apiSidebar from "./apiSidebar";

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    "intro",
    {
      type: "category",
      label: "Getting Started",
      collapsed: false,
      items: ["getting-started", "configuration"],
    },
    {
      type: "category",
      label: "Using Codex",
      collapsed: false,
      items: [
        "libraries",
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
        "formats",
        "filtering",
        "custom-metadata",
        {
          type: "category",
          label: "Users & Permissions",
          link: { type: "doc", id: "users/index" },
          items: [
            "users/user-management",
            "users/permissions",
            "users/api-keys",
            "users/authentication",
          ],
        },
        "opds",
        "reader-settings",
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
    {
      type: "category",
      label: "Development",
      items: [
        "development/development",
        "development/architecture",
        "development/migrations",
      ],
    },
  ],
  apiSidebar,
};

export default sidebars;
