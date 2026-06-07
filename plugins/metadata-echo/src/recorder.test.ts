import { mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { PayloadRecorder, type RecorderLogger, redactConfig } from "./recorder.js";

function makeLogger(): RecorderLogger & { warnings: string[] } {
  const warnings: string[] = [];
  return {
    warnings,
    info: () => {},
    debug: () => {},
    warn: (m: string) => warnings.push(m),
  };
}

const fixedDate = new Date("2026-06-07T08:09:05.123Z");

describe("redactConfig", () => {
  it("redacts secret-like keys but keeps the rest", () => {
    const out = redactConfig({
      adminConfig: { recordPayloads: true, apiKey: "abc", clientSecret: "xyz" },
      userConfig: { progressUnit: "volumes", access_token: "tok", nested: { password: "p" } },
    }) as {
      adminConfig: Record<string, unknown>;
      userConfig: Record<string, unknown>;
    };

    expect(out.adminConfig.recordPayloads).toBe(true);
    expect(out.adminConfig.apiKey).toBe("[REDACTED]");
    expect(out.adminConfig.clientSecret).toBe("[REDACTED]");
    expect(out.userConfig.progressUnit).toBe("volumes");
    expect(out.userConfig.access_token).toBe("[REDACTED]");
    expect((out.userConfig.nested as Record<string, unknown>).password).toBe("[REDACTED]");
  });

  it("defaults missing config sections to empty objects", () => {
    expect(redactConfig({})).toEqual({ adminConfig: {}, userConfig: {} });
  });
});

describe("PayloadRecorder", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "rec-test-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes paired request/response files with a sortable basename", async () => {
    const recorder = new PayloadRecorder({
      pluginName: "metadata-echo",
      dataDir: dir,
      configSnapshot: { adminConfig: {}, userConfig: {} },
      logger: makeLogger(),
      now: () => fixedDate,
    });

    await recorder.record("metadata/search", { query: "naruto" }, { results: [] });

    const files = (await readdir(join(dir, "payloads"))).sort();
    expect(files).toEqual([
      "2026-06-07-08-09-05-0001-metadata_search-request.json",
      "2026-06-07-08-09-05-0001-metadata_search-response.json",
    ]);
  });

  it("embeds payload + config in the envelope and pairs by id", async () => {
    const snapshot = { adminConfig: { maxResults: 5 }, userConfig: {} };
    const recorder = new PayloadRecorder({
      pluginName: "metadata-echo",
      dataDir: dir,
      configSnapshot: snapshot,
      logger: makeLogger(),
      now: () => fixedDate,
    });

    await recorder.record("sync/pushProgress", { entries: [{ externalId: "1" }] }, { success: [] });

    const reqRaw = await readFile(
      join(dir, "payloads", "2026-06-07-08-09-05-0001-sync_pushProgress-request.json"),
      "utf8",
    );
    const resRaw = await readFile(
      join(dir, "payloads", "2026-06-07-08-09-05-0001-sync_pushProgress-response.json"),
      "utf8",
    );
    const req = JSON.parse(reqRaw);
    const res = JSON.parse(resRaw);

    expect(req.direction).toBe("request");
    expect(req.method).toBe("sync/pushProgress");
    expect(req.id).toBe(1);
    expect(req.config).toEqual(snapshot);
    expect(req.payload).toEqual({ entries: [{ externalId: "1" }] });
    expect(req.timestamp).toBe("2026-06-07T08:09:05.123Z");

    expect(res.direction).toBe("response");
    expect(res.id).toBe(1);
    expect(res.payload).toEqual({ success: [] });
  });

  it("increments the id per call", async () => {
    const recorder = new PayloadRecorder({
      pluginName: "p",
      dataDir: dir,
      configSnapshot: {},
      logger: makeLogger(),
      now: () => fixedDate,
    });

    await recorder.record("a/b", {}, {});
    await recorder.record("a/b", {}, {});

    const files = (await readdir(join(dir, "payloads"))).sort();
    expect(files).toContain("2026-06-07-08-09-05-0001-a_b-request.json");
    expect(files).toContain("2026-06-07-08-09-05-0002-a_b-request.json");
  });

  it("prunes oldest files beyond maxFiles", async () => {
    const recorder = new PayloadRecorder({
      pluginName: "p",
      dataDir: dir,
      configSnapshot: {},
      logger: makeLogger(),
      maxFiles: 2, // keeps only the newest single call (2 files)
      now: () => fixedDate,
    });

    await recorder.record("m", {}, {}); // id 0001
    await recorder.record("m", {}, {}); // id 0002

    const files = (await readdir(join(dir, "payloads"))).sort();
    expect(files).toEqual([
      "2026-06-07-08-09-05-0002-m-request.json",
      "2026-06-07-08-09-05-0002-m-response.json",
    ]);
  });

  it("writes nothing when disabled", async () => {
    const recorder = new PayloadRecorder({
      pluginName: "p",
      dataDir: dir,
      configSnapshot: {},
      logger: makeLogger(),
      enabled: false,
      now: () => fixedDate,
    });

    await recorder.record("m", {}, {});
    await expect(readdir(join(dir, "payloads"))).rejects.toThrow(); // dir never created
  });

  it("falls back to the temp dir and warns when no dataDir is given", () => {
    const logger = makeLogger();
    const recorder = new PayloadRecorder({
      pluginName: "metadata-echo",
      configSnapshot: {},
      logger,
      now: () => fixedDate,
    });
    expect(recorder.directory).toBe(join(tmpdir(), "codex-metadata-echo", "payloads"));
  });

  it("never throws when the directory cannot be created", async () => {
    const logger = makeLogger();
    // A path under an existing file cannot be turned into a directory (ENOTDIR).
    const blocker = join(dir, "blocker");
    await writeFile(blocker, "not a dir", "utf8");
    const recorder = new PayloadRecorder({
      pluginName: "p",
      dataDir: blocker,
      configSnapshot: {},
      logger,
      now: () => fixedDate,
    });

    await expect(recorder.record("m", {}, {})).resolves.toBeUndefined();
    expect(logger.warnings.length).toBeGreaterThan(0);
  });
});
