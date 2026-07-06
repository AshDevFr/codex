//! Process-wide bound on concurrent image decode/resize/render work.
//!
//! Every page downscale, thumbnail generation, and PDF page render decodes an
//! image into a *full uncompressed bitmap* (tens of MB for a comic scan) before
//! resizing it. The blocking-thread pool (512 threads by default) places no
//! limit on how many of those run at once, so a reader that prefetches many
//! pages can have a dozen full-resolution bitmaps resident simultaneously and
//! push the process past its memory limit.
//!
//! This module gates that work behind a semaphore so peak image memory is
//! roughly `permits * per-decode footprint` regardless of how many requests
//! arrive together. Requests beyond the cap await a permit (backpressure)
//! rather than all allocating at once; on a single core they were already
//! serialising on CPU, so the semaphore only stops them from grabbing memory
//! before their turn to run.
//!
//! The limiter is a process global (mirroring the process-wide PDFium binding)
//! so it need not be threaded through `AppState`. The core [`run_bounded_image_job`]
//! helper takes the semaphore explicitly so it can be unit-tested by injection.

use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

/// Concurrency used when [`init_image_decode_limiter`] was never called.
/// Small on purpose: each permit corresponds to one in-flight uncompressed
/// bitmap, so this is the multiplier on peak image memory. Three keeps a
/// single-core node responsive without letting a prefetch burst stack up.
const DEFAULT_DECODE_CONCURRENCY: usize = 3;

static LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Initialise the process-wide image-decode limiter with `permits` slots.
///
/// Idempotent: the first call wins and later calls are ignored, matching the
/// process-global PDFium binding. `permits` is clamped to at least 1 so a
/// misconfigured `0` cannot deadlock every image request.
pub fn init_image_decode_limiter(permits: usize) {
    let _ = LIMITER.set(Arc::new(Semaphore::new(permits.max(1))));
}

/// The process-wide image-decode limiter, lazily created with
/// [`DEFAULT_DECODE_CONCURRENCY`] if [`init_image_decode_limiter`] was never
/// called (tests and non-`serve` entrypoints).
pub fn image_decode_limiter() -> Arc<Semaphore> {
    LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(DEFAULT_DECODE_CONCURRENCY)))
        .clone()
}

/// Run a CPU/memory-heavy image job on the blocking pool, bounded by `limiter`.
///
/// At most `limiter`'s permit count of these run concurrently; the rest await a
/// permit in FIFO order. The permit is held only for the duration of the job,
/// so "permits in use" tracks "uncompressed bitmaps alive".
pub async fn run_bounded_image_job<F, T>(limiter: &Arc<Semaphore>, job: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    // Held for the whole job; dropped when the blocking task returns. Requests
    // past the cap park on this await instead of allocating a bitmap.
    let _permit = limiter
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| anyhow::anyhow!("image decode limiter closed"))?;
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| anyhow::anyhow!("image job join error: {e}"))?
}

/// Ceiling on the buffer any single image decode may allocate.
///
/// A guard against decompression bombs and absurdly large scans: a legitimate
/// comic or PDF page decodes to well under this, while a pathological image
/// errors out before it can exhaust the process. Peak memory per job is a small
/// multiple of this (resize allocates intermediates), and the semaphore bounds
/// how many jobs run at once, so worst-case image memory stays predictable.
pub const MAX_DECODE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

/// Decode an image from memory, refusing to allocate more than
/// `max_alloc_bytes`. Callers that must resize an image should decode through
/// this rather than [`image::load_from_memory`] so a single hostile or
/// oversized page cannot allocate unbounded memory even while it holds a permit.
pub fn decode_image_limited(
    data: &[u8],
    max_alloc_bytes: u64,
) -> anyhow::Result<image::DynamicImage> {
    use anyhow::Context;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .context("guess image format")?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(max_alloc_bytes);
    reader.limits(limits);
    reader
        .decode()
        .context("decode image within allocation limit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// Encode a solid RGB image of the given size to PNG bytes.
    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb([10, 20, 30]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn decode_rejects_images_exceeding_alloc_limit() {
        // 256x256 RGB decodes to ~196 KB; a 1 KB cap must reject it, a
        // generous cap must accept it.
        let bytes = png_bytes(256, 256);
        assert!(
            decode_image_limited(&bytes, 1024).is_err(),
            "a tiny allocation cap must reject the decode"
        );
        let img = decode_image_limited(&bytes, 256 * 1024 * 1024)
            .expect("a generous allocation cap must accept the decode");
        assert_eq!(img.width(), 256);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn bounds_concurrent_jobs_to_permit_count() {
        let limiter = Arc::new(Semaphore::new(2));
        let current = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let limiter = limiter.clone();
            let current = current.clone();
            let max_seen = max_seen.clone();
            handles.push(tokio::spawn(async move {
                run_bounded_image_job(&limiter, move || {
                    // Track peak simultaneous occupancy of the critical section.
                    let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                    max_seen.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(50));
                    current.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .await
                .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert!(
            max_seen.load(Ordering::SeqCst) <= 2,
            "observed {} concurrent jobs, permit cap was 2",
            max_seen.load(Ordering::SeqCst)
        );
    }
}
