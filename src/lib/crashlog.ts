import { api } from "./tauri";

export interface CrashEntry {
  t: string;
  kind: string;
  msg: string;
  stack?: string;
}

export interface ReporterConfig {
  dataDir: string | null;
  endpoint: string;
  auto: boolean;
}

export const APP_VERSION = "0.1.0";
const MAX_ENTRIES = 200;

let entries: CrashEntry[] = [];
let watchers: Set<() => void> = new Set();
let installed = false;
let sending = false;
let saveTimer: number | undefined;
let config: ReporterConfig = { dataDir: null, endpoint: "", auto: false };

function notify() {
  watchers.forEach((w) => w());
}

export function subscribeCrashLog(fn: () => void): () => void {
  watchers.add(fn);
  return () => {
    watchers.delete(fn);
  };
}

export function getCrashEntries(): CrashEntry[] {
  return [...entries];
}

function persist() {
  if (!config.dataDir) return;
  const dir = config.dataDir;
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    void api.writeTextFile(`${dir}/crashlog.json`, JSON.stringify(entries)).catch(() => {});
  }, 800);
}

export function pushCrash(kind: string, msg: string, stack?: string): void {
  entries.push({
    t: new Date().toISOString(),
    kind,
    msg: msg.slice(0, 2000),
    stack: stack?.slice(0, 4000),
  });
  if (entries.length > MAX_ENTRIES) entries = entries.slice(entries.length - MAX_ENTRIES);
  notify();
  persist();
  void maybeAutoSend();
}

export function clearCrashLog(): void {
  entries = [];
  notify();
  persist();
}

export function installCrashHandlers(): void {
  if (installed) return;
  installed = true;
  window.addEventListener("error", (e) => {
    pushCrash("error", e.message || "Unknown error", e.error?.stack);
  });
  window.addEventListener("unhandledrejection", (e) => {
    const r = e.reason;
    const msg = r instanceof Error ? r.message : String(r);
    pushCrash("rejection", msg, r instanceof Error ? r.stack : undefined);
  });
}

export function configureCrashReporting(cfg: ReporterConfig): void {
  config = cfg;
  void maybeAutoSend();
}

export async function loadPersistedCrashLog(dataDir: string): Promise<void> {
  try {
    const raw = await api.readTextFile(`${dataDir}/crashlog.json`);
    const arr = JSON.parse(raw);
    if (Array.isArray(arr)) {
      entries = arr
        .filter((e) => e && typeof e.msg === "string" && typeof e.t === "string")
        .slice(-MAX_ENTRIES);
      notify();
    }
  } catch {
    /* first run */
  }
}

export function dumpCrashLog(): string {
  const head = [
    `Enma Patcher Desktop ${APP_VERSION} crash log`,
    `exported: ${new Date().toISOString()}`,
    `platform: ${navigator.platform} · lang: ${navigator.language}`,
    `entries: ${entries.length}`,
    "---",
  ];
  const body = entries.map((e) => `[${e.t}] ${e.kind}: ${e.msg}${e.stack ? `\n${e.stack}` : ""}`);
  return [...head, ...body].join("\n");
}

export async function sendCrashReport(endpoint: string): Promise<string> {
  const id = `cr-${Date.now().toString(36)}`;
  const ctrl = new AbortController();
  const timer = window.setTimeout(() => ctrl.abort(), 12000);
  try {
    const resp = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        app: "enma-patcher-desktop",
        version: APP_VERSION,
        id,
        entries,
      }),
      signal: ctrl.signal,
    });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    return id;
  } finally {
    window.clearTimeout(timer);
  }
}

async function maybeAutoSend(): Promise<void> {
  if (!config.auto || !config.endpoint || sending || entries.length === 0) return;
  sending = true;
  try {
    await sendCrashReport(config.endpoint);
  } catch {
    /* send failures never re-enter the log */
  } finally {
    sending = false;
  }
}
