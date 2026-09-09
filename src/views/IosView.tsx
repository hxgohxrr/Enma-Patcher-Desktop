import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { sound } from "../lib/sound";
import { Modal } from "../components/Modal";
import { FolderOpen, Download, ShieldCheck, Smartphone } from "lucide-react";
import { IpaInfo, ModInfo, ModSpec, api, fmtBytes, humanizeError, modInfoKey, onProgress } from "../lib/tauri";
import type { AppleAccount } from "../lib/tauri";
import { useT } from "../i18n";
import { Badge, Button, Card, Field, ProgressBar, SectionTitle, TextInput } from "../components/ui";
import { StepEntry, StepsLog, finalizeSteps, reduceProgress } from "../components/StepsLog";
import { OutputFolderPicker } from "../components/OutputFolderPicker";

export function IosView(props: {
  mods: ModSpec[];
  modInfos: Record<string, ModInfo>;
  onPatched: (path: string) => void;
}) {
  const { t } = useT();
  const [ipaPath, setIpaPath] = useState<string | null>(null);
  const [info, setInfo] = useState<IpaInfo | null>(null);
  const [outputName, setOutputName] = useState("");
  const [appName, setAppName] = useState("");
  const [patching, setPatching] = useState(false);
  const [progress, setProgress] = useState(0);
  const [steps, setSteps] = useState<StepEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    outputPath: string;
    totalOverrides: number;
    stripped: number;
    warning: string | null;
    appName: string | null;
  } | null>(null);
  const [accounts, setAccounts] = useState<AppleAccount[]>([]);
  const [accountId, setAccountId] = useState("");
  const [signedPath, setSignedPath] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);

  useEffect(() => {
    const un = onProgress((ev) => {
      setSteps((s) => reduceProgress(s, ev, t));
      if (ev.total > 0) setProgress(Math.round((ev.done / ev.total) * 100));
    });
    return () => {
      void un.then((f) => f());
    };
  }, [t]);

  async function pickIpa() {
    const sel = await open({ multiple: false, filters: [{ name: "IPA", extensions: ["ipa"] }] });
    if (typeof sel !== "string" || !sel) return;
    setIpaPath(sel);
    setResult(null);
    setError(null);
    try {
      setInfo(await api.inspectIpa(sel));
    } catch (e) {
      setError(humanizeError(t, e));
      setInfo(null);
    }
  }

  const versionIssues = props.mods
    .filter((m) => m.enabled)
    .map((m) => ({ m, info: props.modInfos[modInfoKey(m)] }))
    .filter(
      (x): x is { m: ModSpec; info: ModInfo } =>
        !!x.info && !!info?.version && x.info.config.incompatibleVersions.includes(info.version as string)
    )
    .map((x) => ({
      mod: x.m.kind === "github" ? x.m.repo : x.m.path.split(/[/\\]/).pop() || x.m.path,
      v: info?.version as string,
    }));

  const cfgAppName = props.mods
    .filter((m) => m.enabled)
    .map((m) => props.modInfos[modInfoKey(m)]?.config.appName)
    .find((n): n is string => !!n && n.trim().length > 0);

  const hasMods = props.mods.some((m) => m.enabled);
  const [noModsOpen, setNoModsOpen] = useState(false);
  const canPatch = ipaPath && !patching;

  async function runPatch() {
    if (!ipaPath || !canPatch) return;
    if (!hasMods) {
      setNoModsOpen(true);
      return;
    }
    await doPatch();
  }

  async function doPatch() {
    if (!ipaPath) return;
    setNoModsOpen(false);
    setPatching(true);
    setSteps([]);
    setError(null);
    setResult(null);
    setProgress(2);
    try {
      const r = await api.patchIos({
        ipaPath,
        mods: props.mods,
        outputName: outputName.trim() || null,
        appName: appName.trim() || null,
      });
      setResult(r);
      props.onPatched(r.outputPath);
      setSteps((s) => finalizeSteps(s, true));
      setProgress(100);
      setSignedPath(null);
      sound.success();
      try {
        const s = await api.listAppleAccounts();
        setAccounts(s.accounts);
        if (!accountId && s.activeId) setAccountId(s.activeId);
      } catch {
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
    const src = signedPath ?? result.outputPath;
    const dest = await save({
      defaultPath: src.split(/[/\\]/).pop() ?? "parcheado.ipa",
      filters: [{ name: "IPA", extensions: ["ipa"] }],
    });
    if (typeof dest !== "string" || !dest) return;
    try {
      await api.exportFile(src, dest);
      sound.done();
    } catch (e) {
      setError(humanizeError(t, e));
      sound.error();
    }
  }

  async function installAttempt() {
    if (!result) return;
    setError(null);
    try {
      const target = signedPath ?? result.outputPath;
      const r = await api.iosInstallAttempt(target);
      const acc = accounts.find((a) => a.id === accountId);
      const detail = acc ? `${r.message} · ${acc.label || acc.appleId}` : r.message;
      setSteps((s) => [...s, { key: "inst", label: t("steps.install"), detail, state: "done" }]);
      if (r.ok) sound.done();
      else sound.error();
    } catch (e) {
      setError(humanizeError(t, e));
      sound.error();
    }
  }

  async function signResult() {
    if (!result || signing) return;
    setSigning(true);
    setError(null);
    try {
      const r = await api.signIos(result.outputPath, accountId || null, null);
      setSignedPath(r.signedPath);
      setSteps((s) => [...s, { key: "sign", label: t("ios.signed"), detail: r.signedPath, state: "done" }]);
      sound.done();
      try {
        const s = await api.listAppleAccounts();
        setAccounts(s.accounts);
      } catch {
      }
    } catch (e) {
      setError(humanizeError(t, e));
      sound.error();
    } finally {
      setSigning(false);
    }
  }

  return (
    <div className="space-y-5">
      <SectionTitle title={t("ios.title")} sub={t("ios.sub")} />

      <div className="grid grid-cols-2 gap-4">
        <Card className="p-4 space-y-3">
          <Field label={t("ios.ipaLabel")} hint={t("ios.ipaHint")}>
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" onClick={() => void pickIpa()}>
                <FolderOpen size={14} /> {t("ios.pick")}
              </Button>
            </div>
          </Field>
          {ipaPath && (
            <p className="truncate font-mono text-xs text-ink-500 dark:text-ink-400">{ipaPath}</p>
          )}
          {info && (
            <div className="space-y-1.5 text-[13px]">
              <div className="flex flex-wrap gap-2">
                <Badge tone="info">{info.bundleName ?? "App"}</Badge>
                {info.version && <Badge tone="neutral">v{info.version}</Badge>}
              </div>
              <p className="font-mono text-xs text-ink-500 dark:text-ink-400">{info.bundleId}</p>
              <p className="tabular text-ink-500 dark:text-ink-400">
                {fmtBytes(info.totalSize)} · {info.totalEntries} · {info.dataEntries} data/
              </p>
            </div>
          )}
          {versionIssues.length > 0 && (
            <div className="rounded-lg border border-red-600/20 bg-red-50 p-2.5 text-xs leading-relaxed text-red-700 dark:bg-red-500/10 dark:text-red-300">
              {versionIssues.map((x) => (
                <p key={x.mod}>{t("ios.versionIncompat", { mod: x.mod, v: x.v })}</p>
              ))}
            </div>
          )}
        </Card>

        <Card className="p-4 space-y-3">
          <Field label={t("ios.howLabel")} hint={t("ios.howHint")}>
            <div />
          </Field>
          <Field label={t("ios.outLabel")} hint={t("ios.outHint")}>
            <TextInput
              value={outputName}
              onChange={(e) => setOutputName(e.target.value)}
              placeholder={t("ios.outPh")}
            />
          </Field>
          <Field label={t("ios.appNameLabel")} hint={t("ios.appNameHint")}>
            <TextInput
              value={appName}
              onChange={(e) => setAppName(e.target.value)}
              placeholder={cfgAppName ?? t("ios.appNamePh")}
            />
          </Field>
          <OutputFolderPicker platform="ios" />
        </Card>
      </div>

      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-3">
          <Button size="lg" className="flex-1" disabled={!canPatch} onClick={() => void runPatch()}>
            <Download size={16} /> {patching ? t("ios.patching") : t("ios.patch")}
          </Button>
        </div>
        {!hasMods && (
          <p className="text-xs text-ink-500 dark:text-ink-400">{t("ios.noModsHint")}</p>
        )}
        {patching && <ProgressBar value={progress} />}
        <StepsLog steps={steps} error={error} />
        {result && (
          <div className="rounded-lg border border-emerald-600/20 bg-emerald-50/60 p-3 text-[13px] dark:bg-emerald-500/5">
            <p className="font-medium text-emerald-800 dark:text-emerald-200">
              {t("ios.ready", { n: result.totalOverrides })}
              {result.appName ? ` · ${result.appName}` : ""}
            </p>
            {result.stripped > 0 && (
              <p className="mt-1 text-xs text-ink-500 dark:text-ink-400">
                {t("ios.stripped", { n: result.stripped })}
              </p>
            )}
            {result.warning && (
              <p className="mt-1 text-xs text-amber-700 dark:text-amber-300">{result.warning}</p>
            )}
            <p className="mt-1 truncate font-mono text-xs text-ink-500 dark:text-ink-400">
              {result.outputPath}
            </p>
            {accounts.length > 0 && (
              <label className="mt-2.5 block">
                <span className="mb-1 block text-xs text-ink-500 dark:text-ink-400">
                  {t("ios.accountLabel")}
                </span>
                <select
                  value={accountId}
                  onChange={(e) => setAccountId(e.target.value)}
                  className="w-full rounded-lg border border-ink-200 bg-white px-2.5 py-2 text-[13px] text-ink-900 dark:border-white/10 dark:bg-ink-800 dark:text-ink-100"
                >
                  {accounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.label || a.appleId} · {a.appleId}
                    </option>
                  ))}
                </select>
                <span className="mt-1 block text-[11px] leading-relaxed text-ink-400 dark:text-ink-500">
                  {t("ios.accountHint")}
                </span>
              </label>
            )}
            <div className="mt-2.5 flex flex-wrap gap-2">
              <Button size="sm" variant="secondary" onClick={() => void exportResult()}>
                {t("ios.export")}
              </Button>
              {accounts.some((a) => a.hasIdentity) && !signedPath && (
                <Button size="sm" variant="primary" disabled={signing} onClick={() => void signResult()}>
                  <ShieldCheck size={14} /> {signing ? t("ios.signing") : t("ios.sign")}
                </Button>
              )}
              {signedPath && (
                <Badge tone="ok">{t("ios.signedBadge")}</Badge>
              )}
              <Button size="sm" variant="secondary" onClick={() => void api.showInFolder(signedPath ?? result.outputPath)}>
                {t("ios.showFolder")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void installAttempt()}>
                <Smartphone size={14} /> {t("ios.installAttempt")}
              </Button>
            </div>
          </div>
        )}
      </Card>
      <Modal open={noModsOpen} onClose={() => setNoModsOpen(false)} title={t("ios.noModsTitle")}>
        <p className="text-[13px] leading-relaxed text-ink-600 dark:text-ink-300">
          {t("ios.noModsBody")}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={() => setNoModsOpen(false)}>
            {t("ios.noModsCancel")}
          </Button>
          <Button size="sm" variant="primary" onClick={() => void doPatch()}>
            {t("ios.noModsContinue")}
          </Button>
        </div>
      </Modal>
    </div>
  );
}
