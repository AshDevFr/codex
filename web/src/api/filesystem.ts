import { api } from './client';
import type { BrowseResponse, FileSystemEntry } from '@/types/api';

export const filesystemApi = {
  // Browse filesystem directories
  browse: async (path?: string): Promise<BrowseResponse> => {
    const params = path ? { path } : {};
    const response = await api.get<BrowseResponse>('/filesystem/browse', { params });
    return response.data;
  },

  // Get system drives
  getDrives: async (): Promise<FileSystemEntry[]> => {
    const response = await api.get<FileSystemEntry[]>('/filesystem/drives');
    return response.data;
  },
};

