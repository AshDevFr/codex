# Troubleshooting

This guide helps you diagnose and fix common issues with Codex.

## Server Hanging on Page Reload / Container Restart

### Symptoms

- Frontend page takes 30-60 seconds to reload
- Docker container restarts hang for 10+ seconds
- Backend process doesn't respond to Ctrl+C immediately
- Database locks persist after shutdown

### Root Causes

This issue had **two separate root causes** that were both fixed:

#### 1. Worker Tasks Without Graceful Shutdown

**Problem:** Background worker tasks (scan workers, cleanup tasks, stale task recovery) ran in infinite loops with no shutdown mechanism. When the server received SIGTERM (Docker stop) or SIGINT (Ctrl+C), these tasks continued running until the process was forcefully killed.

**Impact:**

- Docker `stop` command waited 10 seconds before SIGKILL
- Database connections and locks weren't released cleanly
- In-progress tasks were interrupted mid-execution

**Solution:** Implemented graceful shutdown using Tokio broadcast channels:

```rust
// Worker now responds to shutdown signals
let (mut worker, shutdown_tx) = worker.with_shutdown();

tokio::select! {
    _ = shutdown_rx.recv() => {
        // Graceful shutdown
        break;
    }
    _ = process_task() => {
        // Continue work
    }
}
```

**Implementation Details:**

- Added `tokio::sync::broadcast` channel for shutdown coordination
- Main worker loop uses `tokio::select!` to listen for shutdown signals
- Background cleanup tasks also respect shutdown signals
- Server waits up to 30 seconds for worker to complete
- See `src/tasks/worker.rs` and `src/commands/serve.rs`

#### 2. Server-Sent Events (SSE) Not Detecting Disconnects

**Problem:** Real-time event streams (`/api/v1/events/stream` and `/api/v1/tasks/stream`) waited indefinitely in loops. When a client disconnected or the server restarted, these streams didn't detect the disconnect and kept waiting.

**Impact:**

- Page reloads hung waiting for old SSE connections to timeout
- Frontend reconnection attempts failed while old connections were active
- Vite dev proxy cached stale connections

**Solution:** Added timeout-based disconnect detection:

```rust
// SSE stream with timeout detection
tokio::select! {
    Ok(event) = timeout(Duration::from_secs(30), receiver.recv()) => {
        // Send event to client
    }
    Err(_) => {
        // Timeout - continue (keep-alive will trigger)
    }
}
```

**Implementation Details:**

- 30-second timeout (2x the 15-second keep-alive interval)
- Streams automatically close when broadcaster shuts down
- Proper cleanup logging when streams end
- Enhanced Vite proxy configuration for SSE handling
- See `src/api/handlers/events.rs` and `web/vite.config.ts`

### Performance Impact

| Metric           | Before Fix        | After Fix          |
| ---------------- | ----------------- | ------------------ |
| Page reload      | 40-70 seconds     | 1-2 seconds        |
| Backend restart  | 10+ seconds       | 2-5 seconds        |
| SSE disconnect   | 30-60 seconds     | < 1 second         |
| Database cleanup | Forced/incomplete | Clean and complete |

### Verification

1. **Test graceful shutdown:**

   ```bash
   # Start the dev environment
   docker-compose --profile dev up

   # In another terminal, restart should be fast
   time docker-compose --profile dev restart codex-dev
   # Should complete in 2-5 seconds
   ```

2. **Check logs for proper shutdown:**

   ```bash
   docker-compose logs codex-dev | tail -20
   ```

   You should see:

   ```
   [INFO] Received SIGTERM signal
   [INFO] Starting graceful shutdown...
   [INFO] Shutting down task worker...
   [INFO] Task worker received shutdown signal
   [INFO] Cleanup task shutting down...
   [INFO] Task worker shut down successfully
   [INFO] Shutdown complete
   ```

3. **Test page reload:**

   - Open http://localhost:5173 in your browser
   - Open browser console (F12)
   - Look for: `Entity events connection state: connected`
   - Press F5 to reload
   - Page should reload in 1-2 seconds
   - Console should show quick reconnection

4. **Test SSE reconnection:**

   ```bash
   # With dev environment running
   docker-compose --profile dev restart codex-dev

   # Watch browser console for:
   # - "Proxy error: ..." (expected during restart)
   # - "Entity events connection state: connecting"
   # - "Entity events connection state: connected" (within 5 seconds)
   ```

### Related Files

**Backend:**

- `src/tasks/worker.rs` - Worker shutdown implementation
- `src/commands/serve.rs` - Signal handling and shutdown coordination
- `src/commands/tasks.rs` - CLI worker shutdown
- `src/api/handlers/events.rs` - SSE timeout detection

**Frontend:**

- `web/vite.config.ts` - Proxy configuration for SSE
- `web/src/api/events.ts` - Entity events SSE client
- `web/src/api/tasks.ts` - Task progress SSE client

**Documentation:**

- `GRACEFUL_SHUTDOWN.md` - Technical implementation details
- `tmp/impl/COMPLETE-SOLUTION.md` - Complete problem analysis and solution

### Additional Notes

- The fix is backward compatible - no API or configuration changes needed
- Works correctly with Docker's default 10-second SIGTERM timeout
- Properly handles both SIGTERM (Docker) and SIGINT (Ctrl+C)
- Frontend SSE clients automatically reconnect with exponential backoff
- Keep-alive messages prevent premature connection timeout

## Database Connection Issues

### Symptoms

- Connection timeout errors
- "Too many connections" errors
- Slow query performance

### Solutions

1. **Check database health:**

   ```bash
   docker-compose exec postgres pg_isready -U codex
   ```

2. **Verify connection settings:**

   ```yaml
   # config/config.docker.yaml
   database:
     db_type: postgres
     postgres:
       host: postgres
       port: 5432
       # ... other settings
   ```

3. **Restart database container:**

   ```bash
   docker-compose restart postgres
   ```

4. **Check for connection leaks:**
   ```bash
   docker-compose exec postgres psql -U codex -c \
     "SELECT count(*) FROM pg_stat_activity WHERE datname = 'codex';"
   ```

## Worker Task Issues

### Tasks Not Processing

1. **Check worker status:**

   ```bash
   # Look for worker startup messages
   docker-compose logs codex-dev | grep "Task worker"
   ```

2. **Check task queue:**

   ```sql
   SELECT status, count(*) FROM tasks GROUP BY status;
   ```

3. **Check for stale locks:**
   ```sql
   SELECT * FROM tasks
   WHERE status = 'processing'
   AND locked_until < NOW();
   ```

### Tasks Failing

1. **View task errors:**

   ```bash
   docker-compose logs codex-dev | grep "Task.*failed"
   ```

2. **Check specific task:**
   Use the API to get task details:
   ```bash
   curl -H "Authorization: Bearer <token>" \
     http://localhost:8080/api/v1/tasks/<task-id>
   ```

## Frontend Build Issues

### Vite Proxy Errors

1. **Check backend is running:**

   ```bash
   curl http://localhost:8080/health
   ```

2. **Verify proxy configuration:**

   ```typescript
   // web/vite.config.ts
   proxy: {
     "/api": {
       target: process.env.VITE_API_URL || "http://localhost:8080",
       changeOrigin: true,
     },
   }
   ```

3. **Check Vite logs:**
   ```bash
   docker-compose logs frontend-dev
   ```

### SSE Connection Issues

If real-time updates aren't working:

1. **Check authentication:**

   - Open browser DevTools → Application → Local Storage
   - Verify `jwt_token` exists and is valid

2. **Check network tab:**

   - Open DevTools → Network
   - Filter for "stream"
   - Should see `/api/v1/events/stream` with status 200
   - Connection should stay open (pending)

3. **Check console for errors:**

   - Look for "SSE connection failed" messages
   - Look for connection state changes

4. **Manually test SSE endpoint:**
   ```bash
   curl -H "Authorization: Bearer <token>" \
        -H "Accept: text/event-stream" \
        http://localhost:8080/api/v1/events/stream
   ```

## Docker Issues

### Containers Won't Start

1. **Check for port conflicts:**

   ```bash
   lsof -i :5173  # Frontend
   lsof -i :8080  # Backend
   lsof -i :5432  # Postgres
   ```

2. **Clear volumes and restart:**

   ```bash
   docker-compose down -v
   docker-compose --profile dev up
   ```

3. **Check Docker resources:**
   - Ensure Docker has enough memory (recommend 4GB+)
   - Check disk space

### Containers Keep Restarting

1. **Check logs for errors:**

   ```bash
   docker-compose logs --tail=100
   ```

2. **Check health status:**

   ```bash
   docker-compose ps
   ```

3. **Verify dependencies:**
   - Backend depends on postgres being healthy
   - Frontend depends on backend being available

## Performance Issues

### Slow Scans

1. **Check concurrent scan limit:**

   ```sql
   SELECT value FROM settings WHERE key = 'scanner.max_concurrent_scans';
   ```

2. **Adjust via API:**
   ```bash
   curl -X PUT http://localhost:8080/api/v1/admin/settings/scanner.max_concurrent_scans \
     -H "Authorization: Bearer <token>" \
     -d '{"value": "4"}'
   ```

### High Memory Usage

1. **Check worker concurrency:**

   - Reduce `scanner.max_concurrent_scans`
   - Reduce task worker count

2. **Monitor container resources:**
   ```bash
   docker stats
   ```

## Getting Help

If you're still experiencing issues:

1. **Enable debug logging:**

   ```yaml
   # config/config.docker.yaml or environment variable
   CODEX_LOGGING_LEVEL: debug
   ```

2. **Collect logs:**

   ```bash
   docker-compose logs > codex-logs.txt
   ```

3. **Check for known issues:**

   - Review GitHub issues
   - Check recent commits for fixes

4. **Provide details when reporting:**
   - Codex version
   - Docker/system info
   - Complete error messages
   - Steps to reproduce
   - Relevant configuration
