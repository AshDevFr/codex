import { api } from './client';
import type { Library } from '@/types/api';

export const librariesApi = {
  // Get all libraries
  getAll: async (): Promise<Library[]> => {
    const response = await api.get<Library[]>('/libraries');
    return response.data;
  },

  // Get a single library by ID
  getById: async (id: string): Promise<Library> => {
    const response = await api.get<Library>(`/libraries/${id}`);
    return response.data;
  },

  // Create a new library
  create: async (library: Omit<Library, 'id' | 'created_at' | 'updated_at'>): Promise<Library> => {
    const response = await api.post<Library>('/libraries', library);
    return response.data;
  },

  // Update a library
  update: async (id: string, library: Partial<Library>): Promise<Library> => {
    const response = await api.put<Library>(`/libraries/${id}`, library);
    return response.data;
  },

  // Delete a library
  delete: async (id: string): Promise<void> => {
    await api.delete(`/libraries/${id}`);
  },

  // Trigger a scan
  scan: async (id: string): Promise<void> => {
    await api.post(`/libraries/${id}/scan`);
  },
};
