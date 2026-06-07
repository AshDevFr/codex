import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { RecommendationRequest, UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { provider, setRecorder } from "./index.js";
import { PayloadRecorder } from "./recorder.js";

afterEach(() => {
  setRecorder(null);
});

function seed(title: string): UserLibraryEntry {
  return {
    seriesId: `series-${title}`,
    title,
    alternateTitles: [],
    genres: [],
    tags: [],
    externalIds: [],
    booksRead: 1,
    booksOwned: 1,
  };
}

function request(overrides: Partial<RecommendationRequest> = {}): RecommendationRequest {
  return { library: [], excludeIds: [], ...overrides };
}

describe("get", () => {
  it("echoes each library seed into a fully-populated recommendation", async () => {
    const res = await provider.get(request({ library: [seed("Berserk"), seed("Vinland Saga")] }));

    expect(res.cached).toBe(false);
    expect(typeof res.generatedAt).toBe("string");
    expect(res.recommendations).toHaveLength(2);

    const first = res.recommendations[0];
    expect(first.basedOn).toEqual(["Berserk"]);
    expect(first.reason).toContain("Berserk");
    expect(first.externalId).toBe("echo-rec-1");
    // Fully-populated fields
    expect(first.score).toBeGreaterThan(0);
    expect(first.score).toBeLessThanOrEqual(1);
    expect(first.genres.length).toBeGreaterThan(0);
    expect(first.tags?.length).toBeGreaterThan(0);
    expect(first.status).toBe("ongoing");
    expect(first.rating).toBeDefined();
    expect(first.popularity).toBeDefined();
    expect(first.inLibrary).toBe(false);
  });

  it("returns generic recommendations when the library is empty", async () => {
    const res = await provider.get(request());
    expect(res.recommendations.length).toBeGreaterThan(0);
    expect(res.recommendations[0].basedOn[0]).toMatch(/^Echo Seed/);
  });

  it("respects limit and excludeIds", async () => {
    const library = [seed("A"), seed("B"), seed("C")];
    expect((await provider.get(request({ library, limit: 2 }))).recommendations).toHaveLength(2);

    const excluded = await provider.get(request({ library, excludeIds: ["echo-rec-1"] }));
    expect(excluded.recommendations.map((r) => r.externalId)).not.toContain("echo-rec-1");
    expect(excluded.recommendations).toHaveLength(2);
  });
});

describe("dismiss / clear / updateProfile", () => {
  it("dismiss returns dismissed: true", async () => {
    expect(
      await provider.dismiss?.({ externalId: "echo-rec-1", reason: "not_interested" }),
    ).toEqual({ dismissed: true });
  });

  it("clear returns cleared: true", async () => {
    expect(await provider.clear?.()).toEqual({ cleared: true });
  });

  it("updateProfile reports entries processed", async () => {
    const res = await provider.updateProfile?.({ entries: [seed("A"), seed("B")] });
    expect(res).toEqual({ updated: true, entriesProcessed: 2 });
  });
});

describe("payload recording", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "rec-echo-test-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes paired request/response files when a recorder is set", async () => {
    setRecorder(
      new PayloadRecorder({
        pluginName: "recommendations-echo",
        dataDir: dir,
        configSnapshot: { adminConfig: {}, userConfig: {} },
        logger: { info: () => {}, warn: () => {}, debug: () => {} },
      }),
    );

    await provider.get(request({ library: [seed("A")] }));

    const files = (await readdir(join(dir, "payloads"))).sort();
    expect(files).toHaveLength(2);
    expect(files.some((f) => f.endsWith("-recommendations_get-request.json"))).toBe(true);
    expect(files.some((f) => f.endsWith("-recommendations_get-response.json"))).toBe(true);
  });
});
