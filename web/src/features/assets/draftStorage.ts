import { draftFromAsset, type AssetDraft } from "./model";

const DRAFT_KEY = "digilegacy.assetDraft";

export function hasMeaningfulDraft(draft: AssetDraft): boolean {
  return Object.entries(draft).some(([key, value]) => {
    if (key === "kind" || key === "valueStatus") return false;
    return typeof value === "string" && value.trim().length > 0;
  });
}

export function readAssetDraft(): AssetDraft | null {
  if (typeof window === "undefined") return null;
  const raw = window.sessionStorage.getItem(DRAFT_KEY);
  if (!raw) return null;
  try {
    const draft = draftFromAsset(JSON.parse(raw) as Partial<AssetDraft>);
    return hasMeaningfulDraft(draft) ? draft : null;
  } catch {
    window.sessionStorage.removeItem(DRAFT_KEY);
    return null;
  }
}

export function writeAssetDraft(draft: AssetDraft): void {
  if (typeof window === "undefined") return;
  if (hasMeaningfulDraft(draft)) {
    window.sessionStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
  } else {
    window.sessionStorage.removeItem(DRAFT_KEY);
  }
}

export function clearAssetDraft(): void {
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(DRAFT_KEY);
}
