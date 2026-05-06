import { beforeEach, describe, expect, it } from "vitest";
import { useReleaseAnnouncementsStore } from "./releaseAnnouncementsStore";

describe("releaseAnnouncementsStore", () => {
  beforeEach(() => {
    useReleaseAnnouncementsStore.getState().reset();
  });

  it("bump increments and reset clears the unseen counter", () => {
    const store = useReleaseAnnouncementsStore.getState();
    store.bump();
    store.bump();
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(2);
    store.reset();
    expect(useReleaseAnnouncementsStore.getState().unseenCount).toBe(0);
  });
});
