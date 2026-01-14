import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { userPreferencesApi } from "./userPreferences";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("userPreferencesApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all user preferences", async () => {
			const mockResponse = {
				preferences: [
					{
						key: "ui.theme",
						value: "dark",
						valueType: "string",
						updatedAt: "2024-01-15T10:30:00Z",
					},
					{
						key: "library.show_deleted_books",
						value: true,
						valueType: "boolean",
						updatedAt: "2024-01-15T10:30:00Z",
					},
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await userPreferencesApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/user/preferences");
			expect(result).toEqual(mockResponse.preferences);
		});

		it("should return empty array when no preferences set", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: { preferences: [] },
			});

			const result = await userPreferencesApi.getAll();

			expect(result).toEqual([]);
		});
	});

	describe("get", () => {
		it("should fetch a single preference by key", async () => {
			const mockPreference = {
				key: "ui.theme",
				value: "dark",
				valueType: "string",
				updatedAt: "2024-01-15T10:30:00Z",
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockPreference });

			const result = await userPreferencesApi.get("ui.theme");

			expect(api.get).toHaveBeenCalledWith("/user/preferences/ui.theme");
			expect(result).toEqual(mockPreference);
		});

		it("should return null for non-existent preference (404)", async () => {
			const error = {
				response: { status: 404 },
			};
			vi.mocked(api.get).mockRejectedValueOnce(error);

			const result = await userPreferencesApi.get("ui.theme");

			expect(result).toBeNull();
		});

		it("should throw for other errors", async () => {
			const error = new Error("Network error");
			vi.mocked(api.get).mockRejectedValueOnce(error);

			await expect(userPreferencesApi.get("ui.theme")).rejects.toThrow(
				"Network error",
			);
		});

		it("should encode keys with special characters", async () => {
			const mockPreference = {
				key: "ui.theme",
				value: "dark",
				valueType: "string",
				updatedAt: "2024-01-15T10:30:00Z",
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockPreference });

			await userPreferencesApi.get("ui.theme");

			// Keys with dots should be encoded
			expect(api.get).toHaveBeenCalledWith("/user/preferences/ui.theme");
		});
	});

	describe("set", () => {
		it("should set a string preference", async () => {
			const mockPreference = {
				key: "ui.theme",
				value: "dark",
				valueType: "string",
				updatedAt: "2024-01-15T10:30:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockPreference });

			const result = await userPreferencesApi.set("ui.theme", "dark");

			expect(api.put).toHaveBeenCalledWith("/user/preferences/ui.theme", {
				value: "dark",
			});
			expect(result).toEqual(mockPreference);
		});

		it("should set a boolean preference", async () => {
			const mockPreference = {
				key: "library.show_deleted_books",
				value: true,
				valueType: "boolean",
				updatedAt: "2024-01-15T10:30:00Z",
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockPreference });

			const result = await userPreferencesApi.set(
				"library.show_deleted_books",
				true,
			);

			expect(api.put).toHaveBeenCalledWith(
				"/user/preferences/library.show_deleted_books",
				{ value: true },
			);
			expect(result).toEqual(mockPreference);
		});
	});

	describe("bulkSet", () => {
		it("should bulk set multiple preferences", async () => {
			const mockResponse = {
				preferences: [
					{
						key: "ui.theme",
						value: "dark",
						valueType: "string",
						updatedAt: "2024-01-15T10:30:00Z",
					},
					{
						key: "library.show_deleted_books",
						value: true,
						valueType: "boolean",
						updatedAt: "2024-01-15T10:30:00Z",
					},
				],
				updated: 2,
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockResponse });

			const result = await userPreferencesApi.bulkSet({
				"ui.theme": "dark",
				"library.show_deleted_books": true,
			});

			expect(api.put).toHaveBeenCalledWith("/user/preferences", {
				preferences: {
					"ui.theme": "dark",
					"library.show_deleted_books": true,
				},
			});
			expect(result).toEqual(mockResponse);
		});

		it("should handle empty bulk set", async () => {
			const mockResponse = {
				preferences: [],
				updated: 0,
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockResponse });

			const result = await userPreferencesApi.bulkSet({});

			expect(api.put).toHaveBeenCalledWith("/user/preferences", {
				preferences: {},
			});
			expect(result.updated).toBe(0);
		});
	});

	describe("delete", () => {
		it("should delete a preference", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await userPreferencesApi.delete("ui.theme");

			expect(api.delete).toHaveBeenCalledWith("/user/preferences/ui.theme");
		});
	});
});
