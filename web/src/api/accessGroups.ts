import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type AccessGroupDto = components["schemas"]["AccessGroupDto"];
export type AccessGroupDetailDto =
  components["schemas"]["AccessGroupDetailDto"];
export type AccessGroupMemberDto =
  components["schemas"]["AccessGroupMemberDto"];
export type AccessGroupGrantDto = components["schemas"]["AccessGroupGrantDto"];
export type AccessGroupOidcMappingDto =
  components["schemas"]["AccessGroupOidcMappingDto"];
export type AccessGroupSummaryDto =
  components["schemas"]["AccessGroupSummaryDto"];
export type CreateAccessGroupRequest =
  components["schemas"]["CreateAccessGroupRequest"];
export type UpdateAccessGroupRequest =
  components["schemas"]["UpdateAccessGroupRequest"];
export type AddAccessGroupMembersRequest =
  components["schemas"]["AddAccessGroupMembersRequest"];
export type AddAccessGroupGrantRequest =
  components["schemas"]["AddAccessGroupGrantRequest"];
export type AddAccessGroupOidcMappingRequest =
  components["schemas"]["AddAccessGroupOidcMappingRequest"];
export type EffectiveGrantsResponse =
  components["schemas"]["EffectiveGrantsResponse"];
export type EffectiveGrantDto = components["schemas"]["EffectiveGrantDto"];
export type GrantSourceDto = components["schemas"]["GrantSourceDto"];

export const accessGroupsApi = {
  // ============================================
  // Access Group CRUD (admin only)
  // ============================================

  list: async (): Promise<AccessGroupDto[]> => {
    const response = await api.get<AccessGroupDto[]>("/access-groups");
    return response.data;
  },

  get: async (groupId: string): Promise<AccessGroupDetailDto> => {
    const response = await api.get<AccessGroupDetailDto>(
      `/access-groups/${groupId}`,
    );
    return response.data;
  },

  create: async (
    request: CreateAccessGroupRequest,
  ): Promise<AccessGroupDto> => {
    const response = await api.post<AccessGroupDto>("/access-groups", request);
    return response.data;
  },

  update: async (
    groupId: string,
    request: UpdateAccessGroupRequest,
  ): Promise<AccessGroupDto> => {
    const response = await api.patch<AccessGroupDto>(
      `/access-groups/${groupId}`,
      request,
    );
    return response.data;
  },

  delete: async (groupId: string): Promise<void> => {
    await api.delete(`/access-groups/${groupId}`);
  },

  // ============================================
  // Members
  // ============================================

  addMembers: async (
    groupId: string,
    userIds: string[],
  ): Promise<AccessGroupMemberDto[]> => {
    const response = await api.post<AccessGroupMemberDto[]>(
      `/access-groups/${groupId}/members`,
      { userIds } satisfies AddAccessGroupMembersRequest,
    );
    return response.data;
  },

  removeMember: async (groupId: string, userId: string): Promise<void> => {
    await api.delete(`/access-groups/${groupId}/members/${userId}`);
  },

  // ============================================
  // Grants
  // ============================================

  addGrant: async (
    groupId: string,
    sharingTagId: string,
    accessMode: "allow" | "deny",
  ): Promise<AccessGroupGrantDto> => {
    const response = await api.post<AccessGroupGrantDto>(
      `/access-groups/${groupId}/grants`,
      { sharingTagId, accessMode } satisfies AddAccessGroupGrantRequest,
    );
    return response.data;
  },

  removeGrant: async (groupId: string, tagId: string): Promise<void> => {
    await api.delete(`/access-groups/${groupId}/grants/${tagId}`);
  },

  // ============================================
  // OIDC Mappings
  // ============================================

  addOidcMapping: async (
    groupId: string,
    oidcGroupName: string,
  ): Promise<AccessGroupOidcMappingDto> => {
    const response = await api.post<AccessGroupOidcMappingDto>(
      `/access-groups/${groupId}/oidc-mappings`,
      { oidcGroupName } satisfies AddAccessGroupOidcMappingRequest,
    );
    return response.data;
  },

  removeOidcMapping: async (
    groupId: string,
    mappingId: string,
  ): Promise<void> => {
    await api.delete(`/access-groups/${groupId}/oidc-mappings/${mappingId}`);
  },

  // ============================================
  // User queries
  // ============================================

  getForUser: async (userId: string): Promise<AccessGroupSummaryDto[]> => {
    const response = await api.get<AccessGroupSummaryDto[]>(
      `/users/${userId}/access-groups`,
    );
    return response.data;
  },

  getEffectiveGrants: async (
    userId: string,
  ): Promise<EffectiveGrantsResponse> => {
    const response = await api.get<EffectiveGrantsResponse>(
      `/users/${userId}/effective-grants`,
    );
    return response.data;
  },
};
