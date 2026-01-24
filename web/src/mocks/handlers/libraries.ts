/**
 * Library API mock handlers
 */

import { delay, HttpResponse, http } from "msw";
import {
	createLibrary,
	createPaginatedResponse,
	type MockLibrary,
} from "../data/factories";
import { mockLibraries } from "../data/store";

export const libraryHandlers = [
	// List libraries (paginated, 1-indexed)
	http.get("/api/v1/libraries", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const page = Math.max(
			1,
			Number.parseInt(url.searchParams.get("page") || "1", 10),
		);
		const pageSize = Number.parseInt(
			url.searchParams.get("page_size") || "50",
			10,
		);

		// 1-indexed pagination
		const start = (page - 1) * pageSize;
		const end = start + pageSize;
		const items = mockLibraries.slice(start, end);

		return HttpResponse.json(
			createPaginatedResponse(items, {
				page,
				pageSize,
				total: mockLibraries.length,
				basePath: "/api/v1/libraries",
			}),
		);
	}),

	// Get library by ID
	http.get("/api/v1/libraries/:id", async ({ params }) => {
		await delay(100);
		const library = mockLibraries.find((l) => l.id === params.id);

		if (!library) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		return HttpResponse.json(library);
	}),

	// Create library
	http.post("/api/v1/libraries", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as Partial<MockLibrary>;

		const newLibrary = createLibrary({
			name: body.name,
			path: body.path,
			description: body.description,
		});

		// Note: This won't persist across page reloads since mockLibraries
		// is re-initialized on module load
		mockLibraries.push(newLibrary);
		return HttpResponse.json(newLibrary, { status: 201 });
	}),

	// Update library (PUT - full replace)
	http.put("/api/v1/libraries/:id", async ({ params, request }) => {
		await delay(200);
		const body = (await request.json()) as Partial<MockLibrary>;
		const index = mockLibraries.findIndex((l) => l.id === params.id);

		if (index === -1) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		mockLibraries[index] = {
			...mockLibraries[index],
			...body,
			updatedAt: new Date().toISOString(),
		};

		return HttpResponse.json(mockLibraries[index]);
	}),

	// Update library (PATCH - partial update)
	http.patch("/api/v1/libraries/:id", async ({ params, request }) => {
		await delay(200);
		const body = (await request.json()) as Partial<MockLibrary>;
		const index = mockLibraries.findIndex((l) => l.id === params.id);

		if (index === -1) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		mockLibraries[index] = {
			...mockLibraries[index],
			...body,
			updatedAt: new Date().toISOString(),
		};

		return HttpResponse.json(mockLibraries[index]);
	}),

	// Delete library
	http.delete("/api/v1/libraries/:id", async ({ params }) => {
		await delay(200);
		const index = mockLibraries.findIndex((l) => l.id === params.id);

		if (index === -1) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		mockLibraries.splice(index, 1);
		return new HttpResponse(null, { status: 204 });
	}),

	// Trigger library scan
	http.post("/api/v1/libraries/:id/scan", async ({ params, request }) => {
		await delay(100);
		const library = mockLibraries.find((l) => l.id === params.id);

		if (!library) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		// Parse scan options from request body
		const body = (await request.json().catch(() => ({}))) as {
			deep?: boolean;
			force?: boolean;
		};

		return HttpResponse.json({
			message: "Scan started",
			taskId: `scan-${params.id}-${Date.now()}`,
			deep: body.deep || false,
			force: body.force || false,
		});
	}),

	// Purge deleted books (POST - legacy)
	http.post("/api/v1/libraries/:id/purge", async ({ params }) => {
		await delay(500);
		const library = mockLibraries.find((l) => l.id === params.id);

		if (!library) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		return HttpResponse.json({ purgedCount: 0 });
	}),

	// Purge deleted books (DELETE - current API)
	http.delete("/api/v1/libraries/:id/purge-deleted", async ({ params }) => {
		await delay(500);
		const library = mockLibraries.find((l) => l.id === params.id);

		if (!library) {
			return HttpResponse.json({ error: "Library not found" }, { status: 404 });
		}

		return HttpResponse.json({ purgedCount: 0 });
	}),

	// Preview scan (dry run to see what would be found)
	http.post("/api/v1/libraries/preview-scan", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as { path: string };

		// Simulate finding some files
		return HttpResponse.json({
			path: body.path,
			filesFound: 42,
			seriesEstimate: 8,
			formats: {
				cbz: 25,
				cbr: 10,
				epub: 5,
				pdf: 2,
			},
		});
	}),
];

// Helper to get current mock libraries (for testing)
export const getMockLibraries = () => [...mockLibraries];
