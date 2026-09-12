import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import {
  Layers,
  UserRound,
  Wrench,
  PanelLeftClose,
  PanelLeftOpen,
  SlidersHorizontal,
} from "lucide-react";
import { animate } from "motion";
import logoUrl from "./assets/enma-logo.png";
import { ModSpec, ModInfo, api, modInfoKey } from "./lib/tauri";
import {
  configureCrashReporting,
  installCrashHandlers,
  loadPersistedCrashLog,
} from "./lib/crashlog";
import { AppSettings, coerceSettings, loadSettings, saveSettings } from "./lib/settings";
import { setSoundEnabled, setMelody, sound, unlockAudio } from "./lib/sound";
import {
  I18nContext,
  LOCALES_VERSION,
  applyExtraLocales,
  bundledLocales,
  resolveLang,
  translate,
  type Vars,
} from "./i18n";
import accents from "./config/accents.json";
import { AndroidView } from "./views/AndroidView";
import { IosView } from "./views/IosView";
import { Intro } from "./components/Intro";
import { Enter } from "./components/Enter";
import { UpdateBanner } from "./components/Updater";
import { ModsEditor } from "./components/ModsEditor";
import { AccountView } from "./views/AccountView";
import { ToolsView } from "./views/ToolsView";
import { SettingsView } from "./views/SettingsView";
import { Onboarding } from "./components/Onboarding";
import { Titlebar } from "./components/Titlebar";
import { DiscordFab } from "./components/DiscordFab";
import { SectionTitle } from "./components/ui";

type Tab = "android" | "ios" | "mods" | "cuenta" | "tools" | "settings";

import { AndroidMark, AppleMark, type Mark } from "./components/marks";
import { convertFileSrc } from "@tauri-apps/api/core";

const ModsMark: Mark = (p) => <Layers size={p.size ?? 17} className={p.className} />;
const AccountMark: Mark = (p) => <UserRound size={p.size ?? 17} className={p.className} />;
const ToolsMark: Mark = (p) => <Wrench size={p.size ?? 17} className={p.className} />;

const TABS: { id: Tab; labelKey: string; support?: string; icon: Mark }[] = [
  { id: "android", labelKey: "nav.android", support: ".apks · .apk · .drmb", icon: AndroidMark },
  { id: "ios", labelKey: "nav.ios", support: ".ipa", icon: AppleMark },
  { id: "mods", labelKey: "nav.mods", icon: ModsMark },
  { id: "cuenta", labelKey: "nav.account", icon: AccountMark },
  { id: "tools", labelKey: "nav.tools", icon: ToolsMark },
];

interface AccentDef {
  id: string;
  light: Record<string, string>;
  dark: Record<string, string>;
}

const ACCENT_CSS: [string, string][] = [
  ["primary", "--primary"],
  ["primaryFg", "--primary-foreground"],
  ["ring", "--ring"],
  ["sidebarPrimary", "--sidebar-primary"],
];

function shouldPlayIntro(showEachLaunch: boolean): boolean {  try {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return false;
    if (showEachLaunch) return true;
    return !localStorage.getItem("enma.introSeen");
  } catch {
    return false;
  }
}

function TabPanel(props: { active: boolean; children: ReactNode }) {
  const [visible, setVisible] = useState(props.active);
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    if (props.active) {
      setVisible(true);
      setLeaving(false);
      return;
    }
    if (!visible) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setVisible(false);
      return;
    }
    setLeaving(true);
    const t = window.setTimeout(() => {
      setVisible(false);
      setLeaving(false);
    }, 150);
    return () => window.clearTimeout(t);
  }, [props.active]);

  if (!visible) {
    return (
      <div style={{ display: "none" }} aria-hidden="true">
        {props.children}
      </div>
    );
  }
  if (leaving) {
    return <div className="tab-out tab-leaving" aria-hidden="true">{props.children}</div>;
  }
  return <div className="tab-in">{props.children}</div>;
}

function loadMods(): ModSpec[] {
  try {
    const raw = localStorage.getItem("enma.mods");
    if (raw) {
      const arr = JSON.parse(raw);
      if (Array.isArray(arr)) return arr;
    }
  } catch {
  }
  return [{ kind: "github", repo: "ChipLG08/YW1MESP", branch: "main", path: "", enabled: true }];
}

export default function App() {
  const [tab, setTab] = useState<Tab>("android");
  const [mods, setMods] = useState<ModSpec[]>(loadMods);
  const [settings, setSettings] = useState<AppSettings>(loadSettings);
  const [extraTick, setExtraTick] = useState(0);
  const [lastIpa, setLastIpa] = useState<string | null>(null);
  const [tourOpen, setTourOpen] = useState(false);
  const [introOpen, setIntroOpen] = useState(() => shouldPlayIntro(settings.showIntro));
  const [entered, setEntered] = useState(() => !shouldPlayIntro(settings.showIntro));
  const [crashDir, setCrashDir] = useState<string | null>(null);
  const [modInfos, setModInfos] = useState<Record<string, ModInfo>>({});
  const hydratedRef = useRef(false);

  const lang = useMemo(() => resolveLang(settings.lang), [settings.lang, extraTick]);
  const t = useMemo(
    () => (key: string, vars?: Vars) => translate(lang, key, vars),
    [lang]
  );
  const i18n = useMemo(() => ({ lang, t }), [lang, t]);

  function patchSettings(patch: Partial<AppSettings>) {
    setSettings((s) => ({ ...s, ...patch }));
  }

  useEffect(() => {
    saveSettings(settings);
    if (!hydratedRef.current) return;
    void api.saveUserSettings(JSON.stringify(settings)).catch(() => {});
  }, [settings]);

  useEffect(() => {
    setSoundEnabled(settings.sound);
  }, [settings.sound]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", settings.theme === "dark");
  }, [settings.theme]);

  useEffect(() => {
    const unlock = () => unlockAudio();
    window.addEventListener("pointerdown", unlock);
    window.addEventListener("keydown", unlock);
    return () => {
      window.removeEventListener("pointerdown", unlock);
      window.removeEventListener("keydown", unlock);
    };
  }, []);

  function switchTab(id: Tab) {
    if (id !== tab) {
      sound.tab();
      setTab(id);
    }
    setMelody(id === "ios" ? "ios" : id === "android" ? "android" : "default");
  }

  useEffect(() => {
    setMelody(tab === "ios" ? "ios" : tab === "android" ? "android" : "default");
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("lang", lang);
  }, [lang]);

  useEffect(() => {
    document.documentElement.classList.toggle("font-system", settings.font === "system");
  }, [settings.font]);

  useEffect(() => {
    const root = document.documentElement;
    const acc = (accents as AccentDef[]).find((a) => a.id === settings.accent);
    const vars = (settings.theme === "dark" ? acc?.dark : acc?.light) ?? {};
    for (const [k, css] of ACCENT_CSS) {
      const v = vars[k];
      if (v) root.style.setProperty(css, v);
      else root.style.removeProperty(css);
    }
  }, [settings.accent, settings.theme]);

  useEffect(() => {
    let cancelled = false;
    const missing = mods.filter((m) => m.enabled && !modInfos[modInfoKey(m)]);
    if (missing.length === 0) return;
    void (async () => {
      for (const m of missing) {
        try {
          const info = await api.modInfo(m);
          if (cancelled) return;
          setModInfos((prev) => ({ ...prev, [modInfoKey(m)]: info }));
        } catch {
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [mods, modInfos]);

  async function refreshModInfo(m: ModSpec): Promise<ModInfo> {
    const info = await api.modInfo(m);
    setModInfos((prev) => ({ ...prev, [modInfoKey(m)]: info }));
    return info;
  }

  useEffect(() => {
    installCrashHandlers();
    let cancelled = false;
    void api
      .appDirs()
      .then(async (d) => {
        if (cancelled) return;
        setCrashDir(d.data);
        await loadPersistedCrashLog(d.data);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    configureCrashReporting({
      dataDir: crashDir,
      endpoint: settings.crashEndpoint,
      auto: settings.crashAuto,
    });
  }, [crashDir, settings.crashEndpoint, settings.crashAuto]);

  useEffect(() => {
    localStorage.setItem("enma.mods", JSON.stringify(mods));
  }, [mods]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await api.seedLocales(bundledLocales(), LOCALES_VERSION);
        const disk = await api.extraLocales();
        if (!cancelled && applyExtraLocales(disk).length > 0) setExtraTick((x) => x + 1);
      } catch {
      }
      try {
        const file = await api.loadUserSettings();
        if (!cancelled && file) setSettings(coerceSettings(file));
      } catch {
      }
      hydratedRef.current = true;
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const NAV_W = 240;
  const [navOpen, setNavOpen] = useState(
    () => localStorage.getItem("enma.sidebar") !== "closed"
  );
  const [tip, setTip] = useState<{ text: string; top: number; left: number } | null>(null);
  const asideRef = useRef<HTMLElement | null>(null);
  const widthRef = useRef<number>(NAV_W);
  const velRef = useRef<number>(0);
  const sampleRef = useRef<{ t: number; v: number } | null>(null);
  const animRef = useRef<{ stop: () => void } | null>(null);
  const navOpenRef = useRef(navOpen);
  navOpenRef.current = navOpen;

  function paintNav(el: HTMLElement, w: number) {
    widthRef.current = w;
    el.style.width = `${w}px`;
    const alpha = Math.max(0, Math.min(1, (w - 70) / 80));
    el.style.setProperty("--nav-alpha", alpha.toFixed(3));
  }

  function setNav(open: boolean) {
    navOpenRef.current = open;
    setNavOpen(open);
    setTip(null);
    localStorage.setItem("enma.sidebar", open ? "open" : "closed");
    const el = asideRef.current;
    if (!el) return;
    animRef.current?.stop();
    animRef.current = null;
    const from = widthRef.current;
    const to = open ? NAV_W : 0;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (from === to || reduced) {
      velRef.current = 0;
      sampleRef.current = null;
      paintNav(el, to);
      return;
    }
    animRef.current = animate(from, to, {
      type: "spring",
      bounce: 0,
      duration: 0.3,
      velocity: velRef.current,
      onUpdate: (v: number) => {
        const now = performance.now();
        const prev = sampleRef.current;
        if (prev && now > prev.t) velRef.current = ((v - prev.v) / (now - prev.t)) * 1000;
        sampleRef.current = { t: now, v };
        paintNav(el, v);
      },
      onComplete: () => {
        velRef.current = 0;
        sampleRef.current = null;
        animRef.current = null;
      },
    });
  }

  useEffect(() => {
    const el = asideRef.current;
    if (el) paintNav(el, navOpenRef.current ? NAV_W : 0);
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
        const target = e.target as HTMLElement | null;
        if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
        e.preventDefault();
        setNav(!navOpenRef.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const activeMods = mods.filter((m) => m.enabled).length;

  useEffect(() => {
    let id: number | undefined;
    try {
      if (!localStorage.getItem("enma.onboarded")) {
        id = window.setTimeout(() => setTourOpen(true), 900);
      }
    } catch {
    }
    return () => window.clearTimeout(id);
  }, []);

  function closeTour() {
    try {
      localStorage.setItem("enma.onboarded", "1");
    } catch {
    }
    setTourOpen(false);
  }

  function finishIntro() {
    try {
      localStorage.setItem("enma.introSeen", "1");
    } catch {
    }
    setIntroOpen(false);
    setEntered(true);
  }

  return (
    <I18nContext.Provider value={i18n}>
      <div className="flex h-full bg-ink-50 text-ink-900 dark:bg-ink-950 dark:text-ink-100">
        <aside
            ref={asideRef}
            style={{ width: NAV_W }}
            className="shrink-0 overflow-hidden bg-white dark:bg-ink-900"
          >
            <div
              className="flex h-full w-60 flex-col border-r border-ink-200/80 dark:border-white/10"
              style={{ opacity: "var(--nav-alpha, 1)" }}
            >
              <Enter play={entered} delay={0}>
              <div className="flex items-center gap-2.5 px-5 pb-4 pt-5">
                <img
                  src={settings.logo ? convertFileSrc(settings.logo) : logoUrl}
                  alt="Enma Patcher"
                  className="h-8 w-8 shrink-0 rounded-lg object-cover shadow-card"
                  draggable={false}
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold tracking-tight leading-none">Enma Patcher</p>
                  <p className="mt-1 text-[11px] text-ink-400">{t("app.tag")}</p>
                </div>
                <button
                  onClick={() => setNav(false)}
                  title={`${t("app.hideMenu")} (Ctrl+B)`}
                  className="pressable shrink-0 rounded-lg p-1.5 text-ink-400 hover:bg-ink-100 hover:text-ink-700 dark:hover:bg-white/10 dark:hover:text-ink-100"
                >
                  <PanelLeftClose size={16} />
                </button>
              </div>
              </Enter>

              <nav className="flex-1 space-y-1.5 px-3">
                {TABS.map((item, i) => {
                  const Icon = item.icon;
                  const active = tab === item.id;
                  return (
                    <Enter key={item.id} play={entered} delay={0.05 + i * 0.05}>
                    <button
                      data-tour={`nav-${item.id}`}
                      onClick={() => switchTab(item.id)}
                      onMouseEnter={(e) => {
                        if (!item.support) return;
                        const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        setTip({ text: item.support, top: r.top + r.height / 2, left: r.right + 10 });
                      }}
                      onMouseLeave={() => setTip(null)}
                      className={[
                        "pressable flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left",
                        active
                          ? "bg-ink-100 text-ink-900 dark:bg-white/10 dark:text-white"
                          : "text-ink-500 hover:bg-ink-50 hover:text-ink-800 dark:text-ink-400 dark:hover:bg-white/5 dark:hover:text-ink-100",
                      ].join(" ")}
                    >
                      <Icon size={17} />
                      <span className="flex-1">
                        <span className="block text-[13.5px] font-medium leading-none">{t(item.labelKey)}</span>
                      </span>
                      {item.id === "mods" && (
                        <span className="tabular rounded-full bg-ink-900/[0.07] px-2 py-0.5 text-[11px] font-medium dark:bg-white/10">
                          {activeMods}
                        </span>
                      )}
                    </button>
                    </Enter>
                  );
                })}
              </nav>

              <Enter play={entered} delay={0.32}>
              <div className="p-3">
                <button
                  onClick={() => switchTab("settings")}
                  data-tour="nav-settings"
                  className={[
                    "pressable flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-[13px]",
                    tab === "settings"
                      ? "bg-ink-100 font-medium text-ink-900 dark:bg-white/10 dark:text-white"
                      : "text-ink-500 hover:bg-ink-50 hover:text-ink-800 dark:text-ink-400 dark:hover:bg-white/5 dark:hover:text-ink-100",
                  ].join(" ")}
                >
                  <SlidersHorizontal size={16} />
                  {t("nav.settings")}
                </button>
                <p className="px-3 pt-2 text-[11px] leading-relaxed text-ink-400 dark:text-ink-500">
                  {t("app.disclaimer")}
                </p>
              </div>
              </Enter>
            </div>
          </aside>

          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <Enter play={entered} delay={0.12}>
            <Titlebar />
            </Enter>
            <UpdateBanner />
          <main className="thin-scroll relative min-h-0 min-w-0 flex-1 overflow-y-auto">
            {!navOpen && (
              <button
                onClick={() => setNav(true)}
                title={`${t("app.showMenu")} (Ctrl+B)`}
                className="pressable animate-rise absolute left-4 top-4 z-10 rounded-lg border border-ink-200/80 bg-white/70 p-2 text-ink-500 shadow-pop backdrop-blur-md hover:text-ink-900 dark:border-white/10 dark:bg-white/10 dark:text-ink-300 dark:hover:text-white"
              >
                <PanelLeftOpen size={16} />
              </button>
            )}
            <Enter play={entered} delay={0.2}>
            <div className="tab-stage mx-auto max-w-4xl px-8 py-7">
              <TabPanel active={tab === "android"}>
                <AndroidView mods={mods} />
              </TabPanel>
              <TabPanel active={tab === "ios"}>
                <IosView mods={mods} modInfos={modInfos} onPatched={setLastIpa} />
              </TabPanel>
              <TabPanel active={tab === "mods"}>
                <ModsEditor mods={mods} onChange={setMods} modInfos={modInfos} onRefresh={refreshModInfo} />
                <div className="mt-6">
                  <SectionTitle title={t("mods.repoFmtTitle")} sub={t("mods.repoFmtSub")} />
                </div>
              </TabPanel>
              <TabPanel active={tab === "cuenta"}>
                <AccountView lastIpa={lastIpa} />
              </TabPanel>
              <TabPanel active={tab === "tools"}>
                <ToolsView />
              </TabPanel>
              <TabPanel active={tab === "settings"}>
                <SettingsView
                  settings={settings}
                  onChange={patchSettings}
                  onTour={() => setTourOpen(true)}
                />
              </TabPanel>
            </div>
            </Enter>
          </main>
          </div>
        </div>
        <DiscordFab />
        <Onboarding open={tourOpen} onDone={closeTour} />
        {introOpen && <Intro onDone={finishIntro} />}
        {tip &&
          !tourOpen &&
          createPortal(
            <div
              className="pointer-events-none fixed left-0 top-0 z-50"
              style={{ top: tip.top, left: tip.left, transform: "translateY(-50%)" }}
            >
              <span className="tip-in block whitespace-nowrap rounded-md border border-ink-200 bg-white px-2 py-1 font-mono text-[10.5px] tabular text-ink-500 shadow-pop dark:border-white/10 dark:bg-ink-800 dark:text-ink-300">
                {tip.text}
              </span>
            </div>,
            document.body
          )}
    </I18nContext.Provider>
  );
}
