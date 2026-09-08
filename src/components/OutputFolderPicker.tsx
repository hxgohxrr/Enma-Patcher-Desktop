import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { api, humanizeError } from "../lib/tauri";
import { useT } from "../i18n";
import { Button, Field } from "./ui";

export function OutputFolderPicker(props: { platform: "android" | "ios" }) {
  const { t } = useT();
  const [dir, setDir] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    void api
      .getOutputDir(props.platform)
      .then(setDir)
      .catch((e) => setMsg(humanizeError(t, e)));
  }, []);

  async function pick() {
    const sel = await open({ directory: true, multiple: false });
    if (typeof sel !== "string" || !sel) return;
    setMsg(null);
    try {
      setDir(await api.setOutputDir(props.platform, sel));
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
  }

  async function reset() {
    setMsg(null);
    try {
      setDir(await api.setOutputDir(props.platform, null));
    } catch (e) {
      setMsg(humanizeError(t, e));
    }
  }

  return (
    <Field label={t("settings.debugOut")}>
      <p className="truncate font-mono text-xs text-ink-500 dark:text-ink-400">{dir ?? "…"}</p>
      <div className="mt-2 flex gap-2">
        <Button size="sm" variant="secondary" onClick={() => void pick()}>
          <FolderOpen size={14} /> {t("output.change")}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void reset()}>
          {t("output.reset")}
        </Button>
      </div>
      {msg && <p className="mt-1.5 text-xs text-red-600 dark:text-red-400">{msg}</p>}
    </Field>
  );
}
