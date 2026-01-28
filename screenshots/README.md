# Codex Screenshot Automation

Automated screenshot capture for Codex using Playwright in Docker.

## Quick Start

1. **Add sample files** to the fixture directories:
   ```
   screenshots/fixtures/
   ├── comics/    # CBZ/CBR files
   ├── manga/     # CBZ/CBR files
   └── books/     # EPUB/PDF files
   ```

2. **Run the screenshot workflow:**
   ```bash
   make screenshots
   ```

3. **Find screenshots** in `screenshots/output/`

## Commands

| Command | Description |
|---------|-------------|
| `make screenshots` | Run full workflow (start, capture, stop) |
| `make screenshots-up` | Start the screenshot environment |
| `make screenshots-down` | Stop the screenshot environment |
| `make screenshots-run` | Run capture (requires environment running) |
| `make screenshots-logs` | View container logs |
| `make screenshots-shell` | Open shell in Playwright container |
| `make screenshots-clean` | Remove generated screenshots |

## How It Works

The screenshot system uses Docker Compose with a dedicated `screenshots` profile:

1. **PostgreSQL** - Fresh database with tmpfs (ephemeral)
2. **Backend API** - Codex server
3. **Worker** - Background task processor (for library scans)
4. **Frontend** - Vite dev server
5. **Playwright** - Browser automation

The capture script runs through several scenarios:
- Setup wizard (create admin user)
- Library creation (add libraries, wait for scans)
- Library browsing (series, books, reader)
- Settings pages (all 10+ pages)
- Navigation (dashboard, search, login)

## Screenshot Naming

Screenshots are numbered and named by scenario:

| Range | Scenario |
|-------|----------|
| 01-06 | Setup wizard |
| 07-15 | Libraries |
| 20-23 | Reader |
| 30-41 | Settings |
| 50-53 | Navigation |

## Customization

### Environment Variables

Configure via `docker-compose.yml` in the `playwright` service:

| Variable | Default | Description |
|----------|---------|-------------|
| `VIEWPORT_WIDTH` | 1280 | Screenshot width |
| `VIEWPORT_HEIGHT` | 720 | Screenshot height |
| `ADMIN_USERNAME` | admin | Setup wizard username |
| `ADMIN_EMAIL` | admin@example.com | Setup wizard email |
| `ADMIN_PASSWORD` | SecurePass123! | Setup wizard password |
| `LIBRARY_1_NAME` | Comics | First library name |
| `LIBRARY_1_PATH` | /libraries/comics | First library path |
| `LIBRARY_2_NAME` | Manga | Second library name |
| `LIBRARY_2_PATH` | /libraries/manga | Second library path |
| `LIBRARY_3_NAME` | Books | Third library name |
| `LIBRARY_3_PATH` | /libraries/books | Third library path |

### Adding New Screenshots

1. Create or modify a scenario in `scripts/scenarios/`
2. Use the `captureScreenshot(page, name)` utility
3. Run `make screenshots` to test

Example:
```typescript
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

export async function run(page, context) {
  await page.goto("/my-page");
  await waitForPageReady(page);
  await captureScreenshot(page, "my-screenshot-name");
}
```

## Troubleshooting

### Screenshots are blank or incomplete
- Increase wait times in `waitForPageReady()`
- Check if loading indicators are being detected

### Scans not completing
- Ensure fixture files are valid (not corrupted)
- Check worker logs: `make screenshots-logs`

### Container won't start
- Check if ports are in use
- Try `make screenshots-down-v` to clean volumes

### Debug mode
```bash
make screenshots-up
make screenshots-shell
# In container:
npm run capture:debug
```
