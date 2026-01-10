/**
 * Library API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createLibrary, createList, type MockLibrary } from "../data/factories";

// In-memory mock data store
let libraries: MockLibrary[] = createList((i) => createLibrary({
  name: ["Comics", "Manga", "Ebooks", "Graphic Novels"][i % 4],
}), 4);

export const libraryHandlers = [
  // List libraries
  http.get("/api/v1/libraries", async () => {
    await delay(200);
    return HttpResponse.json(libraries);
  }),

  // Get library by ID
  http.get("/api/v1/libraries/:id", async ({ params }) => {
    await delay(100);
    const library = libraries.find((l) => l.id === params.id);

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

    libraries.push(newLibrary);
    return HttpResponse.json(newLibrary, { status: 201 });
  }),

  // Update library
  http.put("/api/v1/libraries/:id", async ({ params, request }) => {
    await delay(200);
    const body = (await request.json()) as Partial<MockLibrary>;
    const index = libraries.findIndex((l) => l.id === params.id);

    if (index === -1) {
      return HttpResponse.json({ error: "Library not found" }, { status: 404 });
    }

    libraries[index] = {
      ...libraries[index],
      ...body,
      updatedAt: new Date().toISOString(),
    };

    return HttpResponse.json(libraries[index]);
  }),

  // Delete library
  http.delete("/api/v1/libraries/:id", async ({ params }) => {
    await delay(200);
    const index = libraries.findIndex((l) => l.id === params.id);

    if (index === -1) {
      return HttpResponse.json({ error: "Library not found" }, { status: 404 });
    }

    libraries.splice(index, 1);
    return new HttpResponse(null, { status: 204 });
  }),

  // Purge deleted books
  http.post("/api/v1/libraries/:id/purge", async ({ params }) => {
    await delay(500);
    const library = libraries.find((l) => l.id === params.id);

    if (!library) {
      return HttpResponse.json({ error: "Library not found" }, { status: 404 });
    }

    return HttpResponse.json({ purgedCount: 0 });
  }),
];

// Helper to reset mock data (for testing)
export const resetMockLibraries = () => {
  libraries = createList((i) => createLibrary({
    name: ["Comics", "Manga", "Ebooks", "Graphic Novels"][i % 4],
  }), 4);
};

// Helper to get current mock libraries (for testing)
export const getMockLibraries = () => [...libraries];
