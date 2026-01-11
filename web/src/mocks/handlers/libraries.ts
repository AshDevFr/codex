/**
 * Library API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createLibrary, type MockLibrary } from "../data/factories";
import { mockLibraries } from "../data/store";

export const libraryHandlers = [
  // List libraries
  http.get("/api/v1/libraries", async () => {
    await delay(200);
    return HttpResponse.json(mockLibraries);
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

  // Update library
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

  // Purge deleted books
  http.post("/api/v1/libraries/:id/purge", async ({ params }) => {
    await delay(500);
    const library = mockLibraries.find((l) => l.id === params.id);

    if (!library) {
      return HttpResponse.json({ error: "Library not found" }, { status: 404 });
    }

    return HttpResponse.json({ purgedCount: 0 });
  }),
];

// Helper to get current mock libraries (for testing)
export const getMockLibraries = () => [...mockLibraries];
