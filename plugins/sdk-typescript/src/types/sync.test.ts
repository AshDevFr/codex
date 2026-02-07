/**
 * Tests for sync protocol types and SyncProvider interface
 *
 * These tests cover:
 * - Type definitions compile correctly with valid data
 * - All enum string literal values match Rust snake_case serialization
 * - Interface shapes match the Rust protocol (camelCase properties)
 * - Optional fields are properly handled
 * - SyncProvider interface method signatures
 */

import { describe, expect, it } from "vitest";
import type { SyncProvider } from "./capabilities.js";
import type {
  ExternalUserInfo,
  SyncEntry,
  SyncEntryResult,
  SyncEntryResultStatus,
  SyncProgress,
  SyncPullRequest,
  SyncPullResponse,
  SyncPushRequest,
  SyncPushResponse,
  SyncReadingStatus,
  SyncStatusResponse,
} from "./sync.js";

// =============================================================================
// SyncReadingStatus Tests
// =============================================================================

describe("SyncReadingStatus", () => {
  it("should accept all valid reading status values", () => {
    const statuses: SyncReadingStatus[] = [
      "reading",
      "completed",
      "on_hold",
      "dropped",
      "plan_to_read",
    ];
    expect(statuses).toHaveLength(5);
    expect(statuses).toContain("reading");
    expect(statuses).toContain("completed");
    expect(statuses).toContain("on_hold");
    expect(statuses).toContain("dropped");
    expect(statuses).toContain("plan_to_read");
  });

  it("should use snake_case values matching Rust serialization", () => {
    // These must match the Rust #[serde(rename_all = "snake_case")] output
    const onHold: SyncReadingStatus = "on_hold";
    const planToRead: SyncReadingStatus = "plan_to_read";
    expect(onHold).toBe("on_hold");
    expect(planToRead).toBe("plan_to_read");
  });
});

// =============================================================================
// SyncEntryResultStatus Tests
// =============================================================================

describe("SyncEntryResultStatus", () => {
  it("should accept all valid result status values", () => {
    const statuses: SyncEntryResultStatus[] = ["created", "updated", "unchanged", "failed"];
    expect(statuses).toHaveLength(4);
    expect(statuses).toContain("created");
    expect(statuses).toContain("updated");
    expect(statuses).toContain("unchanged");
    expect(statuses).toContain("failed");
  });
});

// =============================================================================
// ExternalUserInfo Tests
// =============================================================================

describe("ExternalUserInfo", () => {
  it("should accept full user info with all fields", () => {
    const info: ExternalUserInfo = {
      externalId: "12345",
      username: "manga_reader",
      avatarUrl: "https://anilist.co/img/avatar.jpg",
      profileUrl: "https://anilist.co/user/manga_reader",
    };
    expect(info.externalId).toBe("12345");
    expect(info.username).toBe("manga_reader");
    expect(info.avatarUrl).toBe("https://anilist.co/img/avatar.jpg");
    expect(info.profileUrl).toBe("https://anilist.co/user/manga_reader");
  });

  it("should accept minimal user info without optional fields", () => {
    const info: ExternalUserInfo = {
      externalId: "99",
      username: "user99",
    };
    expect(info.externalId).toBe("99");
    expect(info.username).toBe("user99");
    expect(info.avatarUrl).toBeUndefined();
    expect(info.profileUrl).toBeUndefined();
  });

  it("should use camelCase property names matching Rust serialization", () => {
    const info: ExternalUserInfo = {
      externalId: "1",
      username: "test",
      avatarUrl: "https://example.com/avatar.jpg",
      profileUrl: "https://example.com/profile",
    };
    // Verify camelCase keys exist (matching serde rename_all = "camelCase")
    expect("externalId" in info).toBe(true);
    expect("avatarUrl" in info).toBe(true);
    expect("profileUrl" in info).toBe(true);
  });
});

// =============================================================================
// SyncProgress Tests
// =============================================================================

describe("SyncProgress", () => {
  it("should accept full progress with all fields", () => {
    const progress: SyncProgress = {
      chapters: 100,
      volumes: 10,
      pages: 3200,
    };
    expect(progress.chapters).toBe(100);
    expect(progress.volumes).toBe(10);
    expect(progress.pages).toBe(3200);
  });

  it("should accept partial progress with only chapters", () => {
    const progress: SyncProgress = {
      chapters: 50,
    };
    expect(progress.chapters).toBe(50);
    expect(progress.volumes).toBeUndefined();
    expect(progress.pages).toBeUndefined();
  });

  it("should accept empty progress with no fields", () => {
    const progress: SyncProgress = {};
    expect(progress.chapters).toBeUndefined();
    expect(progress.volumes).toBeUndefined();
    expect(progress.pages).toBeUndefined();
  });
});

// =============================================================================
// SyncEntry Tests
// =============================================================================

describe("SyncEntry", () => {
  it("should accept full entry with all fields", () => {
    const entry: SyncEntry = {
      externalId: "12345",
      status: "reading",
      progress: {
        chapters: 42,
        volumes: 5,
      },
      score: 8.5,
      startedAt: "2026-01-15T00:00:00Z",
      completedAt: "2026-02-01T00:00:00Z",
      notes: "Great series!",
    };
    expect(entry.externalId).toBe("12345");
    expect(entry.status).toBe("reading");
    expect(entry.progress?.chapters).toBe(42);
    expect(entry.progress?.volumes).toBe(5);
    expect(entry.score).toBe(8.5);
    expect(entry.startedAt).toBe("2026-01-15T00:00:00Z");
    expect(entry.completedAt).toBe("2026-02-01T00:00:00Z");
    expect(entry.notes).toBe("Great series!");
  });

  it("should accept minimal entry with only required fields", () => {
    const entry: SyncEntry = {
      externalId: "99",
      status: "completed",
    };
    expect(entry.externalId).toBe("99");
    expect(entry.status).toBe("completed");
    expect(entry.progress).toBeUndefined();
    expect(entry.score).toBeUndefined();
    expect(entry.startedAt).toBeUndefined();
    expect(entry.completedAt).toBeUndefined();
    expect(entry.notes).toBeUndefined();
  });

  it("should accept all reading statuses in entries", () => {
    const statuses: SyncReadingStatus[] = [
      "reading",
      "completed",
      "on_hold",
      "dropped",
      "plan_to_read",
    ];
    for (const status of statuses) {
      const entry: SyncEntry = { externalId: "1", status };
      expect(entry.status).toBe(status);
    }
  });

  it("should use camelCase property names matching Rust serialization", () => {
    const entry: SyncEntry = {
      externalId: "1",
      status: "reading",
      startedAt: "2026-01-01T00:00:00Z",
      completedAt: "2026-02-01T00:00:00Z",
    };
    expect("externalId" in entry).toBe(true);
    expect("startedAt" in entry).toBe(true);
    expect("completedAt" in entry).toBe(true);
  });
});

// =============================================================================
// SyncPushRequest Tests
// =============================================================================

describe("SyncPushRequest", () => {
  it("should accept request with multiple entries", () => {
    const request: SyncPushRequest = {
      entries: [
        {
          externalId: "1",
          status: "reading",
          progress: { chapters: 10 },
        },
        {
          externalId: "2",
          status: "completed",
          score: 9.0,
          completedAt: "2026-02-01T00:00:00Z",
        },
      ],
    };
    expect(request.entries).toHaveLength(2);
    expect(request.entries[0]?.externalId).toBe("1");
    expect(request.entries[1]?.status).toBe("completed");
  });

  it("should accept request with empty entries array", () => {
    const request: SyncPushRequest = { entries: [] };
    expect(request.entries).toHaveLength(0);
  });
});

// =============================================================================
// SyncEntryResult Tests
// =============================================================================

describe("SyncEntryResult", () => {
  it("should accept successful result without error", () => {
    const result: SyncEntryResult = {
      externalId: "1",
      status: "updated",
    };
    expect(result.externalId).toBe("1");
    expect(result.status).toBe("updated");
    expect(result.error).toBeUndefined();
  });

  it("should accept failed result with error message", () => {
    const result: SyncEntryResult = {
      externalId: "3",
      status: "failed",
      error: "Rate limited",
    };
    expect(result.externalId).toBe("3");
    expect(result.status).toBe("failed");
    expect(result.error).toBe("Rate limited");
  });

  it("should accept all result status values", () => {
    const statuses: SyncEntryResultStatus[] = ["created", "updated", "unchanged", "failed"];
    for (const status of statuses) {
      const result: SyncEntryResult = { externalId: "1", status };
      expect(result.status).toBe(status);
    }
  });
});

// =============================================================================
// SyncPushResponse Tests
// =============================================================================

describe("SyncPushResponse", () => {
  it("should accept response with success and failed entries", () => {
    const response: SyncPushResponse = {
      success: [
        { externalId: "1", status: "updated" },
        { externalId: "2", status: "created" },
      ],
      failed: [{ externalId: "3", status: "failed", error: "Rate limited" }],
    };
    expect(response.success).toHaveLength(2);
    expect(response.success[0]?.status).toBe("updated");
    expect(response.success[1]?.status).toBe("created");
    expect(response.failed).toHaveLength(1);
    expect(response.failed[0]?.status).toBe("failed");
    expect(response.failed[0]?.error).toBe("Rate limited");
  });

  it("should accept response with all success and no failures", () => {
    const response: SyncPushResponse = {
      success: [{ externalId: "1", status: "unchanged" }],
      failed: [],
    };
    expect(response.success).toHaveLength(1);
    expect(response.failed).toHaveLength(0);
  });

  it("should accept empty response", () => {
    const response: SyncPushResponse = {
      success: [],
      failed: [],
    };
    expect(response.success).toHaveLength(0);
    expect(response.failed).toHaveLength(0);
  });
});

// =============================================================================
// SyncPullRequest Tests
// =============================================================================

describe("SyncPullRequest", () => {
  it("should accept request with all fields", () => {
    const request: SyncPullRequest = {
      since: "2026-02-01T00:00:00Z",
      limit: 50,
      cursor: "next_page_token",
    };
    expect(request.since).toBe("2026-02-01T00:00:00Z");
    expect(request.limit).toBe(50);
    expect(request.cursor).toBe("next_page_token");
  });

  it("should accept empty request with no fields", () => {
    const request: SyncPullRequest = {};
    expect(request.since).toBeUndefined();
    expect(request.limit).toBeUndefined();
    expect(request.cursor).toBeUndefined();
  });

  it("should accept request with only since", () => {
    const request: SyncPullRequest = {
      since: "2026-01-01T00:00:00Z",
    };
    expect(request.since).toBe("2026-01-01T00:00:00Z");
  });

  it("should accept request with only cursor for pagination", () => {
    const request: SyncPullRequest = {
      cursor: "page_2_cursor",
    };
    expect(request.cursor).toBe("page_2_cursor");
  });
});

// =============================================================================
// SyncPullResponse Tests
// =============================================================================

describe("SyncPullResponse", () => {
  it("should accept response with entries and pagination", () => {
    const response: SyncPullResponse = {
      entries: [
        {
          externalId: "42",
          status: "on_hold",
          progress: { chapters: 25 },
          score: 7.0,
        },
      ],
      nextCursor: "page2",
      hasMore: true,
    };
    expect(response.entries).toHaveLength(1);
    expect(response.entries[0]?.status).toBe("on_hold");
    expect(response.nextCursor).toBe("page2");
    expect(response.hasMore).toBe(true);
  });

  it("should accept last page response with no more entries", () => {
    const response: SyncPullResponse = {
      entries: [],
      hasMore: false,
    };
    expect(response.entries).toHaveLength(0);
    expect(response.nextCursor).toBeUndefined();
    expect(response.hasMore).toBe(false);
  });

  it("should use camelCase for nextCursor and hasMore", () => {
    const response: SyncPullResponse = {
      entries: [],
      nextCursor: "abc",
      hasMore: true,
    };
    expect("nextCursor" in response).toBe(true);
    expect("hasMore" in response).toBe(true);
  });
});

// =============================================================================
// SyncStatusResponse Tests
// =============================================================================

describe("SyncStatusResponse", () => {
  it("should accept full status response", () => {
    const response: SyncStatusResponse = {
      lastSyncAt: "2026-02-06T12:00:00Z",
      externalCount: 150,
      pendingPush: 5,
      pendingPull: 3,
      conflicts: 1,
    };
    expect(response.lastSyncAt).toBe("2026-02-06T12:00:00Z");
    expect(response.externalCount).toBe(150);
    expect(response.pendingPush).toBe(5);
    expect(response.pendingPull).toBe(3);
    expect(response.conflicts).toBe(1);
  });

  it("should accept minimal status response with required fields only", () => {
    const response: SyncStatusResponse = {
      pendingPush: 0,
      pendingPull: 0,
      conflicts: 0,
    };
    expect(response.lastSyncAt).toBeUndefined();
    expect(response.externalCount).toBeUndefined();
    expect(response.pendingPush).toBe(0);
    expect(response.pendingPull).toBe(0);
    expect(response.conflicts).toBe(0);
  });

  it("should use camelCase property names matching Rust serialization", () => {
    const response: SyncStatusResponse = {
      lastSyncAt: "2026-02-06T12:00:00Z",
      externalCount: 100,
      pendingPush: 0,
      pendingPull: 0,
      conflicts: 0,
    };
    expect("lastSyncAt" in response).toBe(true);
    expect("externalCount" in response).toBe(true);
    expect("pendingPush" in response).toBe(true);
    expect("pendingPull" in response).toBe(true);
  });
});

// =============================================================================
// SyncProvider Interface Tests
// =============================================================================

describe("SyncProvider", () => {
  it("should accept a complete sync provider implementation", () => {
    const provider: SyncProvider = {
      async getUserInfo(): Promise<ExternalUserInfo> {
        return {
          externalId: "12345",
          username: "manga_reader",
          avatarUrl: "https://anilist.co/img/avatar.jpg",
          profileUrl: "https://anilist.co/user/manga_reader",
        };
      },
      async pushProgress(params: SyncPushRequest): Promise<SyncPushResponse> {
        return {
          success: params.entries.map((e) => ({
            externalId: e.externalId,
            status: "updated" as SyncEntryResultStatus,
          })),
          failed: [],
        };
      },
      async pullProgress(_params: SyncPullRequest): Promise<SyncPullResponse> {
        return {
          entries: [
            {
              externalId: "42",
              status: "reading",
              progress: { chapters: 10 },
            },
          ],
          hasMore: false,
        };
      },
      async status(): Promise<SyncStatusResponse> {
        return {
          lastSyncAt: "2026-02-06T12:00:00Z",
          externalCount: 42,
          pendingPush: 0,
          pendingPull: 0,
          conflicts: 0,
        };
      },
    };

    expect(provider.getUserInfo).toBeDefined();
    expect(provider.pushProgress).toBeDefined();
    expect(provider.pullProgress).toBeDefined();
    expect(provider.status).toBeDefined();
  });

  it("should accept provider without optional status method", () => {
    const provider: SyncProvider = {
      async getUserInfo(): Promise<ExternalUserInfo> {
        return { externalId: "1", username: "test" };
      },
      async pushProgress(_params: SyncPushRequest): Promise<SyncPushResponse> {
        return { success: [], failed: [] };
      },
      async pullProgress(_params: SyncPullRequest): Promise<SyncPullResponse> {
        return { entries: [], hasMore: false };
      },
    };

    expect(provider.getUserInfo).toBeDefined();
    expect(provider.pushProgress).toBeDefined();
    expect(provider.pullProgress).toBeDefined();
    expect(provider.status).toBeUndefined();
  });

  it("should produce correct return types from provider methods", async () => {
    const provider: SyncProvider = {
      async getUserInfo() {
        return { externalId: "1", username: "test" };
      },
      async pushProgress() {
        return { success: [], failed: [] };
      },
      async pullProgress() {
        return { entries: [], hasMore: false };
      },
    };

    const userInfo = await provider.getUserInfo();
    expect(userInfo.externalId).toBe("1");
    expect(userInfo.username).toBe("test");

    const pushResult = await provider.pushProgress({ entries: [] });
    expect(pushResult.success).toEqual([]);
    expect(pushResult.failed).toEqual([]);

    const pullResult = await provider.pullProgress({});
    expect(pullResult.entries).toEqual([]);
    expect(pullResult.hasMore).toBe(false);
  });
});

// =============================================================================
// Cross-type Integration Tests
// =============================================================================

describe("Sync Protocol Integration", () => {
  it("should round-trip a full push flow with correct types", () => {
    // Build a push request
    const request: SyncPushRequest = {
      entries: [
        {
          externalId: "anilist:12345",
          status: "reading",
          progress: { chapters: 42, volumes: 5 },
          score: 8.5,
          startedAt: "2026-01-15T00:00:00Z",
          notes: "Enjoying this series",
        },
        {
          externalId: "anilist:67890",
          status: "completed",
          progress: { chapters: 200, volumes: 20 },
          score: 9.5,
          startedAt: "2025-06-01T00:00:00Z",
          completedAt: "2026-01-30T00:00:00Z",
        },
        {
          externalId: "anilist:11111",
          status: "plan_to_read",
        },
      ],
    };

    // Build a push response
    const response: SyncPushResponse = {
      success: [
        { externalId: "anilist:12345", status: "updated" },
        { externalId: "anilist:67890", status: "unchanged" },
        { externalId: "anilist:11111", status: "created" },
      ],
      failed: [],
    };

    expect(request.entries).toHaveLength(3);
    expect(response.success).toHaveLength(3);
    expect(response.failed).toHaveLength(0);
  });

  it("should round-trip a full pull flow with pagination", () => {
    // First page request
    const request1: SyncPullRequest = {
      since: "2026-02-01T00:00:00Z",
      limit: 2,
    };

    // First page response
    const response1: SyncPullResponse = {
      entries: [
        { externalId: "1", status: "reading", progress: { chapters: 10 } },
        { externalId: "2", status: "completed", score: 9.0 },
      ],
      nextCursor: "cursor_page_2",
      hasMore: true,
    };

    // Second page request (using cursor)
    const request2: SyncPullRequest = {
      cursor: response1.nextCursor,
      limit: 2,
    };

    // Second (last) page response
    const response2: SyncPullResponse = {
      entries: [{ externalId: "3", status: "dropped" }],
      hasMore: false,
    };

    expect(request1.since).toBe("2026-02-01T00:00:00Z");
    expect(response1.hasMore).toBe(true);
    expect(request2.cursor).toBe("cursor_page_2");
    expect(response2.hasMore).toBe(false);
    expect(response2.nextCursor).toBeUndefined();
  });
});
