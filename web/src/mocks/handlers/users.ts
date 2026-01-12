/**
 * MSW handlers for user management API endpoints
 */

import { http, HttpResponse, delay } from "msw";
import { createUser, createList } from "../data/factories";

// Generate mock users
const mockUsers = [
	createUser({ id: "admin-user-id", username: "admin", email: "admin@example.com", isAdmin: true }),
	...createList(() => createUser(), 9),
];

export const usersHandlers = [
	// List all users
	http.get("/api/v1/users", async () => {
		await delay(100);
		return HttpResponse.json(mockUsers);
	}),

	// Get single user
	http.get("/api/v1/users/:userId", async ({ params }) => {
		await delay(50);
		const { userId } = params;
		const user = mockUsers.find((u) => u.id === userId);

		if (!user) {
			return new HttpResponse(null, { status: 404 });
		}

		return HttpResponse.json(user);
	}),

	// Create user
	http.post("/api/v1/users", async ({ request }) => {
		await delay(100);
		const body = await request.json() as { username: string; email: string; password: string; isAdmin?: boolean };

		const newUser = createUser({
			username: body.username,
			email: body.email,
			isAdmin: body.isAdmin ?? false,
		});

		mockUsers.push(newUser);
		return HttpResponse.json(newUser, { status: 201 });
	}),

	// Update user
	http.patch("/api/v1/users/:userId", async ({ params, request }) => {
		await delay(100);
		const { userId } = params;
		const body = await request.json() as Partial<{ username: string; email: string; isAdmin: boolean; isActive: boolean }>;

		const userIndex = mockUsers.findIndex((u) => u.id === userId);
		if (userIndex === -1) {
			return new HttpResponse(null, { status: 404 });
		}

		mockUsers[userIndex] = {
			...mockUsers[userIndex],
			...body,
			updatedAt: new Date().toISOString(),
		};

		return HttpResponse.json(mockUsers[userIndex]);
	}),

	// Delete user
	http.delete("/api/v1/users/:userId", async ({ params }) => {
		await delay(100);
		const { userId } = params;
		const userIndex = mockUsers.findIndex((u) => u.id === userId);

		if (userIndex === -1) {
			return new HttpResponse(null, { status: 404 });
		}

		mockUsers.splice(userIndex, 1);
		return new HttpResponse(null, { status: 204 });
	}),

	// Change user password
	http.post("/api/v1/users/:userId/password", async ({ params }) => {
		await delay(100);
		const { userId } = params;
		const user = mockUsers.find((u) => u.id === userId);

		if (!user) {
			return new HttpResponse(null, { status: 404 });
		}

		return HttpResponse.json({ message: "Password updated successfully" });
	}),
];
