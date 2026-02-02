import { useEffect, useState } from "react";
import { api } from "@/api/client";

/**
 * Hook to fetch an authenticated image and convert it to a blob URL
 * @param imageUrl - The API endpoint URL for the image (e.g., "/api/v1/books/{id}/thumbnail")
 * @returns The blob URL for the image, or null if not loaded yet
 */
export function useAuthenticatedImage(imageUrl: string | null): string | null {
  const [blobUrl, setBlobUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!imageUrl) {
      setBlobUrl(null);
      return;
    }

    let objectUrl: string | null = null;
    let isCancelled = false;

    // Fetch image through authenticated API client
    api
      .get(imageUrl, {
        responseType: "blob",
      })
      .then((response) => {
        if (isCancelled) return;
        const url = URL.createObjectURL(response.data);
        objectUrl = url;
        setBlobUrl(url);
      })
      .catch((error) => {
        if (isCancelled) return;
        console.error("Failed to load authenticated image:", error);
        setBlobUrl(null);
      });

    // Cleanup function
    return () => {
      isCancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  }, [imageUrl]);

  return blobUrl;
}
