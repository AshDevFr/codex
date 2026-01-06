import axios, { type AxiosInstance } from 'axios';
import type { ApiError } from '@/types/api';

// Create axios instance with base configuration
export const api: AxiosInstance = axios.create({
  baseURL: '/api/v1',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor to add auth token
api.interceptors.request.use(
  (config) => {
    const token = localStorage.getItem('jwt_token');
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

// Response interceptor to handle errors
api.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response) {
      const apiError: ApiError = {
        error: error.response.data?.error || 'An error occurred',
        message: error.response.data?.message || error.message,
      };

      // Handle 401 Unauthorized - clear token and redirect to login
      if (error.response.status === 401) {
        localStorage.removeItem('jwt_token');
        localStorage.removeItem('user');
        window.location.href = '/login';
      }

      return Promise.reject(apiError);
    }

    return Promise.reject({
      error: 'Network Error',
      message: error.message,
    } as ApiError);
  }
);
