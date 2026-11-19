import type { Envelope } from "./types";

type Entry<T> = {
  data: Envelope<T> | null;
  at: number;
  inflight: Promise<Envelope<T>> | null;
};

const entries = new Map<string, Entry<unknown>>();

export function queryCacheKey(name: string, args: Record<string, unknown>): string {
  return `${name}:${JSON.stringify(args)}`;
}

export async function cachedQuery<T>(
  key: string,
  loader: () => Promise<Envelope<T>>,
  ttlMs = 10_000,
): Promise<Envelope<T>> {
  const existing = entries.get(key) as Entry<T> | undefined;
  if (existing?.inflight) return existing.inflight;
  if (existing?.data && Date.now() - existing.at < ttlMs) return existing.data;

  const inflight = loader()
    .then((envelope) => {
      if (envelope.success) {
        entries.set(key, { data: envelope, at: Date.now(), inflight: null });
      } else {
        entries.delete(key);
      }
      return envelope;
    })
    .catch((cause: unknown) => {
      entries.delete(key);
      throw cause;
    });

  entries.set(key, { data: null, at: 0, inflight });
  return inflight;
}

export function invalidateQuery(prefix: string): void {
  for (const key of entries.keys()) {
    if (key.startsWith(prefix)) entries.delete(key);
  }
}

export function clearQueryCache(): void {
  entries.clear();
}

export function queryCacheSize(): number {
  return entries.size;
}
