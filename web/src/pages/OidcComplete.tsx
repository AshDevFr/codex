import { Box, Center, Loader, Text } from "@mantine/core";
import { useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { useAuthStore } from "@/store/authStore";
import type { User } from "@/types";

interface OidcCallbackData {
  accessToken: string;
  tokenType: string;
  expiresIn: number;
  user: User;
  newAccount: boolean;
  provider: string;
}

function decodeFragment(): OidcCallbackData | null {
  const hash = window.location.hash.slice(1); // Remove leading '#'
  if (!hash) return null;

  try {
    // Decode URL-safe base64
    const decoded = atob(hash.replace(/-/g, "+").replace(/_/g, "/"));
    return JSON.parse(decoded) as OidcCallbackData;
  } catch {
    return null;
  }
}

export function OidcComplete() {
  const navigate = useNavigate();
  const { setAuth } = useAuthStore();
  const processed = useRef(false);

  useEffect(() => {
    // Prevent double-processing in React strict mode
    if (processed.current) return;
    processed.current = true;

    const data = decodeFragment();
    if (data) {
      // Store auth data (same as normal login)
      setAuth(data.user, data.accessToken);

      // Clear the fragment from the URL to prevent token leakage
      window.history.replaceState(null, "", "/");

      // Navigate to home
      navigate("/", { replace: true });
    } else {
      // No valid auth data - redirect to login with error
      navigate("/login?error=Authentication failed. Please try again.", {
        replace: true,
      });
    }
  }, [navigate, setAuth]);

  return (
    <Box bg="dark.7" mih="100vh">
      <Center mih="100vh">
        <div>
          <Center>
            <Loader size="lg" />
          </Center>
          <Text c="dimmed" ta="center" mt="md">
            Completing sign in...
          </Text>
        </div>
      </Center>
    </Box>
  );
}
