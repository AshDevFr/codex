import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type UserDto = components["schemas"]["UserDto"];
export type CreateUserRequest = components["schemas"]["CreateUserRequest"];
export type UpdateUserRequest = components["schemas"]["UpdateUserRequest"];
export type PaginatedUsersResponse =
  components["schemas"]["PaginatedResponse_UserDto"];

/** Parameters for listing users with filtering and pagination */
export interface UserListParams {
  /** Filter by role */
  role?: "reader" | "maintainer" | "admin";
  /** Filter by sharing tag name (users who have a grant for this tag) */
  sharingTag?: string;
  /** Filter by sharing tag access mode (allow/deny) - only used with sharingTag */
  sharingTagMode?: "allow" | "deny";
  /** Page number (0-indexed) */
  page?: number;
  /** Number of items per page (max 100) */
  pageSize?: number;
}

export const usersApi = {
  /**
   * List users with pagination and filtering (admin only)
   */
  list: async (params?: UserListParams): Promise<PaginatedUsersResponse> => {
    const response = await api.get<PaginatedUsersResponse>("/users", {
      params: {
        role: params?.role,
        sharingTag: params?.sharingTag,
        sharingTagMode: params?.sharingTagMode,
        page: params?.page ?? 0,
        pageSize: params?.pageSize ?? 20,
      },
    });
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
  update: async (
    userId: string,
    request: UpdateUserRequest,
  ): Promise<UserDto> => {
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
