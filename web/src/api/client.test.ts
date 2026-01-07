import { describe, it, expect, beforeEach, vi } from 'vitest';
import { api } from './client';
import { navigationService } from '@/services/navigation';

describe('API Client', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    // Mock navigation service to avoid actual navigation
    vi.spyOn(navigationService, 'navigateTo').mockImplementation(() => {});
  });

  it('should create axios instance with correct base URL', () => {
    expect(api.defaults.baseURL).toBe('/api/v1');
    expect(api.defaults.timeout).toBe(30000);
  });

  it('should add JWT token to request headers', async () => {
    const token = 'test-jwt-token';
    localStorage.setItem('jwt_token', token);

    const config = {
      headers: {},
    };

    // Access the request interceptor
    const interceptor = api.interceptors.request.handlers[0];
    const result = interceptor.fulfilled(config as any);

    expect(result.headers.Authorization).toBe(`Bearer ${token}`);
  });

  it('should not add Authorization header if no token', async () => {
    const config = {
      headers: {},
    };

    const interceptor = api.interceptors.request.handlers[0];
    const result = interceptor.fulfilled(config as any);

    expect(result.headers.Authorization).toBeUndefined();
  });

  it('should handle 401 errors and clear auth', async () => {
    const mockError = {
      response: {
        status: 401,
        data: {
          error: 'Unauthorized',
        },
      },
    };

    localStorage.setItem('jwt_token', 'token');

    // Get the response interceptor
    const interceptor = api.interceptors.response.handlers[0];

    // Mock window.location
    delete (window as any).location;
    window.location = { href: '' } as any;

    await expect(interceptor.rejected(mockError)).rejects.toEqual({
      error: 'Unauthorized',
      message: undefined,
    });

    // Verify that clearAuth was called (it removes jwt_token from localStorage)
    expect(localStorage.getItem('jwt_token')).toBeNull();
    // Verify navigation was called
    expect(navigationService.navigateTo).toHaveBeenCalledWith('/login');
  });

  it('should handle network errors', async () => {
    const mockError = {
      message: 'Network Error',
    };

    const interceptor = api.interceptors.response.handlers[0];

    await expect(interceptor.rejected(mockError)).rejects.toEqual({
      error: 'Network Error',
      message: 'Network Error',
    });
  });
});
