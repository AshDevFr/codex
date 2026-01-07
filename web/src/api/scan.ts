import { api } from './client';
import type { ScanProgress } from '@/types/api';

export const scanApi = {
  // Subscribe to scan progress updates via SSE
  subscribeToProgress: (
    onProgress: (progress: ScanProgress) => void,
    onError?: (error: Error) => void
  ): (() => void) => {
    const token = localStorage.getItem('token');
    if (!token) {
      throw new Error('Not authenticated');
    }

    const baseURL = api.defaults.baseURL || '/api/v1';
    const eventSource = new EventSource(`${baseURL}/scans/stream`, {
      withCredentials: true,
    });

    // Add auth header (EventSource doesn't support custom headers,
    // so we need to pass it via URL or use a different approach)
    // For now, we'll rely on cookie-based auth or we need to use fetch with ReadableStream

    eventSource.onmessage = (event) => {
      try {
        const progress: ScanProgress = JSON.parse(event.data);
        onProgress(progress);
      } catch (error) {
        console.error('Failed to parse scan progress:', error);
      }
    };

    eventSource.onerror = (error) => {
      console.error('SSE error:', error);
      onError?.(new Error('Connection to scan progress stream failed'));
    };

    // Return cleanup function
    return () => {
      eventSource.close();
    };
  },

  // Alternative: Use fetch with ReadableStream for better auth support
  subscribeToProgressWithAuth: async (
    onProgress: (progress: ScanProgress) => void,
    onError?: (error: Error) => void
  ): Promise<() => void> => {
    const response = await api.get('/scans/stream', {
      responseType: 'stream',
      headers: {
        'Accept': 'text/event-stream',
      },
    });

    const reader = response.data.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    const processStream = async () => {
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split('\n\n');
          buffer = lines.pop() || '';

          for (const line of lines) {
            if (line.startsWith('data: ')) {
              try {
                const data = line.substring(6);
                if (data === 'keep-alive') continue;
                const progress: ScanProgress = JSON.parse(data);
                onProgress(progress);
              } catch (error) {
                console.error('Failed to parse SSE data:', error);
              }
            }
          }
        }
      } catch (error) {
        console.error('Stream processing error:', error);
        onError?.(error as Error);
      }
    };

    processStream();

    // Return cleanup function
    return () => {
      reader.cancel();
    };
  },
};
