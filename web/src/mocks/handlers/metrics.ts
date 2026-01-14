/**
 * MSW handlers for metrics API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import {
	createInventoryMetrics,
	createTaskMetrics,
	createTaskTypeMetrics,
} from "../data/factories";

export const metricsHandlers = [
	// Get inventory metrics
	http.get("/api/v1/metrics/inventory", async () => {
		await delay(100);
		const metrics = createInventoryMetrics({
			library_count: 4,
			series_count: 287,
			book_count: 3842,
			total_book_size: 78_652_416_000, // ~73.3 GB
			user_count: 8,
			database_size: 156_237_824, // ~149 MB
			page_count: 142_567,
			libraries: [
				{
					id: "lib-comics-001",
					name: "Comics",
					series_count: 124,
					book_count: 1856,
					total_size: 42_949_672_960, // ~40 GB
				},
				{
					id: "lib-manga-002",
					name: "Manga",
					series_count: 89,
					book_count: 1247,
					total_size: 28_991_029_248, // ~27 GB
				},
				{
					id: "lib-ebooks-003",
					name: "Ebooks",
					series_count: 52,
					book_count: 523,
					total_size: 5_368_709_120, // ~5 GB
				},
				{
					id: "lib-graphic-004",
					name: "Graphic Novels",
					series_count: 22,
					book_count: 216,
					total_size: 1_343_004_672, // ~1.25 GB
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
				task_type: "scan_library",
				executed: 156,
				succeeded: 148,
				failed: 8,
				retried: 12,
				avg_duration_ms: 45200,
				min_duration_ms: 2100,
				max_duration_ms: 185000,
				p50_duration_ms: 32000,
				p95_duration_ms: 125000,
				items_processed: 15420,
				bytes_processed: 52_428_800_000,
				throughput_per_sec: 5.2,
				error_rate_pct: 5.1,
			}),
			createTaskTypeMetrics({
				task_type: "generate_thumbnail",
				executed: 8542,
				succeeded: 8539,
				failed: 3,
				retried: 5,
				avg_duration_ms: 350,
				min_duration_ms: 45,
				max_duration_ms: 2500,
				p50_duration_ms: 280,
				p95_duration_ms: 850,
				items_processed: 8542,
				bytes_processed: 1_250_000_000,
				throughput_per_sec: 142.5,
				error_rate_pct: 0.04,
			}),
			createTaskTypeMetrics({
				task_type: "analyze_book",
				executed: 2341,
				succeeded: 2298,
				failed: 43,
				retried: 65,
				avg_duration_ms: 1850,
				min_duration_ms: 120,
				max_duration_ms: 15000,
				p50_duration_ms: 1200,
				p95_duration_ms: 6500,
				items_processed: 2341,
				bytes_processed: 8_500_000_000,
				throughput_per_sec: 18.2,
				error_rate_pct: 1.84,
				last_error: "Failed to parse ComicInfo.xml: invalid UTF-8 sequence",
				last_error_at: new Date(Date.now() - 3600000).toISOString(),
			}),
			createTaskTypeMetrics({
				task_type: "extract_metadata",
				executed: 1205,
				succeeded: 1205,
				failed: 0,
				retried: 0,
				avg_duration_ms: 520,
				min_duration_ms: 85,
				max_duration_ms: 3200,
				p50_duration_ms: 420,
				p95_duration_ms: 1100,
				items_processed: 1205,
				bytes_processed: 450_000_000,
				throughput_per_sec: 38.5,
				error_rate_pct: 0,
			}),
			createTaskTypeMetrics({
				task_type: "cleanup_orphans",
				executed: 24,
				succeeded: 24,
				failed: 0,
				retried: 0,
				avg_duration_ms: 8500,
				min_duration_ms: 1200,
				max_duration_ms: 45000,
				p50_duration_ms: 5500,
				p95_duration_ms: 32000,
				items_processed: 156,
				bytes_processed: 0,
				throughput_per_sec: 0.5,
				error_rate_pct: 0,
			}),
			createTaskTypeMetrics({
				task_type: "hash_file",
				executed: 3842,
				succeeded: 3842,
				failed: 0,
				retried: 0,
				avg_duration_ms: 125,
				min_duration_ms: 15,
				max_duration_ms: 2100,
				p50_duration_ms: 95,
				p95_duration_ms: 450,
				items_processed: 3842,
				bytes_processed: 78_652_416_000,
				throughput_per_sec: 245.8,
				error_rate_pct: 0,
			}),
		];

		// Calculate summary from task types
		const totalExecuted = byType.reduce((sum, t) => sum + t.executed, 0);
		const totalSucceeded = byType.reduce((sum, t) => sum + t.succeeded, 0);
		const totalFailed = byType.reduce((sum, t) => sum + t.failed, 0);
		const weightedDuration = byType.reduce(
			(sum, t) => sum + t.avg_duration_ms * t.executed,
			0,
		);
		const weightedQueueWait = byType.reduce(
			(sum, t) => sum + t.avg_queue_wait_ms * t.executed,
			0,
		);

		const metrics = createTaskMetrics({
			updated_at: new Date().toISOString(),
			retention: "30",
			summary: {
				total_executed: totalExecuted,
				total_succeeded: totalSucceeded,
				total_failed: totalFailed,
				avg_duration_ms:
					totalExecuted > 0 ? weightedDuration / totalExecuted : 0,
				avg_queue_wait_ms:
					totalExecuted > 0 ? weightedQueueWait / totalExecuted : 0,
				tasks_per_minute: 12.4,
			},
			queue: {
				pending_count: 23,
				processing_count: 4,
				stale_count: 0,
				oldest_pending_age_ms: 45200,
			},
			by_type: byType,
		});

		return HttpResponse.json(metrics);
	}),

	// Get task metrics history
	http.get("/api/v1/metrics/tasks/history", async ({ request }) => {
		await delay(150);
		const url = new URL(request.url);
		const days = parseInt(url.searchParams.get("days") || "7", 10);
		const taskType = url.searchParams.get("task_type");
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
				period_start: periodStart.toISOString(),
				task_type: taskType || null,
				count: Math.floor(Math.random() * 100),
				succeeded: Math.floor(Math.random() * 95),
				failed: Math.floor(Math.random() * 5),
				avg_duration_ms: Math.random() * 5000,
				min_duration_ms: Math.floor(Math.random() * 500),
				max_duration_ms: Math.floor(Math.random() * 10000),
				items_processed: Math.floor(Math.random() * 1000),
				bytes_processed: Math.floor(Math.random() * 1000000000),
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
			deleted_count: Math.floor(Math.random() * 1000),
			retention_days: "30",
			oldest_remaining: new Date(
				Date.now() - 30 * 24 * 60 * 60 * 1000,
			).toISOString(),
		});
	}),

	// Nuke all metrics
	http.delete("/api/v1/metrics/tasks", async () => {
		await delay(200);
		return HttpResponse.json({
			deleted_count: Math.floor(Math.random() * 10000),
		});
	}),
];
