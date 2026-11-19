import { beforeEach, describe, expect, it } from "vitest";
import type { Envelope } from "./types";
import { cachedQuery, clearQueryCache, invalidateQuery, queryCacheSize } from "./queryCache";

function ok<T>(data: T): Envelope<T> {
  return { success: true, code: "OK", message: "ok", data, details: null, traceId: "test" };
}

describe("queryCache", () => {
  beforeEach(() => clearQueryCache());

  it("reuses a successful read inside the ttl", async () => {
    let calls = 0;
    const loader = async () => {
      calls += 1;
      return ok({ value: calls });
    };
    const first = await cachedQuery("overview:{}", loader, 500);
    const second = await cachedQuery("overview:{}", loader, 500);
    expect(first.data).toEqual({ value: 1 });
    expect(second.data).toEqual({ value: 1 });
    expect(calls).toBe(1);
  });

  it("dedupes concurrent reads", async () => {
    let calls = 0;
    const loader = async () => {
      calls += 1;
      await new Promise((resolve) => setTimeout(resolve, 20));
      return ok({ value: calls });
    };
    const [first, second] = await Promise.all([
      cachedQuery("assets:{}", loader),
      cachedQuery("assets:{}", loader),
    ]);
    expect(first.data).toEqual({ value: 1 });
    expect(second.data).toEqual({ value: 1 });
    expect(calls).toBe(1);
  });

  it("does not cache failures", async () => {
    let calls = 0;
    const loader = async () => {
      calls += 1;
      return { ...ok({}), success: false, code: "INTERNAL" } as Envelope<Record<string, never>>;
    };
    await cachedQuery("audit:{}", loader);
    await cachedQuery("audit:{}", loader);
    expect(calls).toBe(2);
    expect(queryCacheSize()).toBe(0);
  });

  it("invalidates by scope", async () => {
    await cachedQuery("assets:{}", async () => ok({ value: 1 }));
    await cachedQuery("agent.runs:{}", async () => ok({ value: 2 }));
    invalidateQuery("assets:");
    expect(queryCacheSize()).toBe(1);
    await cachedQuery("agent.runs:{}", async () => ok({ value: 2 }));
    expect(queryCacheSize()).toBe(1);
  });
});
