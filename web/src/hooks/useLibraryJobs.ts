import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type CreateLibraryJobRequest,
  type DryRunRequest,
  type DryRunResponse,
  type FieldGroup,
  type LibraryJob,
  libraryJobsApi,
  type PatchLibraryJobInput,
} from "@/api/libraryJobs";

const listKey = (libraryId: string) =>
  ["library-jobs", libraryId, "list"] as const;
const detailKey = (libraryId: string, jobId: string) =>
  ["library-jobs", libraryId, jobId] as const;
const fieldGroupsKey = ["library-jobs", "field-groups"] as const;

type ApiError = Error & {
  response?: { data?: { error?: string; message?: string } };
};

const errorText = (e: ApiError) =>
  e.response?.data?.message ?? e.response?.data?.error ?? e.message;

export function useLibraryJobsList(libraryId: string) {
  return useQuery<LibraryJob[]>({
    queryKey: listKey(libraryId),
    queryFn: () => libraryJobsApi.list(libraryId),
    enabled: Boolean(libraryId),
  });
}

export function useLibraryJob(libraryId: string, jobId: string | undefined) {
  return useQuery<LibraryJob>({
    queryKey: detailKey(libraryId, jobId ?? ""),
    queryFn: () => libraryJobsApi.get(libraryId, jobId as string),
    enabled: Boolean(libraryId && jobId),
  });
}

export function useFieldGroups() {
  return useQuery<FieldGroup[]>({
    queryKey: fieldGroupsKey,
    queryFn: () => libraryJobsApi.fieldGroups(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useCreateLibraryJob(libraryId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateLibraryJobRequest) =>
      libraryJobsApi.create(libraryId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: listKey(libraryId) });
      notifications.show({
        title: "Job created",
        message: "The new library job is ready.",
        color: "green",
      });
    },
    onError: (e: ApiError) => {
      notifications.show({
        title: "Couldn't create job",
        message: errorText(e),
        color: "red",
      });
    },
  });
}

export function useUpdateLibraryJob(libraryId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { jobId: string; patch: PatchLibraryJobInput }) =>
      libraryJobsApi.update(libraryId, args.jobId, args.patch),
    onSuccess: (_, args) => {
      qc.invalidateQueries({ queryKey: listKey(libraryId) });
      qc.invalidateQueries({ queryKey: detailKey(libraryId, args.jobId) });
    },
    onError: (e: ApiError) => {
      notifications.show({
        title: "Couldn't save job",
        message: errorText(e),
        color: "red",
      });
    },
  });
}

export function useDeleteLibraryJob(libraryId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => libraryJobsApi.delete(libraryId, jobId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: listKey(libraryId) });
      notifications.show({
        title: "Job deleted",
        message: "Library job removed.",
        color: "blue",
      });
    },
    onError: (e: ApiError) => {
      notifications.show({
        title: "Couldn't delete job",
        message: errorText(e),
        color: "red",
      });
    },
  });
}

export function useRunLibraryJobNow(libraryId: string) {
  return useMutation({
    mutationFn: (jobId: string) => libraryJobsApi.runNow(libraryId, jobId),
    onSuccess: (data) => {
      notifications.show({
        title: "Job started",
        message: `Task ${data.taskId.slice(0, 8)}… enqueued.`,
        color: "blue",
      });
    },
    onError: (e: ApiError) => {
      notifications.show({
        title: "Couldn't start job",
        message: errorText(e),
        color: "red",
      });
    },
  });
}

export function useDryRunLibraryJob(libraryId: string) {
  return useMutation<
    DryRunResponse,
    ApiError,
    { jobId: string; body?: DryRunRequest }
  >({
    mutationFn: ({ jobId, body }) =>
      libraryJobsApi.dryRun(libraryId, jobId, body ?? {}),
    onError: (e: ApiError) => {
      notifications.show({
        title: "Preview failed",
        message: errorText(e),
        color: "red",
      });
    },
  });
}
