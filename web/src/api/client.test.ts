import { describe, it, expect, beforeEach } from 'vitest';
import { api } from './client';

describe('API Client', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
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

  it('should handle 401 errors and clear auth', () => {
    const mockError = {
      response: {
        status: 401,
        data: {
          error: 'Unauthorized',
        },
      },
    };

    localStorage.setItem('jwt_token', 'token');
    localStorage.setItem('user', JSON.stringify({ id: '1' }));

    // Get the response interceptor
    const interceptor = api.interceptors.response.handlers[0];

    // Mock window.location
    delete (window as any).location;
    window.location = { href: '' } as any;

    expect(() => interceptor.rejected(mockError)).rejects.toEqual({
      error: 'Unauthorized',
      message: undefined,
    });

    expect(localStorage.getItem('jwt_token')).toBeNull();
    expect(localStorage.getItem('user')).toBeNull();
  });

  it('should handle network errors', () => {
    const mockError = {
      message: 'Network Error',
    };

    const interceptor = api.interceptors.response.handlers[0];

    expect(() => interceptor.rejected(mockError)).rejects.toEqual({
      error: 'Network Error',
      message: 'Network Error',
    });
  });
});
