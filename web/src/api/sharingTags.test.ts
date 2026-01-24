import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { sharingTagsApi } from "./sharingTags";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("sharingTagsApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	// ============================================
	// Admin CRUD operations
	// ============================================

	describe("list", () => {
		it("should fetch all sharing tags", async () => {
			const mockTags = [
				{
					id: "tag-1",
					name: "Kids Content",
					description: "Content for children",
					seriesCount: 15,
					userCount: 3,
					createdAt: "2024-01-01T00:00:00Z",
					updatedAt: "2024-01-15T00:00:00Z",
				},
				{
					id: "tag-2",
					name: "Adult Content",
					description: null,
					seriesCount: 8,
					userCount: 1,
					createdAt: "2024-01-01T00:00:00Z",
					updatedAt: "2024-01-15T00:00:00Z",
				},
			];
			const mockResponse = {
				data: mockTags,
				page: 1,
				pageSize: 50,
				total: 2,
				totalPages: 1,
				links: {
					self: "/api/v1/admin/sharing-tags?page=1&page_size=50",
					first: "/api/v1/admin/sharing-tags?page=1&page_size=50",
					last: "/api/v1/admin/sharing-tags?page=1&page_size=50",
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await sharingTagsApi.list();

			expect(api.get).toHaveBeenCalledWith("/admin/sharing-tags");
			expect(result).toEqual(mockTags);
		});

		it("should return empty array when no sharing tags exist", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: {
					data: [],
					page: 1,
					pageSize: 50,
					total: 0,
					totalPages: 0,
					links: {
						self: "/api/v1/admin/sharing-tags?page=1&page_size=50",
						first: "/api/v1/admin/sharing-tags?page=1&page_size=50",
						last: "/api/v1/admin/sharing-tags?page=1&page_size=50",
					},
				},
			});

			const result = await sharingTagsApi.list();

			expect(result).toEqual([]);
		});
	});

	describe("get", () => {
		it("should fetch a single sharing tag by ID", async () => {
			const mockTag = {
				id: "tag-1",
				name: "Kids Content",
				description: "Content for children",
				seriesCount: 15,
				userCount: 3,
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-15T00:00:00Z",
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockTag });

			const result = await sharingTagsApi.get("tag-1");

			expect(api.get).toHaveBeenCalledWith("/admin/sharing-tags/tag-1");
			expect(result).toEqual(mockTag);
		});
	});

	describe("create", () => {
		it("should create a new sharing tag", async () => {
			const mockTag = {
				id: "tag-new",
				name: "Family Content",
				description: "Content for the whole family",
				seriesCount: 0,
				userCount: 0,
				createdAt: "2024-01-20T00:00:00Z",
				updatedAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockTag });

			const result = await sharingTagsApi.create({
				name: "Family Content",
				description: "Content for the whole family",
			});

			expect(api.post).toHaveBeenCalledWith("/admin/sharing-tags", {
				name: "Family Content",
				description: "Content for the whole family",
			});
			expect(result).toEqual(mockTag);
		});

		it("should create a sharing tag without description", async () => {
			const mockTag = {
				id: "tag-new",
				name: "Test Tag",
				description: null,
				seriesCount: 0,
				userCount: 0,
				createdAt: "2024-01-20T00:00:00Z",
				updatedAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockTag });

			const result = await sharingTagsApi.create({ name: "Test Tag" });

			expect(api.post).toHaveBeenCalledWith("/admin/sharing-tags", {
				name: "Test Tag",
			});
			expect(result).toEqual(mockTag);
		});
	});

	describe("update", () => {
		it("should update a sharing tag", async () => {
			const mockTag = {
				id: "tag-1",
				name: "Updated Name",
				description: "Updated description",
				seriesCount: 15,
				userCount: 3,
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.patch).mockResolvedValueOnce({ data: mockTag });

			const result = await sharingTagsApi.update("tag-1", {
				name: "Updated Name",
				description: "Updated description",
			});

			expect(api.patch).toHaveBeenCalledWith("/admin/sharing-tags/tag-1", {
				name: "Updated Name",
				description: "Updated description",
			});
			expect(result).toEqual(mockTag);
		});

		it("should clear description by setting to null", async () => {
			const mockTag = {
				id: "tag-1",
				name: "Kids Content",
				description: null,
				seriesCount: 15,
				userCount: 3,
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.patch).mockResolvedValueOnce({ data: mockTag });

			const result = await sharingTagsApi.update("tag-1", {
				description: null,
			});

			expect(api.patch).toHaveBeenCalledWith("/admin/sharing-tags/tag-1", {
				description: null,
			});
			expect(result).toEqual(mockTag);
		});
	});

	describe("delete", () => {
		it("should delete a sharing tag", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await sharingTagsApi.delete("tag-1");

			expect(api.delete).toHaveBeenCalledWith("/admin/sharing-tags/tag-1");
		});
	});

	// ============================================
	// Series sharing tag operations
	// ============================================

	describe("getForSeries", () => {
		it("should fetch sharing tags for a series", async () => {
			const mockTags = [
				{ id: "tag-1", name: "Kids Content", description: "For children" },
				{ id: "tag-2", name: "Family", description: null },
			];

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockTags });

			const result = await sharingTagsApi.getForSeries("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/sharing-tags");
			expect(result).toEqual(mockTags);
		});

		it("should return empty array when series has no sharing tags", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: [] });

			const result = await sharingTagsApi.getForSeries("series-123");

			expect(result).toEqual([]);
		});
	});

	describe("setForSeries", () => {
		it("should set sharing tags for a series (replacing existing)", async () => {
			const mockTags = [
				{ id: "tag-1", name: "Kids Content", description: "For children" },
				{ id: "tag-3", name: "New Tag", description: null },
			];

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockTags });

			const result = await sharingTagsApi.setForSeries("series-123", [
				"tag-1",
				"tag-3",
			]);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/sharing-tags", {
				sharingTagIds: ["tag-1", "tag-3"],
			});
			expect(result).toEqual(mockTags);
		});

		it("should clear all sharing tags when given empty array", async () => {
			vi.mocked(api.put).mockResolvedValueOnce({ data: [] });

			const result = await sharingTagsApi.setForSeries("series-123", []);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/sharing-tags", {
				sharingTagIds: [],
			});
			expect(result).toEqual([]);
		});
	});

	describe("addToSeries", () => {
		it("should add a sharing tag to a series", async () => {
			const mockTags = [
				{ id: "tag-1", name: "Kids Content", description: "For children" },
				{ id: "tag-2", name: "Added Tag", description: null },
			];

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockTags });

			const result = await sharingTagsApi.addToSeries("series-123", "tag-2");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/sharing-tags", {
				sharingTagId: "tag-2",
			});
			expect(result).toEqual(mockTags);
		});
	});

	describe("removeFromSeries", () => {
		it("should remove a sharing tag from a series", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await sharingTagsApi.removeFromSeries("series-123", "tag-1");

			expect(api.delete).toHaveBeenCalledWith(
				"/series/series-123/sharing-tags/tag-1",
			);
		});
	});

	// ============================================
	// User sharing tag grant operations
	// ============================================

	describe("getGrantsForUser", () => {
		it("should fetch sharing tag grants for a user", async () => {
			const mockResponse = {
				userId: "user-123",
				grants: [
					{
						id: "grant-1",
						sharingTagId: "tag-1",
						sharingTagName: "Kids Content",
						accessMode: "deny",
						createdAt: "2024-01-01T00:00:00Z",
					},
					{
						id: "grant-2",
						sharingTagId: "tag-2",
						sharingTagName: "Family",
						accessMode: "allow",
						createdAt: "2024-01-05T00:00:00Z",
					},
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await sharingTagsApi.getGrantsForUser("user-123");

			expect(api.get).toHaveBeenCalledWith("/users/user-123/sharing-tags");
			expect(result).toEqual(mockResponse);
		});

		it("should return empty grants array when user has no grants", async () => {
			const mockResponse = {
				userId: "user-123",
				grants: [],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await sharingTagsApi.getGrantsForUser("user-123");

			expect(result.grants).toEqual([]);
		});
	});

	describe("setGrantForUser", () => {
		it("should set a deny grant for a user", async () => {
			const mockGrant = {
				id: "grant-new",
				sharingTagId: "tag-1",
				sharingTagName: "Adult Content",
				accessMode: "deny",
				createdAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockGrant });

			const result = await sharingTagsApi.setGrantForUser(
				"user-123",
				"tag-1",
				"deny",
			);

			expect(api.put).toHaveBeenCalledWith("/users/user-123/sharing-tags", {
				sharingTagId: "tag-1",
				accessMode: "deny",
			});
			expect(result).toEqual(mockGrant);
		});

		it("should set an allow grant for a user", async () => {
			const mockGrant = {
				id: "grant-new",
				sharingTagId: "tag-2",
				sharingTagName: "Kids Content",
				accessMode: "allow",
				createdAt: "2024-01-20T00:00:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockGrant });

			const result = await sharingTagsApi.setGrantForUser(
				"user-123",
				"tag-2",
				"allow",
			);

			expect(api.put).toHaveBeenCalledWith("/users/user-123/sharing-tags", {
				sharingTagId: "tag-2",
				accessMode: "allow",
			});
			expect(result).toEqual(mockGrant);
		});
	});

	describe("removeGrantFromUser", () => {
		it("should remove a sharing tag grant from a user", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await sharingTagsApi.removeGrantFromUser("user-123", "tag-1");

			expect(api.delete).toHaveBeenCalledWith(
				"/users/user-123/sharing-tags/tag-1",
			);
		});
	});

	describe("getMyGrants", () => {
		it("should fetch current user's sharing tag grants", async () => {
			const mockResponse = {
				userId: "current-user",
				grants: [
					{
						id: "grant-1",
						sharingTagId: "tag-1",
						sharingTagName: "Kids Content",
						accessMode: "allow",
						createdAt: "2024-01-01T00:00:00Z",
					},
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await sharingTagsApi.getMyGrants();

			expect(api.get).toHaveBeenCalledWith("/user/sharing-tags");
			expect(result).toEqual(mockResponse);
		});
	});
});
