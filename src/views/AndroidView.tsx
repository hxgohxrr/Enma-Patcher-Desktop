import { useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { sound } from "../lib/sound";
import { Modal } from "../components/Modal";
import { HoldButton } from "../components/HoldButton";
import { Check, ChevronDown, FolderOpen, ShieldCheck, TriangleAlert, Download, MonitorSmartphone } from "lucide-react";
import {
  ApksInfo,
  DeviceCompat,
  DrmbInfo,
  ModSpec,
  PatchedCheck,
  api,
  fmtBytes,
  humanizeError,
  onProgress,
} from "../lib/tauri";
import { useT } from "../i18n";
import { Badge, Button, Card, Checkbox, Field, ProgressBar, SectionTitle, TextInput } from "../components/ui";
import { StepEntry, StepsLog, finalizeSteps, reduceProgress } from "../components/StepsLog";
import { OutputFolderPicker } from "../components/OutputFolderPicker";

export function AndroidView(props: { mods: ModSpec[] }) {
  const { t } = useT();
  const [apksPath, setApksPath] = useState<string | null>(null);
  const [apksInfo, setApksInfo] = useState<ApksInfo | null>(null);
  const [drmbPath, setDrmbPath] = useState<string | null>(null);
  const [drmbInfo, setDrmbInfo] = useState<DrmbInfo | null>(null);
  const [check, setCheck] = useState<PatchedCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [forceSingle, setForceSingle] = useState(false);
  const [outputName, setOutputName] = useState("");
  const [appName, setAppName] = useState("");
  const [patching, setPatching] = useState(false);
  const [progress, setProgress] = useState(0);
  const [steps, setSteps] = useState<StepEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    outputPath: string;
    totalOverrides: number;
    smaliFiles: number;
    usedApktool: boolean;
    signSchemes: string[];
    warning: string | null;
    appName: string | null;
  } | null>(null);
  const [compat, setCompat] = useState<DeviceCompat | null>(null);
  const [androidVer, setAndroidVer] = useState("");
  const [customSign, setCustomSign] = useState(false);
  const [sv1, setSv1] = useState(true);
  const [sv2, setSv2] = useState(true);
  const [sv3, setSv3] = useState(true);
  const [signOpen, setSignOpen] = useState(false);
  const signBoxRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!signOpen) return;
    const close = (e: PointerEvent) => {
      if (!signBoxRef.current?.contains(e.target as Node)) setSignOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSignOpen(false);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [signOpen ]);

  const SIGN_OPTIONS = [
    { value: "auto", label: t("sign.auto") },
    { value: "6", label: "Android < 7" },
    { value: "8", label: "7 – 8" },
    { value: "10", label: "9 – 10" },
    { value: "13", label: "11+" },
  ];
  const signCurrent = SIGN_OPTIONS.find((o) => (androidVer || "auto") === o.value) ?? SIGN_OPTIONS[0];

  function effectiveSchemes(): string[] {
    if (customSign) {
      const out: string[] = [];
      if (sv1) out.push("v1");
      if (sv2 && !sv3) out.push("v2");
      if (sv3) out.push("v3");
      return out;
    }
    const m = parseInt(androidVer || "0", 10);
    if (!m) return ["v1", "v2"];
    if (m < 9) return ["v1", "v2"];
    return ["v3"];
  }

  useEffect(() => {
    const un = onProgress((ev) => {
      setSteps((s) => reduceProgress(s, ev, t));
      if (ev.total > 0) setProgress(Math.round((ev.done / ev.total) * 100));
    });
    return () => {
      void un.then((f) => f());
    };
  }, [t]);

  async function pickApks() {
    const sel = await open({
      multiple: false,
      filters: [{ name: "Paquete", extensions: ["apks", "apk", "xapk"] }],
    });
    if (typeof sel !== "string" || !sel) return;
    setApksPath(sel);
    setCheck(null);
    setForceSingle(false);
    setResult(null);
    setError(null);
    try {
      const info = await api.inspectApks(sel);
      setApksInfo(info);
    } catch (e) {
      setError(humanizeError(t, e));
      setApksInfo(null);
    }
  }

  async function pickDrmb() {
    const sel = await open({ multiple: false, filters: [{ name: "DRMB", extensions: ["drmb", "zip"] }] });
    if (typeof sel !== "string" || !sel) return;
    setDrmbPath(sel);
    setError(null);
    try {
      setDrmbInfo(await api.inspectDrmb(sel));
    } catch (e) {
      setError(humanizeError(t, e));
      setDrmbInfo(null);
    }
  }

  async function runCheck() {
    if (!apksPath) return;
    setChecking(true);
    setError(null);
    try {
      const gh = props.mods.find((m) => m.enabled && m.kind === "github");
      let markers: string[] = [];
      if (gh) {
        const [owner, ...rest] = gh.repo.split("/");
        markers = await api.listGithubFiles(owner, rest.join("/"), gh.branch || "main");
        markers = markers.filter((p) => p !== "enmapatcher.cfg.json");
      }
      const r = await api.checkApkPatched(apksPath, markers);
      setCheck(r);
    } catch (e) {
      setError(humanizeError(t, e));
    } finally {
      setChecking(false);
    }
  }

  const needsDrmb = apksInfo && !apksInfo.singleApk && apksInfo.splits.length > 0;
  const hasMods = props.mods.some((m) => m.enabled);
  const [noModsOpen, setNoModsOpen] = useState(false);
  const [smaliOpen, setSmaliOpen] = useState(false);
  const [smaliFiles, setSmaliFiles] = useState(0);
  const [scanning, setScanning] = useState(false);
  const canPatch =
    apksPath &&
    (!needsDrmb || drmbPath) &&
    !patching &&
    (!customSign || sv1 || sv2 || sv3) &&
    (apksInfo?.singleApk ? check?.verdict === "patched" || forceSingle : true);

  async function runPatch() {
    if (!apksPath || !canPatch) return;
    if (!hasMods) {
      setNoModsOpen(true);
      return;
    }
    setScanning(true);
    try {
      const enabled = props.mods.filter((m) => m.enabled);
      const scans = await Promise.all(enabled.map((m) => api.inspectModSmali(m)));
      const total = scans.reduce((n, s) => n + s.smaliFiles, 0);
      if (total > 0) {
        setSmaliFiles(total);
        setSmaliOpen(true);
        return;
      }
    } catch (e) {
      setError(humanizeError(t, e));
      return;
    } finally {
      setScanning(false);
    }
    await doPatch();
  }

  async function doPatch() {
    if (!apksPath) return;
    setNoModsOpen(false);
    setSmaliOpen(false);
    setPatching(true);
    setSteps([]);
    setError(null);
    setResult(null);
    setCompat(null);
    setProgress(2);
    try {
      const r = await api.patchAndroid({
        apksPath,
        drmbPath,
        mods: props.mods,
        outputName: outputName.trim() || null,
        forceSingleApk: forceSingle,
        appName: appName.trim() || null,
        signMode: customSign ? "custom" : "auto",
        signV1: customSign ? sv1 : true,
        signV2: customSign ? sv2 : true,
        signV3: customSign ? sv3 : true,
        androidVersion: androidVer || null,
      });
      setResult(r);
      setSteps((s) => finalizeSteps(s, true));
      setProgress(100);
      sound.success();
      try {
        setCompat(await api.checkDeviceCompat(r.outputPath));
      } catch {
        setCompat(null);
      }
    } catch (e) {
      setSteps((s) => finalizeSteps(s, false));
      setError(humanizeError(t, e));
      sound.error();
    } finally {
      setPatching(false);
    }
  }

  async function exportResult() {
    if (!result) return;
    const dest = await save({
      defaultPath: result.outputPath.split(/[/\\]/).pop() ?? "parcheado.apk",
      filters: [{ name: "APK", extensions: ["apk"] }],
    });
    if (typeof dest !== "string" || !dest) return;
    try {
      await api.exportFile(result.outputPath, dest);
      sound.done();
    } catch (e) {
      setError(humanizeError(t, e));
      sound.error();
    }
  }

  async function installAdb() {
    if (!result) return;
    setError(null);
    try {
      const msg = await api.androidInstall(result.outputPath);
      setSteps((s) => [...s, { key: "inst", label: t("steps.install"), detail: msg, state: "done" }]);
      sound.done();
    } catch (e) {
      setError(humanizeError(t, e));
      sound.error();
    }
  }

  function verdictBadge(v: string) {
    if (v === "patched") return <Badge tone="ok">{t("android.verdictPatched")}</Badge>;
    if (v === "clean") return <Badge tone="bad">{t("android.verdictClean")}</Badge>;
    return <Badge tone="warn">{t("android.verdictUnknown")}</Badge>;
  }

  return (
    <div className="space-y-5">
      <SectionTitle title={t("android.title")} sub={t("android.sub")} />

      <div className="grid grid-cols-2 gap-4">
        <Card className="p-4 space-y-3">
          <Field label={t("android.pkgLabel")} hint={t("android.pkgHint")}>
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" onClick={() => void pickApks()}>
                <FolderOpen size={14} /> {t("android.pick")}
              </Button>
            </div>
          </Field>
          {apksPath && (
            <p className="truncate font-mono text-xs text-ink-500 dark:text-ink-400">{apksPath}</p>
          )}
          {apksInfo && (
            <div className="space-y-2 text-[13px]">
              <div className="flex items-center gap-2">
                <Badge tone={apksInfo.singleApk ? "warn" : "info"}>
                  {apksInfo.singleApk
                    ? t("android.singleApk")
                    : t("android.pieces", { n: apksInfo.splits.length + 1 })}
                </Badge>
                <span className="tabular text-ink-500 dark:text-ink-400">{fmtBytes(apksInfo.totalSize)}</span>
              </div>
              {apksInfo.baseApk && (
                <p className="text-ink-700 dark:text-ink-200">
                  {t("android.base")} <span className="font-mono text-xs">{apksInfo.baseApk}</span>
                </p>
              )}
              {apksInfo.splits.slice(0, 6).map((s) => (
                <div key={s.name} className="flex justify-between gap-2 font-mono text-xs text-ink-500 dark:text-ink-400">
                  <span className="truncate">{s.name}</span>
                  <span className="tabular shrink-0">
                    {s.kind} · {fmtBytes(s.size)}
                  </span>
                </div>
              ))}
              {apksInfo.singleApk && (
                <div className="rounded-lg border border-amber-600/20 bg-amber-50 p-3 text-xs leading-relaxed text-amber-800 dark:bg-amber-500/10 dark:text-amber-200">
                  <p className="font-medium mb-1 flex items-center gap-1.5">
                    <TriangleAlert size={13} /> {t("android.checkTitle")}
                  </p>
                  <p>{t("android.checkBody")}</p>
                  <div className="mt-2 flex items-center gap-2">
                    <Button size="sm" variant="secondary" disabled={checking} onClick={() => void runCheck()}>
                      <ShieldCheck size={13} /> {checking ? t("android.checking") : t("android.checkBtn")}
                    </Button>
                  </div>
                  {check && (
                    <div className="mt-2 space-y-1.5">
                      <span className="inline-flex items-center gap-2">
                        {verdictBadge(check.verdict)}
                        <span className="tabular">
                          {check.matched}/{check.sampled}
                        </span>
                      </span>
                      <p>{check.detail}</p>
                      {check.verdict !== "patched" && (
                        <label className="flex items-center gap-2 pt-1">
                          <Checkbox checked={forceSingle} onChange={setForceSingle} />
                          {t("android.force")}
                        </label>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </Card>

        <Card className="p-4 space-y-3">
          <Field label={t("android.drmbLabel")} hint={t(needsDrmb ? "android.drmbHintReq" : "android.drmbHintOpt")}>
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" onClick={() => void pickDrmb()}>
                <FolderOpen size={14} /> {t("android.pickDrmb")}
              </Button>
            </div>
          </Field>
          {drmbPath && (
            <p className="truncate font-mono text-xs text-ink-500 dark:text-ink-400">{drmbPath}</p>
          )}
          {drmbInfo && (
            <div className="flex flex-wrap gap-2 text-[13px]">
              <Badge tone="info">base/ {drmbInfo.baseFiles}</Badge>
              <Badge tone="info">split/ {drmbInfo.splitFiles}</Badge>
              <Badge tone={drmbInfo.smaliFiles > 0 ? "warn" : "neutral"}>
                smali {drmbInfo.smaliFiles}
              </Badge>
              {drmbInfo.smaliFiles > 0 && (
                <p className="w-full text-xs text-ink-500 dark:text-ink-400">{t("android.smaliNote")}</p>
              )}
            </div>
          )}
          <Field label={t("android.outLabel")} hint={t("android.outHint")}>
            <TextInput
              value={outputName}
              onChange={(e) => setOutputName(e.target.value)}
              placeholder={t("android.outPh")}
            />
          </Field>
          <Field label={t("android.appNameLabel")} hint={t("android.appNameHint")}>
            <TextInput
              value={appName}
              onChange={(e) => setAppName(e.target.value)}
              placeholder={t("android.appNamePh")}
            />
          </Field>
          <OutputFolderPicker platform="android" />
        </Card>
      </div>

      <Card className="p-4 space-y-3">
        <Field label={t("sign.title")} hint={t("sign.sub")}>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[13px] text-ink-600 dark:text-ink-300">{t("sign.version")}</span>
            <div ref={signBoxRef} className="relative">
              <button
                onClick={() => setSignOpen((o) => !o)}
                className="pressable flex h-10 items-center gap-2 rounded-lg border border-ink-200 bg-white px-3 text-sm text-ink-900 hover:border-ink-300 dark:border-white/10 dark:bg-white/5 dark:text-ink-50 dark:hover:border-white/20"
              >
                <span className="flex-1 text-left">{signCurrent.label}</span>
                <ChevronDown
                  size={15}
                  className={`shrink-0 text-ink-400 transition-transform duration-200 ${signOpen ? "rotate-180" : ""}`}
                />
              </button>
              {signOpen && (
                <div className="animate-rise absolute left-0 top-full z-30 mt-1.5 w-48 rounded-lg border border-ink-200 bg-white p-1 shadow-pop dark:border-white/10 dark:bg-ink-800">
                  {SIGN_OPTIONS.map((o) => {
                    const selected = (androidVer || "auto") === o.value;
                    return (
                      <button
                        key={o.value}
                        onClick={() => {
                          setAndroidVer(o.value === "auto" ? "" : o.value);
                          setSignOpen(false);
                        }}
                        className={[
                          "pressable flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-[13px]",
                          selected
                            ? "bg-ink-100 font-medium text-ink-900 dark:bg-white/10 dark:text-white"
                            : "text-ink-600 hover:bg-ink-50 dark:text-ink-300 dark:hover:bg-white/5",
                        ].join(" ")}
                      >
                        <span className="flex-1">{o.label}</span>
                        {selected && <Check size={14} className="shrink-0" />}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
            <label className="flex items-center gap-2 text-[13px] text-ink-600 dark:text-ink-300">
              <Checkbox checked={customSign} onChange={setCustomSign} />
              {t("sign.custom")}
            </label>
          </div>
        </Field>
        {customSign && (
          <div className="flex gap-4 text-[13px] text-ink-700 dark:text-ink-200">
            {(["v1", "v2", "v3"] as const).map((v, i) => (
              <label key={v} className="flex items-center gap-1.5 font-mono">
                <Checkbox
                  checked={[sv1, sv2, sv3][i]}
                  onChange={(c) => {
                    const next = [sv1, sv2, sv3];
                    next[i] = c;
                    setSv1(next[0]);
                    setSv2(next[1]);
                    setSv3(next[2]);
                  }}
                />
                {v}
              </label>
            ))}
          </div>
        )}
        <p className="tabular font-mono text-xs text-ink-500 dark:text-ink-400">
          {t("sign.will", { s: effectiveSchemes().join(" + ") || "—" })}
        </p>
      </Card>

      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-3">
          <Button size="lg" className="flex-1" disabled={!canPatch} onClick={() => void runPatch()}>
            <Download size={16} /> {patching || scanning ? t("android.patching") : t("android.patch")}
          </Button>
        </div>
        {!hasMods && (
          <p className="text-xs text-ink-500 dark:text-ink-400">{t("android.noModsHint")}</p>
        )}
        {needsDrmb && !drmbPath && (
          <p className="text-xs text-amber-700 dark:text-amber-300">{t("android.needDrmb")}</p>
        )}
        {patching && <ProgressBar value={progress} />}
        <StepsLog steps={steps} error={error} />
        {result && (
          <div className="rounded-lg border border-emerald-600/20 bg-emerald-50/60 p-3 text-[13px] dark:bg-emerald-500/5">
            <p className="font-medium text-emerald-800 dark:text-emerald-200">
              {t("android.ready", { n: result.totalOverrides })}
              {result.usedApktool ? t("android.viaApktool") : ""}.
            </p>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              <Badge tone={result.signSchemes.includes("v2") || result.signSchemes.includes("v3") ? "ok" : "bad"}>
                {result.signSchemes.join(" + ") || "—"}
              </Badge>
              {result.appName && <Badge tone="info">{result.appName}</Badge>}
            </div>
            <p className="mt-1 truncate font-mono text-xs text-ink-500 dark:text-ink-400">
              {result.outputPath}
            </p>
            {result.warning && <p className="mt-1 text-amber-700 dark:text-amber-300">{result.warning}</p>}
            <p className="mt-1 text-xs text-ink-500 dark:text-ink-400">{t("android.uninstallHint")}</p>
            {compat && compat.compatible === false && (
              <div className="mt-2 rounded-lg border border-red-600/20 bg-red-50 p-2.5 text-xs leading-relaxed text-red-700 dark:bg-red-500/10 dark:text-red-300">
                <p className="font-medium">ABI: {compat.apkAbis.join(", ") || "—"}</p>
                <p className="mt-0.5 font-mono">{compat.note}</p>
              </div>
            )}
            <div className="mt-2.5 flex flex-wrap gap-2">
              <Button size="sm" variant="secondary" onClick={() => void exportResult()}>
                {t("android.export")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void api.showInFolder(result.outputPath)}>
                {t("android.showFolder")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void installAdb()}>
                <MonitorSmartphone size={14} /> {t("android.installAdb")}
              </Button>
            </div>
          </div>
        )}
      </Card>
      <Modal open={noModsOpen} onClose={() => setNoModsOpen(false)} title={t("android.noModsTitle")}>
        <p className="text-[13px] leading-relaxed text-ink-600 dark:text-ink-300">
          {t("android.noModsBody")}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={() => setNoModsOpen(false)}>
            {t("android.noModsCancel")}
          </Button>
          <Button size="sm" variant="primary" onClick={() => void doPatch()}>
            {t("android.noModsContinue")}
          </Button>
        </div>
      </Modal>
      <Modal open={smaliOpen} onClose={() => setSmaliOpen(false)} title={t("android.smaliTitle")}>
        <p className="text-[13px] leading-relaxed text-ink-600 dark:text-ink-300">
          {t("android.smaliBody", { n: smaliFiles })}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={() => setSmaliOpen(false)}>
            {t("android.smaliCancel")}
          </Button>
          <HoldButton durationMs={5000} onDone={() => void doPatch()}>
            {t("android.smaliHold")}
          </HoldButton>
        </div>
      </Modal>
    </div>
  );
}
