export type Theme = "light" | "dark";
export type FontMode = "preset" | "system";

export interface AppSettings {
  lang: string;
  theme: Theme;
  accent: string;
  font: FontMode;
  showIntro: boolean;
  sound: boolean;
  logo: string;
  crashEndpoint: string;
  crashAuto: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  lang: "auto",
  theme: "dark",
  accent: "mist",
  font: "preset",
  showIntro: false,
  sound: true,
  logo: "",
  crashEndpoint: "https://example.com/api/crashlog",
  crashAuto: false,
};

const KEY = "enma.settings";

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
  }
  const migrated = { ...DEFAULT_SETTINGS };
  const oldTheme = localStorage.getItem("enma.theme");
  if (oldTheme === "light" || oldTheme === "dark") migrated.theme = oldTheme;
  return migrated;
}

export function saveSettings(s: AppSettings): void {
  localStorage.setItem(KEY, JSON.stringify(s));
}

export function resetSettings(): AppSettings {
  localStorage.removeItem(KEY);
  return { ...DEFAULT_SETTINGS };
}

export function coerceSettings(v: unknown): AppSettings {
  const o = (v && typeof v === "object" ? v : {}) as Record<string, unknown>;
  return {
    lang: typeof o.lang === "string" && o.lang ? o.lang : DEFAULT_SETTINGS.lang,
    theme: o.theme === "light" ? "light" : "dark",
    accent: typeof o.accent === "string" && o.accent ? o.accent : DEFAULT_SETTINGS.accent,
    font: o.font === "system" ? "system" : "preset",
    showIntro: o.showIntro === true,
    sound: o.sound !== false,
    logo: typeof o.logo === "string" ? o.logo : DEFAULT_SETTINGS.logo,
    crashEndpoint:
      typeof o.crashEndpoint === "string" ? o.crashEndpoint : DEFAULT_SETTINGS.crashEndpoint,
    crashAuto: o.crashAuto === true,
  };
}
