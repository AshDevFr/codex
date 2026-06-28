import { afterEach, describe, expect, it, vi } from "vitest";
import { preloadImage } from "./imagePreload";

/**
 * A controllable stand-in for the browser's HTMLImageElement. jsdom ships no
 * real image loader or `decode()`, so we drive `onload` / `decode()` by hand to
 * exercise each branch of {@link preloadImage}.
 */
class FakeImage {
  src = "";
  naturalWidth = 120;
  naturalHeight = 80;
  complete = false;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  decode?: () => Promise<void>;

  constructor(decode?: () => Promise<void>) {
    this.decode = decode;
  }

  fireLoad() {
    this.complete = true;
    this.onload?.();
  }

  fireError() {
    this.onerror?.();
  }
}

const stubImage = (factory: () => FakeImage) => {
  const created: FakeImage[] = [];
  vi.stubGlobal(
    "Image",
    class {
      constructor() {
        const img = factory();
        created.push(img);
        // biome-ignore lint/correctness/noConstructorReturn: test double mimics `new Image()`
        return img as unknown as HTMLImageElement;
      }
    },
  );
  return created;
};

describe("preloadImage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("decodes the image and resolves when decode() is supported", async () => {
    const created = stubImage(() => new FakeImage(() => Promise.resolve()));

    const img = await preloadImage("/page/1.jpg");

    expect(created[0].src).toBe("/page/1.jpg");
    expect(img).toBe(created[0]);
  });

  it("falls back to onload when decode() is unavailable", async () => {
    const created = stubImage(() => new FakeImage(undefined));

    const promise = preloadImage("/page/2.jpg");
    // No decode(): resolution waits on the load event.
    created[0].fireLoad();

    await expect(promise).resolves.toBe(created[0]);
  });

  it("falls back to onload when decode() rejects before the image is complete", async () => {
    const created = stubImage(
      () => new FakeImage(() => Promise.reject(new Error("aborted"))),
    );

    const promise = preloadImage("/page/3.jpg");
    // Let the rejected decode settle, then complete the load.
    await Promise.resolve();
    created[0].fireLoad();

    await expect(promise).resolves.toBe(created[0]);
  });

  it("skips decode() and waits on load when decode is disabled", async () => {
    const decode = vi.fn(() => Promise.resolve());
    const created = stubImage(() => new FakeImage(decode));

    const promise = preloadImage("/page/5.jpg", { decode: false });
    // decode() must not be used; resolution comes from the load event.
    expect(decode).not.toHaveBeenCalled();
    created[0].fireLoad();

    await expect(promise).resolves.toBe(created[0]);
    expect(decode).not.toHaveBeenCalled();
  });

  it("rejects when the image fails to load", async () => {
    const created = stubImage(() => new FakeImage(undefined));

    const promise = preloadImage("/page/4.jpg");
    created[0].fireError();

    await expect(promise).rejects.toThrow();
  });
});
