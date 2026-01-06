import { useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate, useNavigate } from 'react-router-dom';
import { useAuthStore } from '@/store/authStore';
import { navigationService } from '@/services/navigation';
import { AppLayout } from '@/components/layout/AppLayout';
import { Login } from '@/pages/Login';
import { Register } from '@/pages/Register';
import { Home } from '@/pages/Home';

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

function App() {
  const { isAuthenticated } = useAuthStore();

  return (
    <BrowserRouter>
      <NavigationServiceInitializer />
      <Routes>
        <Route
          path="/login"
          element={isAuthenticated ? <Navigate to="/" replace /> : <Login />}
        />

        <Route
          path="/register"
          element={isAuthenticated ? <Navigate to="/" replace /> : <Register />}
        />

        <Route
          path="/"
          element={
            <ProtectedRoute>
              <AppLayout currentPath="/">
                <Home />
              </AppLayout>
            </ProtectedRoute>
          }
        />

        <Route
          path="/libraries"
          element={
            <ProtectedRoute>
              <AppLayout currentPath="/libraries">
                <Home />
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
