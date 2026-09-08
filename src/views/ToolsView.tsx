import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, FolderInput, RefreshCw, Wrench } from "lucide-react";
import { ToolStatus, api, humanizeError } from "../lib/tauri";
import { useT } from "../i18n";
import { Badge, Button, Card, SectionTitle } from "../components/ui";

const HINT_KEY: Record<string, string> = {
  apktool: "tools.hint_apktool",
  "uber-apk-signer": "tools.hint_uber",
  zsign: "tools.hint_zsign",
  adb: "tools.hint_adb",
  java: "tools.hint_java",
};

export function ToolsView() {
  const { t } = useT();
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const [adb, setAdb] = useState<{ devices: string[]; raw: string } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  async function refresh() {
    try {
      setTools(await api.toolStatus());
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
    try {
      const d = await api.adbDevices();
      setAdb({ devices: d.devices, raw: d.raw });
    } catch {
      setAdb(null);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function dl(name: string) {
    setBusy(name);
    setMsg(null);
    try {
      const p = await api.downloadTool(name);
      setMsg(`${name}: ${p}`);
      await refresh();
    } catch (e) {
      setMsg(humanizeError(t, e));
    } finally {
      setBusy(null);
    }
  }

  async function installAll() {
    const missing = tools.filter((x) => !x.present && x.canInstall).map((x) => x.name);
    if (missing.length === 0) return;
    setBusy("__all__");
    setMsg(null);
    try {
      for (const name of missing) {
        setMsg(t("tools.installing", { name }));
        await api.downloadTool(name);
      }
      setMsg(t("tools.allReady"));
      await refresh();
    } catch (e) {
      setMsg(humanizeError(t, e));
      await refresh();
    } finally {
      setBusy(null);
    }
  }

  async function placeFile(name: string) {
    const sel = await open({ multiple: false });
    if (typeof sel !== "string" || !sel) return;
    setBusy(name);
    setMsg(null);
    try {
      const p = await api.importToolFile(name, sel);
      setMsg(`${name}: ${p}`);
      await refresh();
    } catch (e) {
      setMsg(humanizeError(t, e));
    } finally {
      setBusy(null);
    }
  }

  const missingInstallable = tools.filter((x) => !x.present && x.canInstall);

  return (
    <div className="space-y-5 max-w-2xl">
      <SectionTitle title={t("tools.title")} sub={t("tools.sub")} />

      {missingInstallable.length > 0 && (
        <Button
          variant="primary"
          disabled={busy === "__all__"}
          onClick={() => void installAll()}
        >
          <Download size={15} />{" "}
          {busy === "__all__" ? t("tools.installingAll") : t("tools.installAll")}
        </Button>
      )}

      <div className="space-y-2">
        {tools.map((tool) => (
          <Card key={tool.name} className="flex items-center gap-3 p-3.5">
            <Wrench size={16} className="shrink-0 text-ink-400" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <p className="font-mono text-sm font-medium text-ink-900 dark:text-white">{tool.name}</p>
                <Badge tone={tool.present ? "ok" : "neutral"}>
                  {tool.present ? t("tools.ready") : t("tools.missing")}
                </Badge>
              </div>
              <p className="mt-0.5 text-xs leading-relaxed text-ink-500 dark:text-ink-400">
                {t(HINT_KEY[tool.name] ?? "tools.sub")}
              </p>
              {tool.path && <p className="truncate font-mono text-[11px] text-ink-400">{tool.path}</p>}
            </div>
            {tool.canInstall && !tool.present && (
              <div className="flex shrink-0 gap-2">
                <Button size="sm" variant="secondary" disabled={busy === tool.name} onClick={() => void dl(tool.name)}>
                  <Download size={13} /> {busy === tool.name ? "…" : t("tools.download")}
                </Button>
                {tool.name === "zsign" && (
                  <Button size="sm" variant="ghost" disabled={busy === tool.name} onClick={() => void placeFile(tool.name)}>
                    <FolderInput size={13} /> {t("tools.placeFile")}
                  </Button>
                )}
              </div>
            )}
          </Card>
        ))}
      </div>

      <Card className="p-4 space-y-2">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-ink-900 dark:text-white">{t("tools.devices")}</h3>
          <Button size="sm" variant="ghost" onClick={() => void refresh()}>
            <RefreshCw size={13} /> {t("tools.refresh")}
          </Button>
        </div>
        {adb ? (
          adb.devices.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {adb.devices.map((d) => (
                <Badge key={d} tone="ok">
                  {d}
                </Badge>
              ))}
            </div>
          ) : (
            <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("tools.noDevices")}</p>
          )
        ) : (
          <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("tools.noAdb")}</p>
        )}
        {msg && <p className="text-xs text-ink-500 dark:text-ink-400">{msg}</p>}
      </Card>
    </div>
  );
}
