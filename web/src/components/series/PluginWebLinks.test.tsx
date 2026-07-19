import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { WebLinkProviderDto } from "@/api/plugins";
import { pluginsApi } from "@/api/plugins";
import { renderWithProviders } from "@/test/utils";
import { buildWebLinkOptions, PluginWebLinks } from "./PluginWebLinks";

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

describe("buildWebLinkOptions", () => {
  it("orders options as declared direct links first, then search last", () => {
    const options = buildWebLinkOptions(tsundoku, "Berserk", [
      { source: "api:myanimelist", externalId: "22" },
      { source: "api:mangabaka", externalId: "777" },
    ]);
    expect(options.map((option) => option.key)).toEqual([
      "mangabaka",
      "myanimelist",
      "search",
    ]);
    expect(options[0].url).toBe(
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
    expect(options[1].url).toBe(
      "https://tsundoku.example.com/series/lookup?source=mal&id=22",
    );
    expect(options[2].url).toBe(
      "https://tsundoku.example.com/search?q=Berserk",
    );
  });

  it("uses human-readable source labels and Search for the fallback", () => {
    const options = buildWebLinkOptions(tsundoku, "Berserk", [
      { source: "api:mangabaka", externalId: "777" },
    ]);
    expect(options.map((option) => option.label)).toEqual([
      "MangaBaka",
      "Search",
    ]);
  });

  it("matches stored sources regardless of api:/plugin: namespace and case", () => {
    const options = buildWebLinkOptions(tsundoku, "Berserk", [
      { source: "plugin:MangaBaka", externalId: "777" },
    ]);
    expect(options[0].url).toBe(
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
  });

  it("URL-encodes titles and external IDs", () => {
    const provider: WebLinkProviderDto = {
      ...tsundoku,
      seriesLinks: [
        {
          source: "mangaupdates",
          urlTemplate: "https://site.example.com/mu/{externalId}",
        },
      ],
    };
    const options = buildWebLinkOptions(
      provider,
      "Frieren: Beyond Journey's End",
      [{ source: "api:mangaupdates", externalId: "a/b c" }],
    );
    expect(options[0].url).toBe("https://site.example.com/mu/a%2Fb%20c");
    expect(options[1].url).toBe(
      "https://tsundoku.example.com/search?q=Frieren%3A%20Beyond%20Journey's%20End",
    );
  });

  it("ignores empty external-ID values, leaving only search", () => {
    const options = buildWebLinkOptions(tsundoku, "Berserk", [
      { source: "api:mangabaka", externalId: "" },
    ]);
    expect(options.map((option) => option.key)).toEqual(["search"]);
  });
});

describe("PluginWebLinks", () => {
  it("renders the primary button with the best direct link and a dropdown with the rest", async () => {
    const user = userEvent.setup();
    vi.mocked(pluginsApi.getWebLinks).mockResolvedValue({
      providers: [tsundoku],
    });

    renderWithProviders(
      <PluginWebLinks
        title="Berserk"
        externalIds={[
          { source: "api:mangabaka", externalId: "777" },
          { source: "api:myanimelist", externalId: "22" },
        ]}
      />,
    );

    const primary = await screen.findByRole("link", { name: /tsundoku/i });
    expect(primary).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
    expect(primary).toHaveAttribute("target", "_blank");
    expect(primary).toHaveAttribute("rel", "noopener noreferrer");

    await user.click(
      screen.getByRole("button", { name: "More Tsundoku links" }),
    );
    // Menu items are asserted via text, not role: in jsdom the Mantine menu
    // transition never fires `transitionend`, so the dropdown keeps
    // `display: none` and role queries (which filter hidden elements) can't
    // see the items. Text queries don't filter by visibility; this matches
    // how other menu tests in the codebase (e.g. MediaCard) assert items.
    // The dropdown lists every option, including the primary one.
    const mangabakaItem = (await screen.findByText("MangaBaka")).closest("a");
    expect(mangabakaItem).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/series/lookup?source=mangabaka&id=777",
    );
    const malItem = (await screen.findByText("MyAnimeList")).closest("a");
    expect(malItem).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/series/lookup?source=mal&id=22",
    );
    const searchItem = (await screen.findByText("Search")).closest("a");
    expect(searchItem).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/search?q=Berserk",
    );
    // Direct links are separated from the search fallback by a divider.
    expect(document.querySelector(".mantine-Menu-divider")).not.toBeNull();
  });

  it("renders a plain search button without a dropdown when nothing else matches", async () => {
    vi.mocked(pluginsApi.getWebLinks).mockResolvedValue({
      providers: [{ ...tsundoku, seriesLinks: [] }],
    });

    renderWithProviders(<PluginWebLinks title="Berserk" externalIds={[]} />);

    const primary = await screen.findByRole("link", { name: /tsundoku/i });
    expect(primary).toHaveAttribute(
      "href",
      "https://tsundoku.example.com/search?q=Berserk",
    );
    expect(
      screen.queryByRole("button", { name: /more tsundoku links/i }),
    ).toBeNull();
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
