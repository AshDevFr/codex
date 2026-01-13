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
import { setupApi } from "@/api/setup";
import { AppLayout } from "@/components/layout/AppLayout";
import { useEntityEvents } from "@/hooks/useEntityEvents";
import { BookDetail } from "@/pages/BookDetail";
import { Home } from "@/pages/Home";
import { LibraryPage } from "@/pages/Library";
import { Login } from "@/pages/Login";
import { Reader } from "@/pages/Reader";
import { Register } from "@/pages/Register";
import { SearchResults } from "@/pages/SearchResults";
import { SeriesDetail } from "@/pages/SeriesDetail";
import { Setup } from "@/pages/Setup";
import {
	DuplicatesSettings,
	MetricsSettings,
	ProfileSettings,
	ServerSettings,
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

				<Route
					path="/libraries/:libraryId/*"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/libraries/:libraryId">
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
							<AppLayout currentPath="/series/:seriesId">
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
							<AppLayout currentPath="/books/:bookId">
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
							<AppLayout currentPath="/search">
								<SearchResults />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				{/* Settings routes */}
				<Route
					path="/settings/profile"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/profile">
								<ProfileSettings />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				<Route
					path="/settings/server"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/server">
								<ServerSettings />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				<Route
					path="/settings/users"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/users">
								<UsersSettings />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				<Route
					path="/settings/tasks"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/tasks">
								<TasksSettings />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				<Route
					path="/settings/duplicates"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/duplicates">
								<DuplicatesSettings />
							</AppLayout>
						</ProtectedRoute>
					}
				/>

				<Route
					path="/settings/metrics"
					element={
						<ProtectedRoute>
							<AppLayout currentPath="/settings/metrics">
								<MetricsSettings />
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
