import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type UserDto = components["schemas"]["UserDto"];
export type CreateUserRequest = components["schemas"]["CreateUserRequest"];
export type UpdateUserRequest = components["schemas"]["UpdateUserRequest"];
export type PaginatedUsersResponse = components["schemas"]["PaginatedResponse_UserDto"];

export const usersApi = {
	/**
	 * List all users (admin only)
	 */
	list: async (): Promise<UserDto[]> => {
		const response = await api.get<UserDto[]>("/users");
		return response.data;
	},

	/**
	 * Get a single user by ID (admin only)
	 */
	get: async (userId: string): Promise<UserDto> => {
		const response = await api.get<UserDto>(`/users/${userId}`);
		return response.data;
	},

	/**
	 * Create a new user (admin only)
	 */
	create: async (request: CreateUserRequest): Promise<UserDto> => {
		const response = await api.post<UserDto>("/users", request);
		return response.data;
	},

	/**
	 * Update a user (admin only)
	 */
	update: async (userId: string, request: UpdateUserRequest): Promise<UserDto> => {
		const response = await api.patch<UserDto>(`/users/${userId}`, request);
		return response.data;
	},

	/**
	 * Delete a user (admin only)
	 */
	delete: async (userId: string): Promise<void> => {
		await api.delete(`/users/${userId}`);
	},
};
