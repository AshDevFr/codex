import { beforeEach, describe, expect, it } from "vitest";
import { useReleaseAnnouncementsStore } from "./releaseAnnouncementsStore";

describe("releaseAnnouncementsStore", () => {
  beforeEach(() => {
    const store = useReleaseAnnouncementsStore.getState();
    store.reset();
    store.setAllowedLanguages([]);
    store.setAllowedPlugins([]);
    // Clear any leftover muted series from a prior test.
    const muted = Array.from(store.mutedSeriesIds);
    for (const id of muted) {
      store.toggleMute(id);
    }
  });

  it("bump increments and reset clears the unseen counter", () => {
    const store = useReleaseAnnouncementsStore.getState();
    store.bump();
    store.bump();
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(2);
    store.reset();
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(0);
  });

  it("shouldNotify lets everything through when filters are empty", () => {
    const { shouldNotify } = useReleaseAnnouncementsStore.getState();
    expect(
      shouldNotify({
        seriesId: "s1",
        pluginId: "release-nyaa",
        language: "en",
      }),
    ).toBe(true);
  });

  it("shouldNotify blocks muted series", () => {
    useReleaseAnnouncementsStore.getState().toggleMute("muted-series");
    const { shouldNotify } = useReleaseAnnouncementsStore.getState();
    expect(
      shouldNotify({
        seriesId: "muted-series",
        pluginId: "release-nyaa",
        language: "en",
      }),
    ).toBe(false);
  });

  it("shouldNotify enforces language allowlist (case-insensitive)", () => {
    useReleaseAnnouncementsStore.getState().setAllowedLanguages(["EN"]);
    const { shouldNotify } = useReleaseAnnouncementsStore.getState();
    expect(
      shouldNotify({ seriesId: "s1", pluginId: "p", language: "en" }),
    ).toBe(true);
    expect(
      shouldNotify({ seriesId: "s1", pluginId: "p", language: "es" }),
    ).toBe(false);
  });

  it("shouldNotify enforces plugin allowlist", () => {
    useReleaseAnnouncementsStore
      .getState()
      .setAllowedPlugins(["release-mangaupdates"]);
    const { shouldNotify } = useReleaseAnnouncementsStore.getState();
    expect(
      shouldNotify({
        seriesId: "s1",
        pluginId: "release-mangaupdates",
        language: "en",
      }),
    ).toBe(true);
    expect(
      shouldNotify({
        seriesId: "s1",
        pluginId: "release-nyaa",
        language: "en",
      }),
    ).toBe(false);
  });

  it("toggleMute is reversible", () => {
    const store = useReleaseAnnouncementsStore.getState();
    store.toggleMute("series-x");
    expect(
      useReleaseAnnouncementsStore.getState().mutedSeriesIds.has("series-x"),
    ).toBe(true);
    useReleaseAnnouncementsStore.getState().toggleMute("series-x");
    expect(
      useReleaseAnnouncementsStore.getState().mutedSeriesIds.has("series-x"),
    ).toBe(false);
  });
});
