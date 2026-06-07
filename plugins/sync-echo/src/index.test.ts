import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { SyncPushRequest } from "@ashdev/codex-plugin-sdk";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { provider, setPullCount, setRecorder } from "./index.js";
import { PayloadRecorder } from "./recorder.js";

afterEach(() => {
  setRecorder(null);
  setPullCount(3);
});

describe("getUserInfo", () => {
  it("returns a deterministic fake identity", async () => {
    const info = await provider.getUserInfo();
    expect(info.externalId).toBe("echo-user-1");
    expect(info.username).toBe("echo_user");
  });
});

describe("pushProgress", () => {
  it("echoes every entry as success, alternating created/updated, never failing", async () => {
    const req: SyncPushRequest = {
      entries: [
        { externalId: "a", status: "reading" },
        { externalId: "b", status: "completed" },
        { externalId: "c", status: "dropped" },
      ],
    };
    const res = await provider.pushProgress(req);

    expect(res.failed).toEqual([]);
    expect(res.success.map((s) => s.externalId)).toEqual(["a", "b", "c"]);
    expect(res.success.map((s) => s.status)).toEqual(["created", "updated", "created"]);
  });
});

describe("pullProgress", () => {
  it("returns pullCount fully-populated entries by default", async () => {
    setPullCount(3);
    const res = await provider.pullProgress({});

    expect(res.hasMore).toBe(false);
    expect(res.entries).toHaveLength(3);

    const first = res.entries[0];
    expect(first.externalId).toBe("1000");
    expect(first.status).toBe("reading");
    expect(first.score).toBeDefined();
    expect(first.startedAt).toBeDefined();
    expect(first.completedAt).toBeDefined();
    expect(first.notes).toBeDefined();
    expect(first.latestUpdatedAt).toBeDefined();
    expect(first.title).toBe("Echo Series 1");
    // Detailed progress is populated (this plugin opts into wantsDetailedProgress).
    expect(first.progress?.maxVolume).toBe(1);
    expect(first.progress?.maxChapter).toBe(1.5);
    expect(first.progress?.readBooks).toHaveLength(1);
  });

  it("respects an explicit limit, capped at pullCount", async () => {
    setPullCount(5);
    expect((await provider.pullProgress({ limit: 2 })).entries).toHaveLength(2);
    expect((await provider.pullProgress({ limit: 99 })).entries).toHaveLength(5);
  });
});

describe("status", () => {
  it("returns canned status counts", async () => {
    setPullCount(7);
    const status = await provider.status();
    expect(status.externalCount).toBe(7);
    expect(status.pendingPush).toBe(0);
    expect(status.conflicts).toBe(0);
  });
});

describe("payload recording", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "sync-echo-test-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes paired request/response files when a recorder is set", async () => {
    setRecorder(
      new PayloadRecorder({
        pluginName: "sync-echo",
        dataDir: dir,
        configSnapshot: { adminConfig: {}, userConfig: {} },
        logger: { info: () => {}, warn: () => {}, debug: () => {} },
      }),
    );

    await provider.pushProgress({ entries: [{ externalId: "a", status: "reading" }] });

    const files = (await readdir(join(dir, "payloads"))).sort();
    expect(files).toHaveLength(2);
    expect(files.some((f) => f.endsWith("-sync_pushProgress-request.json"))).toBe(true);
    expect(files.some((f) => f.endsWith("-sync_pushProgress-response.json"))).toBe(true);
  });
});
