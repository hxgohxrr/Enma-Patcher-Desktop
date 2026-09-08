import { createContext, useContext } from "react";

export type Vars = Record<string, string | number>;
type Dict = Record<string, unknown>;

const BASE = "en";

const bundled = import.meta.glob("./locales/*.json", { eager: true }) as Record<
  string,
  { default: Dict }
>;

const registry: Record<string, Dict> = {};
const bundledRaw: Record<string, Dict> = {};
for (const [path, mod] of Object.entries(bundled)) {
  const code = path.split("/").pop()?.replace(/\.json$/, "");
  if (code) {
    bundledRaw[code] = mod.default;
    registry[code] = mod.default;
  }
}

export function bundledLocales(): { code: string; json: string }[] {
  return Object.entries(bundledRaw).map(([code, dict]) => ({
    code,
    json: JSON.stringify(dict, null, 2),
  }));
}

export const BASE_LANG = BASE;
export const LOCALES_VERSION = 10;

function deepMerge(base: Dict, over: Dict): Dict {
  const out: Dict = { ...base };
  for (const [k, v] of Object.entries(over)) {
    const b = base[k];
    if (
      v &&
      typeof v === "object" &&
      !Array.isArray(v) &&
      b &&
      typeof b === "object" &&
      !Array.isArray(b)
    ) {
      out[k] = deepMerge(b as Dict, v as Dict);
    } else {
      out[k] = v;
    }
  }
  return out;
}

export function applyExtraLocales(list: { code: string; json: string }[]): string[] {
  const added: string[] = [];
  for (const { code, json } of list) {
    try {
      const parsed = JSON.parse(json) as Dict;
      registry[code] = deepMerge(registry[code] ?? {}, parsed);
      added.push(code);
    } catch {
    }
  }
  return added;
}

function lookup(dict: Dict | undefined, key: string): string | undefined {
  if (!dict) return undefined;
  const value = key.split(".").reduce<unknown>((acc, part) => {
    if (acc && typeof acc === "object") return (acc as Dict)[part];
    return undefined;
  }, dict);
  return typeof value === "string" ? value : undefined;
}

export function translate(lang: string, key: string, vars?: Vars): string {
  let s = lookup(registry[lang], key) ?? lookup(registry[BASE], key);
  if (s === undefined) {
    if (import.meta.env.DEV) console.warn(`[i18n] missing key: ${key}`);
    return key;
  }
  if (vars) {
    for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  }
  return s;
}

export function listLanguages(): { code: string; label: string; external: boolean }[] {
  return Object.keys(registry)
    .sort()
    .map((code) => ({
      code,
      label: lookup(registry[code], "_meta.label") ?? code,
      external: !(code in bundledRaw),
    }));
}

function flatKeys(d: Dict, prefix = ""): string[] {
  const out: string[] = [];
  for (const [k, v] of Object.entries(d)) {
    if (k === "_meta") continue;
    const full = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object" && !Array.isArray(v)) out.push(...flatKeys(v as Dict, full));
    else out.push(full);
  }
  return out;
}

export function coverage(code: string): { have: number; total: number } {
  const total = flatKeys(registry[BASE] ?? {}).length;
  const have = flatKeys(registry[code] ?? {}).length;
  return { have, total };
}

export function resolveLang(preferred: string): string {
  if (preferred && preferred !== "auto" && registry[preferred]) return preferred;
  const nav = navigator.language?.toLowerCase() ?? "";
  if (registry[nav]) return nav;
  const short = nav.split("-")[0];
  const hit = Object.keys(registry).find((c) => c.toLowerCase() === short);
  if (hit) return hit;
  return BASE;
}

export interface I18nCtx {
  lang: string;
  t: (key: string, vars?: Vars) => string;
}

export const I18nContext = createContext<I18nCtx>({
  lang: BASE,
  t: (k, v) => translate(BASE, k, v),
});

export function useT(): I18nCtx {
  return useContext(I18nContext);
}
