# Codex Frontend

Modern React-based web interface for Codex ebook management system.

## Tech Stack

- **Framework**: React 19 + TypeScript + Vite
- **UI Library**: Mantine 7.14 (dark theme optimized)
- **State Management**:
  - Zustand (client state)
  - TanStack Query (server state)
- **Routing**: React Router v7
- **HTTP Client**: Axios
- **Icons**: Tabler Icons

## Quick Start

### Development

```bash
# Install dependencies
npm install

# Start dev server
npm run dev
```

Visit `http://localhost:5173`

### Docker Development (with backend)

```bash
# From project root
docker-compose --profile dev up
```

This starts:
- Frontend: `http://localhost:5173`
- Backend: `http://localhost:8080`
- Database: `localhost:5432`

## Project Structure

```
web/
├── src/
│   ├── api/              # API clients
│   ├── components/       # React components
│   │   ├── layout/       # AppShell, Sidebar, Header
│   │   ├── ui/           # Reusable UI components
│   │   ├── covers/       # Cover grid components
│   │   └── reader/       # Book reader components
│   ├── pages/            # Page components
│   ├── store/            # Zustand stores
│   ├── types/            # TypeScript types
│   ├── theme.ts          # Mantine theme
│   └── main.tsx          # Entry point
└── vite.config.ts
```

## Features

### Implemented ✅

- Authentication (login, JWT storage)
- Library list view with cards
- Scan library functionality
- Dark theme (Komga-inspired)
- Responsive layout
- Protected routes

### Coming Soon

- Series grid view
- Book detail view
- Reading interface
- Search & filters
- User management (admin)

## Commands

```bash
npm run dev              # Start dev server
npm run build            # Production build
npm run preview          # Preview production build
npm run lint             # Run linter
npm test                 # Run tests in watch mode
npm run test:run         # Run tests once (CI)
npm run test:ui          # Run tests with UI
npm run test:coverage    # Generate coverage report
```

## API Integration

The frontend communicates with the Rust backend at `/api/v1`:

- **Dev**: Proxied through Vite to `http://localhost:8080`
- **Production**: Served by Rust binary (embedded static files)

## Configuration

### Environment Variables

Create `.env` file:

```env
VITE_API_URL=http://localhost:8080/api/v1
```

### Theme Customization

Edit [src/theme.ts](src/theme.ts) to customize colors, spacing, and component defaults.

## Testing

The frontend has comprehensive test coverage using Vitest and React Testing Library.

**Current Test Coverage:**
- 25 tests passing (100%)
- Auth store tests
- API client tests
- Component tests (Login, Home, Sidebar)

See [TESTING.md](TESTING.md) for detailed testing guide.

## Documentation

- See [TESTING.md](TESTING.md) for testing guide
- See [../tmp/impl/FRONTEND_PLAN.md](../tmp/impl/FRONTEND_PLAN.md) for complete roadmap
- See [../tmp/impl/FRONTEND_PHASE1_COMPLETE.md](../tmp/impl/FRONTEND_PHASE1_COMPLETE.md) for Phase 1 summary
