import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    "intro",
    {
      type: "category",
      label: "Getting Started",
      collapsed: false,
      items: ["installation", "getting-started", "configuration"],
    },
    {
      type: "category",
      label: "Using Codex",
      collapsed: false,
      items: ["libraries", "formats", "users", "opds"],
    },
    {
      type: "category",
      label: "Deployment",
      items: ["deployment", "troubleshooting"],
    },
    {
      type: "category",
      label: "Reference",
      items: ["api"],
    },
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
};

export default sidebars;
