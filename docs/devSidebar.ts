import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const devSidebar: SidebarsConfig = {
  devSidebar: [
    "intro",
    {
      type: "category",
      label: "Plugins",
      collapsed: false,
      link: { type: "doc", id: "plugins/overview" },
      items: [
        "plugins/writing-plugins",
        "plugins/protocol",
        "plugins/sdk",
      ],
    },
    {
      type: "category",
      label: "Contributing",
      collapsed: false,
      items: [
        "contributing/development",
        "contributing/architecture",
        "contributing/migrations",
      ],
    },
  ],
};

export default devSidebar;
