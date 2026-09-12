import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, RefreshCw, X } from "lucide-react";
import { useT } from "../i18n";
import { sound } from "../lib/sound";
import { Button, ProgressBar } from "./ui";

const SEEN_KEY = "enma.updateSeen";

export async function checkUpdateSilent(): Promise<string | null> {
  try {
    const update = await check();
    return update?.version ?? null;
  } catch {
    return null;
  }
}

export function UpdateBanner() {
  const { t } = useT();
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(async () => {
      const v = await checkUpdateSilent();
      if (cancelled || !v) return;
      try {
        if (localStorage.getItem(SEEN_KEY) === v) return;
      } catch {
      }
      setVersion(v);
    }, 6000);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  function dismiss() {
    try {
      if (version) localStorage.setItem(SEEN_KEY, version);
    } catch {
    }
    setVersion(null);
  }

  async function updateNow() {
    setError(null);
    try {
      const update = await check();
      if (!update) {
        setVersion(null);
        return;
      }
      let done = 0;
      let total = 0;
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") {
          total = ev.data.contentLength ?? 0;
        } else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          if (total > 0) setProgress(Math.round((done / total) * 100));
        } else if (ev.event === "Finished") {
          setProgress(100);
        }
      });
      setProgress(100);
      sound.done();
      await relaunch();
    } catch (e) {
      setError(String(e));
      setProgress(null);
      sound.error();
    }
  }

  if (!version) return null;
  return (
    <div className="mx-auto w-full max-w-4xl px-8 pt-4">
      <div className="flex items-center gap-3 rounded-xl border border-amber-600/25 bg-amber-50 p-3 text-[13px] dark:bg-amber-500/10">
        <RefreshCw size={15} className="shrink-0 text-amber-700 dark:text-amber-300" />
        <p className="min-w-0 flex-1 text-ink-800 dark:text-ink-100">
          {t("update.available", { v: version })}
          {error && <span className="mt-0.5 block text-xs text-red-600 dark:text-red-400">{error}</span>}
        </p>
        {progress !== null && (
          <div className="w-28 shrink-0">
            <ProgressBar value={progress} />
          </div>
        )}
        <Button size="sm" variant="primary" disabled={progress !== null} onClick={() => void updateNow()}>
          <Download size={13} /> {t("update.now")}
        </Button>
        <button
          onClick={dismiss}
          aria-label="Close"
          className="pressable shrink-0 rounded-md p-1 text-ink-400 hover:bg-ink-100 hover:text-ink-700 dark:hover:bg-white/10 dark:hover:text-ink-100"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
