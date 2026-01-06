# Codex Frontend Setup

This document explains how the frontend is integrated into the Codex project for both development and production environments.

## Architecture

The frontend is a React 19 + TypeScript + Vite application that:

- **In Production**: Gets embedded into the Rust binary using `rust-embed`
- **In Development**: Runs as a separate Vite dev server with hot reload

## Production Build

### How It Works

1. **Frontend Build**: The Dockerfile builds the React app using Node.js
2. **Embedding**: The built files are copied and embedded into the Rust binary using `rust-embed`
3. **Serving**: The Rust server serves the static files at the root path `/` as a fallback route
4. **SPA Routing**: All non-API routes fall back to `index.html` for client-side routing

### Building for Production

```bash
# Build the Docker image (includes frontend)
docker build -t codex:latest .

# Run the production container
docker-compose up codex
```

The frontend will be accessible at `http://localhost:8080/`

### Feature Flag

The frontend embedding is controlled by the `embed-frontend` feature flag in `Cargo.toml`:

```toml
[features]
embed-frontend = []
```

When building with Docker, this feature is automatically enabled.

## Development Setup

### Option 1: Docker Compose (Recommended)

Run both backend and frontend in separate containers:

```bash
# Start development environment
docker-compose --profile dev up

# Or start individual services
docker-compose --profile dev up codex-dev frontend-dev postgres
```

This will start:

- **Backend** (codex-dev): `http://localhost:8080` - API server with hot reload
- **Frontend** (frontend-dev): `http://localhost:5173` - Vite dev server with HMR
- **Database** (postgres): `localhost:5432` - PostgreSQL database

**⚠️ Important**: Access the app at `http://localhost:5173` (frontend URL).

The Vite dev server automatically proxies `/api` and `/opds` requests to the backend at `http://localhost:8080`, so you only need to use the frontend URL.

### Option 2: Local Development

Run the backend and frontend separately on your machine:

**Terminal 1 - Backend:**

```bash
cargo run -- serve
```

**Terminal 2 - Frontend:**

```bash
cd web
npm install
npm run dev
```

Access the app at `http://localhost:5173`

### Environment Variables

The frontend uses these environment variables (configure in `web/.env`):

```env
# API base URL (defaults to http://localhost:8080 in dev)
VITE_API_URL=http://localhost:8080
```

## Frontend Structure

```
web/
├── dist/              # Build output (gitignored)
├── public/            # Static assets
├── src/
│   ├── api/          # API client and hooks
│   ├── components/   # React components
│   ├── pages/        # Page components
│   ├── hooks/        # Custom hooks
│   ├── store/        # State management
│   ├── types/        # TypeScript types
│   ├── utils/        # Utilities
│   ├── App.tsx
│   └── main.tsx
├── package.json
├── vite.config.ts
├── tsconfig.json
└── biome.json
```

## API Proxying

In development, Vite proxies requests to the backend:

```typescript
// vite.config.ts
server: {
  proxy: {
    "/api": {
      target: process.env.VITE_API_URL || "http://localhost:8080",
      changeOrigin: true,
    },
    "/opds": {
      target: process.env.VITE_API_URL || "http://localhost:8080",
      changeOrigin: true,
    },
  },
}
```

This means:

- Frontend code makes requests to `/api/v1/...` and `/opds/...`
- Vite automatically forwards them to the backend
- **Local development**: Uses `http://localhost:8080`
- **Docker development**: Uses `http://codex-dev:8080` (via `VITE_API_URL` env var)
- **No CORS issues** because requests appear to come from the same origin

## Rust Integration

### Static File Serving Module

The `src/web.rs` module handles serving the embedded frontend:

```rust
#[derive(RustEmbed)]
#[folder = "web/dist"]
#[cfg(feature = "embed-frontend")]
pub struct StaticAssets;

#[cfg(feature = "embed-frontend")]
pub async fn serve_static(uri: Uri) -> impl IntoResponse {
    // Serves files from embedded assets
    // Falls back to index.html for SPA routing
}
```

### Router Configuration

The Axum router adds the frontend as a fallback route:

```rust
// src/api/routes/mod.rs
Router::new()
    .route("/health", get(handlers::health_check))
    .nest("/opds", opds_routes())
    .nest("/api/v1", api_v1_routes())
    .fallback(get(web::serve_static))  // Frontend here
```

This ensures:

- API routes (`/api/*`, `/opds/*`) are handled first
- Everything else serves the React app
- Client-side routing works (all paths → index.html)

## Testing the Setup

### Test Production Build

```bash
# Build and run production image
docker build -t codex:latest .
docker run -p 8080:8080 codex:latest

# Visit http://localhost:8080
# Should see the React app
# API available at http://localhost:8080/api/v1
```

### Test Development Setup

```bash
# Start dev environment
docker-compose --profile dev up

# Frontend: http://localhost:5173
# Backend: http://localhost:8080
# Frontend should proxy API requests to backend
```

## Troubleshooting

### Frontend not loading in production

Check that:

1. Frontend was built: `ls web/dist/` should show files
2. Feature flag is enabled: `--features embed-frontend`
3. Router has fallback route configured

### API requests failing in dev

Check that:

1. Backend is running on port 8080
2. Vite proxy is configured correctly
3. CORS is enabled in backend config

### Docker build fails

Check that:

1. Node.js version is compatible (22+)
2. `web/package.json` exists
3. `npm ci` can install dependencies

## Next Steps

See [tmp/impl/FRONTEND_PLAN.md](tmp/impl/FRONTEND_PLAN.md) for:

- Recommended tech stack (Mantine UI + TailwindCSS)
- Implementation phases
- Component architecture
- Feature roadmap

## Resources

- **Vite Documentation**: https://vitejs.dev/
- **React 19 Docs**: https://react.dev/
- **Rust Embed**: https://github.com/pyrossh/rust-embed
- **Axum Framework**: https://docs.rs/axum/
