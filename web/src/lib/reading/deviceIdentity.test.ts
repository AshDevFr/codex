import { describe, expect, it } from "vitest";
import { getDeviceId, getDeviceName } from "./deviceIdentity";

function memoryStorage(): Storage {
  const map = new Map<string, string>();
  return {
    get length() {
      return map.size;
    },
    clear: () => map.clear(),
    getItem: (k: string) => map.get(k) ?? null,
    key: (i: number) => Array.from(map.keys())[i] ?? null,
    removeItem: (k: string) => {
      map.delete(k);
    },
    setItem: (k: string, v: string) => {
      map.set(k, v);
    },
  } as Storage;
}

describe("deviceIdentity", () => {
  it("returns the same id across calls", () => {
    const storage = memoryStorage();

    expect(getDeviceId(storage)).toBe(getDeviceId(storage));
  });

  it("persists the id so it survives a reload", () => {
    const storage = memoryStorage();
    const first = getDeviceId(storage);

    // A reload is a fresh call against the same storage.
    expect(getDeviceId(storage)).toBe(first);
    expect(storage.length).toBe(1);
  });

  it("treats separate stores as separate devices", () => {
    expect(getDeviceId(memoryStorage())).not.toBe(getDeviceId(memoryStorage()));
  });

  it("still yields an id when storage is unavailable", () => {
    // Private mode and blocked cookies must not break a reader.
    const id = getDeviceId(undefined);

    expect(id).toBeTruthy();
    expect(typeof id).toBe("string");
  });

  it("survives a storage that throws", () => {
    const throwing = {
      ...memoryStorage(),
      getItem: () => {
        throw new Error("SecurityError");
      },
    } as unknown as Storage;

    expect(() => getDeviceId(throwing)).not.toThrow();
  });

  it("reports a human-readable device name", () => {
    expect(getDeviceName()).toMatch(/^Codex Web/);
  });
});
