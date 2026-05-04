import type { components } from "@/types/api.generated";
import { api } from "./client";

export type LibraryJob = components["schemas"]["LibraryJobDto"];
export type LibraryJobConfig = components["schemas"]["LibraryJobConfigDto"];
export type MetadataRefreshJobConfig =
  components["schemas"]["MetadataRefreshJobConfigDto"];
export type CreateLibraryJobRequest =
  components["schemas"]["CreateLibraryJobRequest"];
export type DryRunRequest = components["schemas"]["DryRunRequest"];
export type DryRunResponse = components["schemas"]["DryRunResponse"];
export type FieldGroup = components["schemas"]["FieldGroupDto"];
export type RefreshScope = components["schemas"]["RefreshScope"];

/**
 * Hand-written PATCH input. The generated PatchLibraryJobRequest expects
 * `T | null` for nullable fields where we want "absent / null / value"
 * semantics. Callers omit fields they don't want to touch and pass `null`
 * to clear `timezone`.
 */
export type PatchLibraryJobInput = {
  name?: string;
  enabled?: boolean;
  cronSchedule?: string;
  timezone?: string | null;
  config?: LibraryJobConfig;
};

export const libraryJobsApi = {
  list: async (libraryId: string): Promise<LibraryJob[]> => {
    const response = await api.get<{ jobs: LibraryJob[] }>(
      `/libraries/${libraryId}/jobs`,
    );
    return response.data.jobs;
  },

  get: async (libraryId: string, jobId: string): Promise<LibraryJob> => {
    const response = await api.get<LibraryJob>(
      `/libraries/${libraryId}/jobs/${jobId}`,
    );
    return response.data;
  },

  create: async (
    libraryId: string,
    body: CreateLibraryJobRequest,
  ): Promise<LibraryJob> => {
    const response = await api.post<LibraryJob>(
      `/libraries/${libraryId}/jobs`,
      body,
    );
    return response.data;
  },

  update: async (
    libraryId: string,
    jobId: string,
    body: PatchLibraryJobInput,
  ): Promise<LibraryJob> => {
    const response = await api.patch<LibraryJob>(
      `/libraries/${libraryId}/jobs/${jobId}`,
      body,
    );
    return response.data;
  },

  delete: async (libraryId: string, jobId: string): Promise<void> => {
    await api.delete(`/libraries/${libraryId}/jobs/${jobId}`);
  },

  runNow: async (
    libraryId: string,
    jobId: string,
  ): Promise<{ taskId: string }> => {
    const response = await api.post<{ taskId: string }>(
      `/libraries/${libraryId}/jobs/${jobId}/run-now`,
    );
    return response.data;
  },

  dryRun: async (
    libraryId: string,
    jobId: string,
    body: DryRunRequest = {},
  ): Promise<DryRunResponse> => {
    const response = await api.post<DryRunResponse>(
      `/libraries/${libraryId}/jobs/${jobId}/dry-run`,
      body,
    );
    return response.data;
  },

  fieldGroups: async (): Promise<FieldGroup[]> => {
    const response = await api.get<FieldGroup[]>(
      `/library-jobs/metadata-refresh/field-groups`,
    );
    return response.data;
  },
};
