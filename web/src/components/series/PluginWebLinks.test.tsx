import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WebLinkProviderDto } from "@/api/plugins";
import { pluginsApi } from "@/api/plugins";
import { renderWithProviders } from "@/test/utils";
import { buildWebLinkUrl, PluginWebLinks } from "./PluginWebLinks";

vi.mock("@/api/plugins", async (importOriginal) => {
  const original = await importOriginal<typeof import("@/api/plugins")>();
  return {
    ...original,
    pluginsApi: {
      ...original.pluginsApi,
      getWebLinks: vi.fn(),
    },
  };
});

const tsundoku: WebLinkProviderDto = {
  pluginName: "release-tsundoku",
  displayName: "Tsundoku",
  searchUrlTemplate: "https://tsundoku.example.com/search?q={title}",
  seriesLinks: [
    {
      source: "mangabaka",
      urlTemplate:
        "https://tsundoku.example.com/series/lookup?source=mangabaka&id={externalId}",
    },
    {
      source: "myanimelist",
      urlTemplate:
        "https://tsundoku.example.com/series/lookup?source=mal&id={externalId}",
    },
  ],
};

describe("buildWebLinkUrl", () => {
  it("prefers the first declared series link the series has an ID for", () => {
    const url = buildWebLinkUrl(tsundoku, "Berserk", [
      { source: "api:myanimelist", externalId: "22" },
      { source: "api:mangabaka", externalId: "777" },
    ]);
    // mangabaka is declared first, so it wins even though the series also
    // has a myanimelist ID listed earlier.
    expect(url).toBe(
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
  });

  it("falls through declaration order to the next matching source", () => {
    const url = buildWebLinkUrl(tsundoku, "Berserk", [
      { source: "api:myanimelist", externalId: "22" },
    ]);
    expect(url).toBe(
      "https://tsundoku.example.com/series/lookup?source=mal&id=22",
    );
  });

  it("matches stored sources regardless of api:/plugin: namespace and case", () => {
    expect(
      buildWebLinkUrl(tsundoku, "Berserk", [
        { source: "plugin:MangaBaka", externalId: "777" },
      ]),
    ).toBe(
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
  });

  it("falls back to the search template with the URL-encoded title", () => {
    const url = buildWebLinkUrl(tsundoku, "Frieren: Beyond Journey's End", []);
    expect(url).toBe(
      "https://tsundoku.example.com/search?q=Frieren%3A%20Beyond%20Journey's%20End",
    );
  });

  it("URL-encodes external IDs", () => {
    const provider: WebLinkProviderDto = {
      ...tsundoku,
      seriesLinks: [
        {
          source: "mangaupdates",
          urlTemplate: "https://site.example.com/mu/{externalId}",
        },
      ],
    };
    const url = buildWebLinkUrl(provider, "X", [
      { source: "api:mangaupdates", externalId: "a/b c" },
    ]);
    expect(url).toBe("https://site.example.com/mu/a%2Fb%20c");
  });

  it("ignores empty external-ID values", () => {
    const url = buildWebLinkUrl(tsundoku, "Berserk", [
      { source: "api:mangabaka", externalId: "" },
    ]);
    expect(url).toBe("https://tsundoku.example.com/search?q=Berserk");
  });
});

describe("PluginWebLinks", () => {
  it("renders one button per provider with the direct link", async () => {
    vi.mocked(pluginsApi.getWebLinks).mockResolvedValue({
      providers: [tsundoku],
    });

    renderWithProviders(
      <PluginWebLinks
        title="Berserk"
        externalIds={[{ source: "api:mangabaka", externalId: "777" }]}
      />,
    );

    const link = await screen.findByRole("link", { name: /tsundoku/i });
    expect(link).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("renders nothing when there are no providers", async () => {
    vi.mocked(pluginsApi.getWebLinks).mockResolvedValue({ providers: [] });

    renderWithProviders(<PluginWebLinks title="Berserk" externalIds={[]} />);

    await waitFor(() => {
      expect(pluginsApi.getWebLinks).toHaveBeenCalled();
    });
    expect(screen.queryByRole("link")).toBeNull();
  });
});
