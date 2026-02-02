/**
 * MSW handlers for metrics API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import {
  createInventoryMetrics,
  createPluginMethodMetrics,
  createPluginMetrics,
  createTaskMetrics,
  createTaskTypeMetrics,
} from "../data/factories";

type PluginMetricsResponse = components["schemas"]["PluginMetricsResponse"];

export const metricsHandlers = [
  // Get inventory metrics
  http.get("/api/v1/metrics/inventory", async () => {
    await delay(100);
    const metrics = createInventoryMetrics({
      libraryCount: 4,
      seriesCount: 287,
      bookCount: 3842,
      totalBookSize: 78_652_416_000, // ~73.3 GB
      userCount: 8,
      databaseSize: 156_237_824, // ~149 MB
      pageCount: 142_567,
      libraries: [
        {
          id: "lib-comics-001",
          name: "Comics",
          seriesCount: 124,
          bookCount: 1856,
          totalSize: 42_949_672_960, // ~40 GB
        },
        {
          id: "lib-manga-002",
          name: "Manga",
          seriesCount: 89,
          bookCount: 1247,
          totalSize: 28_991_029_248, // ~27 GB
        },
        {
          id: "lib-ebooks-003",
          name: "Ebooks",
          seriesCount: 52,
          bookCount: 523,
          totalSize: 5_368_709_120, // ~5 GB
        },
        {
          id: "lib-graphic-004",
          name: "Graphic Novels",
          seriesCount: 22,
          bookCount: 216,
          totalSize: 1_343_004_672, // ~1.25 GB
        },
      ],
    });

    return HttpResponse.json(metrics);
  }),

  // Get task metrics
  http.get("/api/v1/metrics/tasks", async () => {
    await delay(100);

    // Define task type metrics
    const byType = [
      createTaskTypeMetrics({
        taskType: "scan_library",
        executed: 156,
        succeeded: 148,
        failed: 8,
        retried: 12,
        avgDurationMs: 45200,
        minDurationMs: 2100,
        maxDurationMs: 185000,
        p50DurationMs: 32000,
        p95DurationMs: 125000,
        itemsProcessed: 15420,
        bytesProcessed: 52_428_800_000,
        throughputPerSec: 5.2,
        errorRatePct: 5.1,
      }),
      createTaskTypeMetrics({
        taskType: "generate_thumbnail",
        executed: 8542,
        succeeded: 8539,
        failed: 3,
        retried: 5,
        avgDurationMs: 350,
        minDurationMs: 45,
        maxDurationMs: 2500,
        p50DurationMs: 280,
        p95DurationMs: 850,
        itemsProcessed: 8542,
        bytesProcessed: 1_250_000_000,
        throughputPerSec: 142.5,
        errorRatePct: 0.04,
      }),
      createTaskTypeMetrics({
        taskType: "analyze_book",
        executed: 2341,
        succeeded: 2298,
        failed: 43,
        retried: 65,
        avgDurationMs: 1850,
        minDurationMs: 120,
        maxDurationMs: 15000,
        p50DurationMs: 1200,
        p95DurationMs: 6500,
        itemsProcessed: 2341,
        bytesProcessed: 8_500_000_000,
        throughputPerSec: 18.2,
        errorRatePct: 1.84,
        lastError: "Failed to parse ComicInfo.xml: invalid UTF-8 sequence",
        lastErrorAt: new Date(Date.now() - 3600000).toISOString(),
      }),
      createTaskTypeMetrics({
        taskType: "extract_metadata",
        executed: 1205,
        succeeded: 1205,
        failed: 0,
        retried: 0,
        avgDurationMs: 520,
        minDurationMs: 85,
        maxDurationMs: 3200,
        p50DurationMs: 420,
        p95DurationMs: 1100,
        itemsProcessed: 1205,
        bytesProcessed: 450_000_000,
        throughputPerSec: 38.5,
        errorRatePct: 0,
      }),
      createTaskTypeMetrics({
        taskType: "cleanup_orphans",
        executed: 24,
        succeeded: 24,
        failed: 0,
        retried: 0,
        avgDurationMs: 8500,
        minDurationMs: 1200,
        maxDurationMs: 45000,
        p50DurationMs: 5500,
        p95DurationMs: 32000,
        itemsProcessed: 156,
        bytesProcessed: 0,
        throughputPerSec: 0.5,
        errorRatePct: 0,
      }),
      createTaskTypeMetrics({
        taskType: "hash_file",
        executed: 3842,
        succeeded: 3842,
        failed: 0,
        retried: 0,
        avgDurationMs: 125,
        minDurationMs: 15,
        maxDurationMs: 2100,
        p50DurationMs: 95,
        p95DurationMs: 450,
        itemsProcessed: 3842,
        bytesProcessed: 78_652_416_000,
        throughputPerSec: 245.8,
        errorRatePct: 0,
      }),
    ];

    // Calculate summary from task types
    const totalExecuted = byType.reduce((sum, t) => sum + t.executed, 0);
    const totalSucceeded = byType.reduce((sum, t) => sum + t.succeeded, 0);
    const totalFailed = byType.reduce((sum, t) => sum + t.failed, 0);
    const weightedDuration = byType.reduce(
      (sum, t) => sum + t.avgDurationMs * t.executed,
      0,
    );
    const weightedQueueWait = byType.reduce(
      (sum, t) => sum + t.avgQueueWaitMs * t.executed,
      0,
    );

    const metrics = createTaskMetrics({
      updatedAt: new Date().toISOString(),
      retention: "30",
      summary: {
        totalExecuted: totalExecuted,
        totalSucceeded: totalSucceeded,
        totalFailed: totalFailed,
        avgDurationMs: totalExecuted > 0 ? weightedDuration / totalExecuted : 0,
        avgQueueWaitMs:
          totalExecuted > 0 ? weightedQueueWait / totalExecuted : 0,
        tasksPerMinute: 12.4,
      },
      queue: {
        pendingCount: 23,
        processingCount: 4,
        staleCount: 0,
        oldestPendingAgeMs: 45200,
      },
      byType: byType,
    });

    return HttpResponse.json(metrics);
  }),

  // Get task metrics history
  http.get("/api/v1/metrics/tasks/history", async ({ request }) => {
    await delay(150);
    const url = new URL(request.url);
    const days = parseInt(url.searchParams.get("days") || "7", 10);
    const taskType = url.searchParams.get("taskType");
    const granularity = url.searchParams.get("granularity") || "hour";

    // Generate historical data points
    const now = new Date();
    const points = [];
    const pointCount = granularity === "hour" ? days * 24 : days;

    for (let i = 0; i < pointCount; i++) {
      const periodStart = new Date(now);
      if (granularity === "hour") {
        periodStart.setHours(periodStart.getHours() - i);
      } else {
        periodStart.setDate(periodStart.getDate() - i);
      }

      points.push({
        periodStart: periodStart.toISOString(),
        taskType: taskType || null,
        count: Math.floor(Math.random() * 100),
        succeeded: Math.floor(Math.random() * 95),
        failed: Math.floor(Math.random() * 5),
        avgDurationMs: Math.random() * 5000,
        minDurationMs: Math.floor(Math.random() * 500),
        maxDurationMs: Math.floor(Math.random() * 10000),
        itemsProcessed: Math.floor(Math.random() * 1000),
        bytesProcessed: Math.floor(Math.random() * 1000000000),
      });
    }

    const from = new Date(now);
    from.setDate(from.getDate() - days);

    return HttpResponse.json({
      from: from.toISOString(),
      to: now.toISOString(),
      granularity,
      points: points.reverse(),
    });
  }),

  // Cleanup old metrics
  http.post("/api/v1/metrics/tasks/cleanup", async () => {
    await delay(200);
    return HttpResponse.json({
      deletedCount: Math.floor(Math.random() * 1000),
      retentionDays: "30",
      oldestRemaining: new Date(
        Date.now() - 30 * 24 * 60 * 60 * 1000,
      ).toISOString(),
    });
  }),

  // Nuke all metrics
  http.delete("/api/v1/metrics/tasks", async () => {
    await delay(200);
    return HttpResponse.json({
      deletedCount: Math.floor(Math.random() * 10000),
    });
  }),

  // Get plugin metrics
  http.get("/api/v1/metrics/plugins", async () => {
    await delay(100);

    // Create realistic plugin metrics with method breakdowns
    const mangabakaMetrics = createPluginMetrics({
      pluginId: "plugin-mangabaka",
      pluginName: "MangaBaka",
      requestsTotal: 1250,
      requestsSuccess: 1198,
      requestsFailed: 52,
      avgDurationMs: 285.5,
      rateLimitRejections: 8,
      errorRatePct: 4.16,
      healthStatus: "healthy",
      lastSuccess: new Date(Date.now() - 300000).toISOString(), // 5 min ago
      lastFailure: new Date(Date.now() - 3600000).toISOString(), // 1 hour ago
      byMethod: {
        search: createPluginMethodMetrics({
          method: "search",
          requestsTotal: 450,
          requestsSuccess: 438,
          requestsFailed: 12,
          avgDurationMs: 320.5,
        }),
        get: createPluginMethodMetrics({
          method: "get",
          requestsTotal: 680,
          requestsSuccess: 665,
          requestsFailed: 15,
          avgDurationMs: 245.2,
        }),
        match: createPluginMethodMetrics({
          method: "match",
          requestsTotal: 120,
          requestsSuccess: 95,
          requestsFailed: 25,
          avgDurationMs: 380.8,
        }),
      },
      failureCounts: {
        TIMEOUT: 28,
        RPC_ERROR: 15,
        RATE_LIMITED: 9,
      },
    });

    const comicvineMetrics = createPluginMetrics({
      pluginId: "plugin-comicvine",
      pluginName: "ComicVine",
      requestsTotal: 340,
      requestsSuccess: 285,
      requestsFailed: 55,
      avgDurationMs: 425.3,
      rateLimitRejections: 22,
      errorRatePct: 16.18,
      healthStatus: "degraded",
      lastSuccess: new Date(Date.now() - 1800000).toISOString(), // 30 min ago
      lastFailure: new Date(Date.now() - 600000).toISOString(), // 10 min ago
      byMethod: {
        search: createPluginMethodMetrics({
          method: "search",
          requestsTotal: 200,
          requestsSuccess: 165,
          requestsFailed: 35,
          avgDurationMs: 480.2,
        }),
        get: createPluginMethodMetrics({
          method: "get",
          requestsTotal: 140,
          requestsSuccess: 120,
          requestsFailed: 20,
          avgDurationMs: 350.1,
        }),
      },
      failureCounts: {
        RATE_LIMITED: 32,
        TIMEOUT: 18,
        AUTH_FAILED: 5,
      },
    });

    const anilistMetrics = createPluginMetrics({
      pluginId: "plugin-anilist",
      pluginName: "AniList",
      requestsTotal: 890,
      requestsSuccess: 888,
      requestsFailed: 2,
      avgDurationMs: 125.8,
      rateLimitRejections: 0,
      errorRatePct: 0.22,
      healthStatus: "healthy",
      lastSuccess: new Date(Date.now() - 60000).toISOString(), // 1 min ago
      lastFailure: null,
      byMethod: {
        search: createPluginMethodMetrics({
          method: "search",
          requestsTotal: 520,
          requestsSuccess: 519,
          requestsFailed: 1,
          avgDurationMs: 115.3,
        }),
        get: createPluginMethodMetrics({
          method: "get",
          requestsTotal: 370,
          requestsSuccess: 369,
          requestsFailed: 1,
          avgDurationMs: 140.2,
        }),
      },
      failureCounts: {
        TIMEOUT: 2,
      },
    });

    const plugins = [mangabakaMetrics, comicvineMetrics, anilistMetrics];

    // Calculate summary from plugins
    const totalRequests = plugins.reduce((sum, p) => sum + p.requestsTotal, 0);
    const totalSuccess = plugins.reduce((sum, p) => sum + p.requestsSuccess, 0);
    const totalFailed = plugins.reduce((sum, p) => sum + p.requestsFailed, 0);
    const totalRateLimitRejections = plugins.reduce(
      (sum, p) => sum + p.rateLimitRejections,
      0,
    );

    const response: PluginMetricsResponse = {
      updatedAt: new Date().toISOString(),
      summary: {
        totalPlugins: plugins.length,
        healthyPlugins: plugins.filter((p) => p.healthStatus === "healthy")
          .length,
        degradedPlugins: plugins.filter((p) => p.healthStatus === "degraded")
          .length,
        unhealthyPlugins: plugins.filter((p) => p.healthStatus === "unhealthy")
          .length,
        totalRequests: totalRequests,
        totalSuccess: totalSuccess,
        totalFailed: totalFailed,
        totalRateLimitRejections: totalRateLimitRejections,
      },
      plugins,
    };

    return HttpResponse.json(response);
  }),
];
