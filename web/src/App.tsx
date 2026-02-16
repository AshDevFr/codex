import { notifications } from "@mantine/notifications";
import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import {
  BrowserRouter,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from "react-router-dom";
import { onRateLimitNotification } from "@/api/client";
import { setupApi } from "@/api/setup";
import { AppLayout } from "@/components/layout/AppLayout";
import { useEntityEvents } from "@/hooks/useEntityEvents";
import { BookDetail } from "@/pages/BookDetail";
import { Home } from "@/pages/Home";
import { LibraryPage } from "@/pages/Library";
import { Login } from "@/pages/Login";
import { OidcComplete } from "@/pages/OidcComplete";
import { Reader } from "@/pages/Reader";
import { Recommendations } from "@/pages/Recommendations";
import { Register } from "@/pages/Register";
import { SearchResults } from "@/pages/SearchResults";
import { SeriesDetail } from "@/pages/SeriesDetail";
import { Setup } from "@/pages/Setup";
import {
  BooksInErrorSettings,
  CleanupSettings,
  DuplicatesSettings,
  IntegrationsSettings,
  MetricsSettings,
  PdfCacheSettings,
  PluginStorageSettings,
  PluginsSettings,
  ProfileSettings,
  ServerSettings,
  SharingTagsSettings,
  TasksSettings,
  UsersSettings,
} from "@/pages/settings";
import { navigationService } from "@/services/navigation";
import { useAuthStore } from "@/store/authStore";

// Protected route wrapper
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuthStore();

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

// Component to initialize navigation service
function NavigationServiceInitializer() {
  const navigate = useNavigate();

  useEffect(() => {
    navigationService.setNavigate(navigate);
  }, [navigate]);

  return null;
}

// Component to handle rate limit notifications
function RateLimitNotificationHandler() {
  useEffect(() => {
    // Register handler for rate limit notifications
    onRateLimitNotification((retryAfterSeconds) => {
      notifications.show({
        id: "rate-limit-warning",
        title: "Slow down",
        message: `Too many requests. Retrying in ${retryAfterSeconds} seconds...`,
        color: "yellow",
        autoClose: retryAfterSeconds * 1000,
      });
    });

    // Cleanup handler on unmount
    return () => {
      onRateLimitNotification(null);
    };
  }, []);

  return null;
}

// Setup redirect component - redirects to /setup if needed
function SetupRedirect() {
  const navigate = useNavigate();
  const location = useLocation();

  // Check setup status
  const { data: setupStatus, isLoading } = useQuery({
    queryKey: ["setup-status"],
    queryFn: setupApi.checkStatus,
    retry: 1,
  });

  useEffect(() => {
    if (isLoading) return;

    // Redirect to setup if required and not already on setup page
    if (setupStatus?.setupRequired && location.pathname !== "/setup") {
      navigate("/setup", { replace: true });
    }
    // Redirect away from setup if already complete
    else if (!setupStatus?.setupRequired && location.pathname === "/setup") {
      navigate("/", { replace: true });
    }
  }, [setupStatus, isLoading, location.pathname, navigate]);

  return null;
}

function App() {
  const { isAuthenticated } = useAuthStore();

  // Enable real-time updates for entity changes (books, series, covers, etc.)
  // Hook handles authentication check internally
  useEntityEvents();

  return (
    <BrowserRouter>
      <NavigationServiceInitializer />
      <RateLimitNotificationHandler />
      <SetupRedirect />
      <Routes>
        {/* Setup route - highest priority, no auth required */}
        <Route path="/setup" element={<Setup />} />

        <Route
          path="/login"
          element={isAuthenticated ? <Navigate to="/" replace /> : <Login />}
        />

        {/* OIDC callback completion - processes auth data from URL fragment */}
        <Route path="/login/oidc/complete" element={<OidcComplete />} />

        <Route
          path="/register"
          element={isAuthenticated ? <Navigate to="/" replace /> : <Register />}
        />

        <Route
          path="/"
          element={
            <ProtectedRoute>
              <AppLayout>
                <Home />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/recommendations"
          element={
            <ProtectedRoute>
              <AppLayout>
                <Recommendations />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/libraries"
          element={
            <ProtectedRoute>
              <AppLayout>
                <Home />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/libraries/:libraryId/*"
          element={
            <ProtectedRoute>
              <AppLayout>
                <LibraryPage />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        {/* Series detail page */}
        <Route
          path="/series/:seriesId"
          element={
            <ProtectedRoute>
              <AppLayout>
                <SeriesDetail />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        {/* Book detail page */}
        <Route
          path="/books/:bookId"
          element={
            <ProtectedRoute>
              <AppLayout>
                <BookDetail />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        {/* Reader page - fullscreen, no AppLayout */}
        <Route
          path="/reader/:bookId"
          element={
            <ProtectedRoute>
              <Reader />
            </ProtectedRoute>
          }
        />

        {/* Search results page */}
        <Route
          path="/search"
          element={
            <ProtectedRoute>
              <AppLayout>
                <SearchResults />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        {/* Settings routes */}
        <Route
          path="/settings/integrations"
          element={
            <ProtectedRoute>
              <AppLayout>
                <IntegrationsSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/profile"
          element={
            <ProtectedRoute>
              <AppLayout>
                <ProfileSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/server"
          element={
            <ProtectedRoute>
              <AppLayout>
                <ServerSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/users"
          element={
            <ProtectedRoute>
              <AppLayout>
                <UsersSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/sharing-tags"
          element={
            <ProtectedRoute>
              <AppLayout>
                <SharingTagsSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/plugins"
          element={
            <ProtectedRoute>
              <AppLayout>
                <PluginsSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/tasks"
          element={
            <ProtectedRoute>
              <AppLayout>
                <TasksSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/duplicates"
          element={
            <ProtectedRoute>
              <AppLayout>
                <DuplicatesSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/cleanup"
          element={
            <ProtectedRoute>
              <AppLayout>
                <CleanupSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/pdf-cache"
          element={
            <ProtectedRoute>
              <AppLayout>
                <PdfCacheSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/plugin-storage"
          element={
            <ProtectedRoute>
              <AppLayout>
                <PluginStorageSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/metrics"
          element={
            <ProtectedRoute>
              <AppLayout>
                <MetricsSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/settings/book-errors"
          element={
            <ProtectedRoute>
              <AppLayout>
                <BooksInErrorSettings />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
