import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Check, FolderOpen, Moon, Sun } from "lucide-react";
import { api, humanizeError } from "../lib/tauri";
import {
  clearCrashLog,
  dumpCrashLog,
  getCrashEntries,
  sendCrashReport,
  subscribeCrashLog,
} from "../lib/crashlog";
import { coverage, listLanguages, useT } from "../i18n";
import { AppSettings } from "../lib/settings";
import accents from "../config/accents.json";
import { Badge, Button, Card, Checkbox, Field, SectionTitle, TextInput } from "../components/ui";

interface AccentDef {
  id: string;
  labelKey: string;
  light: Record<string, string>;
  dark: Record<string, string>;
}

const ACCENTS = accents as AccentDef[];

export function SettingsView(props: {
  settings: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
  onTour: () => void;
}) {
  const { t, lang } = useT();
  const { settings, onChange } = props;
  const [dirs, setDirs] = useState<{
    output: string;
    data: string;
    locales: string;
    account: string;
  } | null>(null);
  const [outDirs, setOutDirs] = useState<{ android: string; ios: string } | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [, setCrashTick] = useState(0);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  useEffect(() => subscribeCrashLog(() => setCrashTick((x) => x + 1)), []);
  const list = getCrashEntries();
  const count = list.length;

  async function copyLog() {
    setStatus(null);
    try {
      await navigator.clipboard.writeText(dumpCrashLog());
      setStatus(t("crash.copied"));
    } catch {
      try {
        const ta = document.createElement("textarea");
        ta.value = dumpCrashLog();
        document.body.appendChild(ta);
        ta.select();
        document.execCommand("copy");
        ta.remove();
        setStatus(t("crash.copied"));
      } catch (e) {
        setStatus(humanizeError(t, e));
      }
    }
  }

  async function saveLog() {
    setStatus(null);
    const dest = await save({
      defaultPath: "enma-crashlog.txt",
      filters: [{ name: "Log", extensions: ["txt", "log"] }],
    });
    if (typeof dest !== "string" || !dest) return;
    try {
      await api.writeTextFile(dest, dumpCrashLog());
      setStatus(dest);
    } catch (e) {
      setStatus(humanizeError(t, e));
    }
  }

  async function sendNow() {
    if (busy || list.length === 0) return;
    setBusy(true);
    setStatus(null);
    try {
      const id = await sendCrashReport(settings.crashEndpoint);
      setStatus(t("crash.sentOk", { id }));
    } catch (e) {
      setStatus(`${t("crash.sentFail")} ${humanizeError(t, e)}`);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void api
      .appDirs()
      .then(setDirs)
      .catch((e) => setMsg(humanizeError(t, e)));
    void Promise.all([api.getOutputDir("android"), api.getOutputDir("ios")])
      .then(([android, ios]) => setOutDirs({ android, ios }))
      .catch(() => {});
  }, []);

  const langs = listLanguages();

  async function forgetAccount() {
    try {
      const s = await api.listAppleAccounts();
      for (const a of s.accounts) {
        await api.deleteAppleAccount(a.id);
      }
      setMsg(t("settings.accountGone"));
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
  }

  function reset() {
    onChange({ lang: "auto", theme: "dark", accent: "mist", font: "preset", showIntro: false, logo: "" });
    try {
      void api.clearAppLogo();
    } catch {
    }
    setMsg(t("settings.resetDone"));
  }

  async function pickLogo() {
    const sel = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "ico"] }],
    });
    if (typeof sel !== "string" || !sel) return;
    try {
      const dest = await api.setAppLogo(sel);
      onChange({ logo: dest });
      setMsg(t("settings.logoSaved"));
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
  }

  async function resetLogo() {
    try {
      await api.clearAppLogo();
    } catch {
    }
    onChange({ logo: "" });
  }

  async function openSettingsFile() {
    if (!dirs) return;
    try {
      await api.saveUserSettings(JSON.stringify(settings));
    } catch {
    }
    try {
      await api.showInFolder(`${dirs.data}/settings.json`);
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
  }

  return (
    <div className="space-y-5 max-w-2xl">
      <SectionTitle title={t("settings.title")} sub={t("settings.sub")} />

      <Card className="p-5 space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.langTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.langSub")}</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {langs.map((l) => {
            const cov = coverage(l.code);
            const pct = cov.total === 0 ? 0 : Math.round((cov.have / cov.total) * 100);
            const selected = l.code === lang;
            return (
              <button
                key={l.code}
                onClick={() => onChange({ lang: l.code })}
                className={[
                  "pressable flex items-center gap-2.5 rounded-lg border p-3 text-left",
                  selected
                    ? "border-primary/60 bg-primary/5"
                    : "border-ink-200 hover:border-ink-300 dark:border-white/10 dark:hover:border-white/25",
                ].join(" ")}
              >
                <span
                  className={[
                    "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
                    selected ? "border-primary bg-primary text-primary-foreground" : "border-ink-300",
                  ].join(" ")}
                >
                  {selected && <Check size={11} strokeWidth={3} />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-medium text-ink-900 dark:text-white">
                    {l.label}
                  </span>
                  <span className="tabular block font-mono text-[11px] text-ink-400">
                    {l.code} · {pct}% {t("settings.coverage")}
                  </span>
                </span>
                {l.external && <Badge tone="info">{t("settings.external")}</Badge>}
              </button>
            );
          })}
        </div>
        <p className="tabular text-xs text-ink-500 dark:text-ink-400">
          {t("settings.langsFound", { n: langs.length })}
        </p>
      </Card>

      <Card className="p-5 space-y-5">
        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.themeTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.themeSub")}</p>
          <div className="mt-2.5 flex gap-2">
            <Button
              size="sm"
              variant={settings.theme === "light" ? "primary" : "secondary"}
              onClick={() => onChange({ theme: "light" })}
            >
              <Sun size={14} /> {t("settings.themeLight")}
            </Button>
            <Button
              size="sm"
              variant={settings.theme === "dark" ? "primary" : "secondary"}
              onClick={() => onChange({ theme: "dark" })}
            >
              <Moon size={14} /> {t("settings.themeDark")}
            </Button>
          </div>
        </div>

        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.accentTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.accentSub")}</p>
          <div className="mt-2.5 flex flex-wrap gap-2">
            {ACCENTS.map((a) => {
              const selected = settings.accent === a.id;
              const dot = a.light.primary || "var(--primary)";
              return (
                <button
                  key={a.id}
                  onClick={() => onChange({ accent: a.id })}
                  className={[
                    "pressable flex items-center gap-2 rounded-full border py-1.5 pl-2 pr-3 text-[13px]",
                    selected
                      ? "border-primary/60 bg-primary/5 text-ink-900 dark:text-white"
                      : "border-ink-200 text-ink-600 hover:border-ink-300 dark:border-white/10 dark:text-ink-300 dark:hover:border-white/25",
                  ].join(" ")}
                >
                  <span className="h-4 w-4 rounded-full border border-black/10" style={{ background: dot }} />
                  {t(a.labelKey)}
                </button>
              );
            })}
          </div>
        </div>

        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.fontTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.fontSub")}</p>
          <div className="mt-2.5 flex gap-2">
            <Button
              size="sm"
              variant={settings.font === "preset" ? "primary" : "secondary"}
              onClick={() => onChange({ font: "preset" })}
            >
              {t("settings.fontPreset")}
            </Button>
            <Button
              size="sm"
              variant={settings.font === "system" ? "primary" : "secondary"}
              onClick={() => onChange({ font: "system" })}
            >
              {t("settings.fontSystem")}
            </Button>
          </div>
        </div>

        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.introTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.introSub")}</p>
          <label className="mt-2 flex items-center gap-2 text-[13px] text-ink-700 dark:text-ink-200">
            <Checkbox
              checked={settings.showIntro}
              onChange={(v) => onChange({ showIntro: v })}
            />
            {t("settings.introEachLaunch")}
          </label>
          <label className="mt-2 flex items-center gap-2 text-[13px] text-ink-700 dark:text-ink-200">
            <Checkbox
              checked={settings.sound}
              onChange={(v) => onChange({ sound: v })}
            />
            {t("settings.soundLabel")}
          </label>
        </div>

        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.logoTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.logoSub")}</p>
          <div className="mt-2.5 flex items-center gap-3">
            {settings.logo && (
              <img
                src={convertFileSrc(settings.logo)}
                alt=""
                className="h-10 w-10 rounded-lg object-cover"
                draggable={false}
              />
            )}
            <Button size="sm" variant="secondary" onClick={() => void pickLogo()}>
              {t("settings.logoChoose")}
            </Button>
            {settings.logo && (
              <Button size="sm" variant="ghost" onClick={() => void resetLogo()}>
                {t("settings.logoReset")}
              </Button>
            )}
          </div>
        </div>
      </Card>

      <Card className="p-5 space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.dataTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.dataSub")}</p>
          <p className="mt-1 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.dataHint")}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" onClick={reset}>
            {t("settings.reset")}
          </Button>
          <Button size="sm" variant="secondary" disabled={!dirs} onClick={() => void openSettingsFile()}>
            {t("settings.openSettings")}
          </Button>
          <Button size="sm" variant="secondary" onClick={props.onTour}>
            {t("tour.replay")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void forgetAccount()}>
            {t("settings.forgetAccount")}
          </Button>
        </div>
        {msg && <p className="text-xs text-ink-500 dark:text-ink-400">{msg}</p>}
      </Card>

      <Card className="p-5 space-y-3">
        <div className="flex items-center gap-2">
          <h3 className="flex-1 text-sm font-semibold text-ink-900 dark:text-white">
            {t("crash.title")}
          </h3>
          <Badge tone={count > 0 ? "warn" : "neutral"}>{t("crash.count", { n: count })}</Badge>
        </div>
        <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("crash.sub")}</p>
        <Field label={t("crash.endpoint")} hint={t("crash.endpointHint")}>
          <TextInput
            value={settings.crashEndpoint}
            onChange={(e) => onChange({ crashEndpoint: e.target.value })}
            placeholder="https://example.com/api/crashlog"
          />
        </Field>
        <label className="flex items-center gap-2 text-[13px] text-ink-700 dark:text-ink-200">
          <Checkbox
            checked={settings.crashAuto}
            onChange={(v) => onChange({ crashAuto: v })}
          />
          {t("crash.auto")}
        </label>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" onClick={() => void copyLog()}>
            {t("crash.copy")}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => void saveLog()}>
            {t("crash.save")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={busy || count === 0}
            onClick={() => void sendNow()}
          >
            {busy ? t("crash.sending") : t("crash.sendNow")}
          </Button>
          <Button size="sm" variant="ghost" disabled={count === 0} onClick={() => clearCrashLog()}>
            {t("crash.clear")}
          </Button>
        </div>
        {status && <p className="text-xs text-ink-500 dark:text-ink-400">{status}</p>}
        {list.length > 0 ? (
          <div className="thin-scroll max-h-40 overflow-y-auto rounded-lg border border-ink-200/80 p-2.5 font-mono text-[11px] leading-relaxed dark:border-white/10">
            {list
              .slice(-30)
              .reverse()
              .map((e, i) => (
                <p key={`${e.t}-${i}`} className="truncate text-ink-600 dark:text-ink-300">
                  [{e.t}] {e.kind}: {e.msg}
                </p>
              ))}
          </div>
        ) : (
          <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("crash.empty")}</p>
        )}
      </Card>

      <Card className="p-5 space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("settings.debugTitle")}</h3>
          <p className="mt-0.5 text-[13px] text-ink-500 dark:text-ink-400">{t("settings.debugSub")}</p>
        </div>
        <dl className="space-y-2 text-[13px]">
          <div className="flex justify-between gap-3">
            <dt className="text-ink-500 dark:text-ink-400">{t("settings.debugVersion")}</dt>
            <dd className="tabular font-mono text-xs text-ink-800 dark:text-ink-200">{t("app.tag")}</dd>
          </div>
          {dirs && outDirs && (
            <>
              <div className="flex items-center justify-between gap-3">
                <dt className="shrink-0 text-ink-500 dark:text-ink-400">
                  {t("settings.debugOut")} · APK
                </dt>
                <dd className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-mono text-xs text-ink-800 dark:text-ink-200">
                    {outDirs.android}
                  </span>
                  <Button size="sm" variant="ghost" onClick={() => void api.showInFolder(outDirs.android)}>
                    <FolderOpen size={13} />
                  </Button>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
                <dt className="shrink-0 text-ink-500 dark:text-ink-400">
                  {t("settings.debugOut")} · IPA
                </dt>
                <dd className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-mono text-xs text-ink-800 dark:text-ink-200">
                    {outDirs.ios}
                  </span>
                  <Button size="sm" variant="ghost" onClick={() => void api.showInFolder(outDirs.ios)}>
                    <FolderOpen size={13} />
                  </Button>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
                <dt className="shrink-0 text-ink-500 dark:text-ink-400">{t("settings.debugData")}</dt>
                <dd className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-mono text-xs text-ink-800 dark:text-ink-200">{dirs.data}</span>
                  <Button size="sm" variant="ghost" onClick={() => void api.showInFolder(dirs.data)}>
                    <FolderOpen size={13} />
                  </Button>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
                <dt className="shrink-0 text-ink-500 dark:text-ink-400">{t("settings.debugLocales")}</dt>
                <dd className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-mono text-xs text-ink-800 dark:text-ink-200">{dirs.locales}</span>
                  <Button size="sm" variant="ghost" onClick={() => void api.showInFolder(dirs.locales)}>
                    <FolderOpen size={13} />
                  </Button>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
                <dt className="shrink-0 text-ink-500 dark:text-ink-400">{t("settings.accountFile")}</dt>
                <dd className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-mono text-xs text-ink-800 dark:text-ink-200">{dirs.account}</span>
                  <Button size="sm" variant="ghost" onClick={() => void api.showInFolder(dirs.account)}>
                    <FolderOpen size={13} />
                  </Button>
                </dd>
              </div>
            </>
          )}
        </dl>
      </Card>
    </div>
  );
}
