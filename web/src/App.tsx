import { useEffect } from "react";
import {
	BrowserRouter,
	Navigate,
	Route,
	Routes,
	useLocation,
	useNavigate,
} from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { setupApi } from "@/api/setup";
import { AppLayout } from "@/components/layout/AppLayout";
import { useEntityEvents } from "@/hooks/useEntityEvents";
import { Home } from "@/pages/Home";
import { Login } from "@/pages/Login";
import { Register } from "@/pages/Register";
import { Setup } from "@/pages/Setup";
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
		// Only redirect if setup is required and we're not already on setup page
		if (
			!isLoading &&
			setupStatus?.setupRequired &&
			location.pathname !== "/setup"
		) {
			navigate("/setup", { replace: true });
		}
	}, [setupStatus, isLoading, location.pathname, navigate]);

	return null;
}

function App() {
	const { isAuthenticated } = useAuthStore();

	// Enable real-time updates for entity changes (books, series, covers, etc.)
	useEntityEvents();

	return (
		<BrowserRouter>
			<NavigationServiceInitializer />
			<SetupRedirect />
			<Routes>
				{/* Setup route - highest priority, no auth required */}
				<Route path="/setup" element={<Setup />} />

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
