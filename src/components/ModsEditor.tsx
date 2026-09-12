import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, ExternalLink, FileArchive, Github, Info, Plus, Star, Trash2, ToggleLeft, ToggleRight, TriangleAlert } from "lucide-react";
import { ModInfo, ModSpec, modInfoKey } from "../lib/tauri";
import { useT } from "../i18n";
import { AndroidMark, AppleMark } from "./marks";
import { Badge, Button, Card, SectionTitle, TextInput } from "./ui";
import { Modal } from "./Modal";

function normRepo(s: string): string {
  return s.trim().toLowerCase().split("@")[0].replace(/\/+$/, "");
}

function consoleLabel(c: string): string {
  const l = c.trim().toLowerCase();
  if (l === "switch") return "Switch";
  if (l === "3ds") return "3DS";
  if (l === "android") return "Android";
  if (l === "ios") return "iOS";
  return c.trim();
}

function DetailRow(props: { k: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[128px_1fr] gap-2 border-b border-ink-100 py-1.5 text-[13px] last:border-0 dark:border-white/5">
      <dt className="truncate font-mono text-[11px] leading-5 text-ink-400">{props.k}</dt>
      <dd className="min-w-0 break-words text-ink-800 dark:text-ink-100">{props.children}</dd>
    </div>
  );
}

function ListValue(props: { items: string[] }) {
  if (props.items.length === 0) return <span className="text-ink-400">—</span>;
  return <span className="font-mono text-xs">{props.items.join(", ")}</span>;
}

function ModDetails(props: { mod: ModSpec; info?: ModInfo; onClose: () => void }) {
  const { t } = useT();
  const { mod, info } = props;
  const cfg = info?.config;

  async function openRepo() {
    if (!info?.sourceUrl) return;
    try {
      await openUrl(info.sourceUrl);
    } catch {
      window.open(info.sourceUrl, "_blank", "noopener");
    }
  }

  return (
    <Modal open onClose={props.onClose} title={modLabel(mod)}>
      <dl>
        <DetailRow k="source">
          <span className="flex items-center gap-2">
            {mod.kind === "github" ? <Github size={14} /> : <FileArchive size={14} />}
            {mod.kind === "github" ? mod.repo : mod.path}
            {info?.sourceUrl && (
              <button
                onClick={() => void openRepo()}
                className="pressable text-ink-400 hover:text-ink-700 dark:hover:text-ink-200"
                title={info.sourceUrl}
              >
                <ExternalLink size={14} />
              </button>
            )}
          </span>
        </DetailRow>
        {mod.kind === "github" && info?.stars != null && (
          <DetailRow k="stars">
            <span className="flex items-center gap-1.5">
              <Star size={14} className="text-amber-500" />
              <span className="tabular">{info.stars}</span>
            </span>
          </DetailRow>
        )}
        <DetailRow k="files">
          <span className="tabular">{info ? info.fileCount : "—"}</span>
        </DetailRow>
        {cfg?.appName && <DetailRow k="appName">{cfg.appName}</DetailRow>}
        {cfg?.console && <DetailRow k="console">{consoleLabel(cfg.console)}</DetailRow>}
        {info?.blocked && (
          <DetailRow k="status">
            <Badge tone="bad">{t("mods.blocked")}</Badge>
          </DetailRow>
        )}
        {info && (info.hasPatches || info.hasNestedMods) && (
          <DetailRow k="contains">
            <span className="flex flex-wrap gap-1.5">
              {info.hasPatches && <Badge tone="warn">{t("mods.usesPatches")}</Badge>}
              {info.hasNestedMods && <Badge tone="info">{t("mods.usesNestedMods")}</Badge>}
            </span>
          </DetailRow>
        )}
        {cfg?.version && <DetailRow k="version">{cfg.version}</DetailRow>}
        {cfg?.recommendedVersion && <DetailRow k="recommended_version">{cfg.recommendedVersion}</DetailRow>}
        {cfg && cfg.testedVersions.length > 0 && (
          <DetailRow k="tested_versions"><ListValue items={cfg.testedVersions} /></DetailRow>
        )}
        {cfg && cfg.incompatibleVersions.length > 0 && (
          <DetailRow k="uncompatible_versions"><ListValue items={cfg.incompatibleVersions} /></DetailRow>
        )}
        {cfg?.license && (
          <DetailRow k="license"><Badge tone="neutral">{cfg.license}</Badge></DetailRow>
        )}
        {cfg && (
          <DetailRow k="ai_content">
            {cfg.aiContent ? <Badge tone="warn">AI Usage Present</Badge> : <span>false</span>}
          </DetailRow>
        )}
        {cfg && (
          <DetailRow k="platforms">
            <span className="flex items-center gap-2">
              <span className={cfg.platforms.android ? "" : "opacity-30"}>
                <AndroidMark size={14} />
              </span>
              <span className={cfg.platforms.ios ? "" : "opacity-30"}>
                <AppleMark size={13} />
              </span>
            </span>
          </DetailRow>
        )}
        {cfg && cfg.compatibleMods.length > 0 && (
          <DetailRow k="compatible_mods"><ListValue items={cfg.compatibleMods} /></DetailRow>
        )}
        {cfg && cfg.incompatibleMods.length > 0 && (
          <DetailRow k="uncompatible_mods"><ListValue items={cfg.incompatibleMods} /></DetailRow>
        )}
        {cfg && cfg.include.length > 0 && (
          <DetailRow k="include"><ListValue items={cfg.include} /></DetailRow>
        )}
        {cfg && cfg.exclude.length > 0 && (
          <DetailRow k="exclude"><ListValue items={cfg.exclude} /></DetailRow>
        )}
        {cfg && cfg.includeAndroid.length > 0 && (
          <DetailRow k="include_android"><ListValue items={cfg.includeAndroid} /></DetailRow>
        )}
        {cfg && cfg.excludeAndroid.length > 0 && (
          <DetailRow k="exclude_android"><ListValue items={cfg.excludeAndroid} /></DetailRow>
        )}
        {cfg && cfg.includeIos.length > 0 && (
          <DetailRow k="include_ios"><ListValue items={cfg.includeIos} /></DetailRow>
        )}
        {cfg && cfg.excludeIos.length > 0 && (
          <DetailRow k="exclude_ios"><ListValue items={cfg.excludeIos} /></DetailRow>
        )}
        {!cfg && (
          <p className="py-2 text-[13px] text-ink-500 dark:text-ink-400">{t("mods.test")}</p>
        )}
      </dl>
    </Modal>
  );
}

function modLabel(m: ModSpec): string {
  return m.kind === "github" ? `${m.repo}@${m.branch || "main"}` : m.path.split(/[/\\]/).pop() || m.path;
}

export function ModsEditor(props: {
  mods: ModSpec[];
  onChange: (m: ModSpec[]) => void;
  modInfos: Record<string, ModInfo>;
  onRefresh: (m: ModSpec) => Promise<ModInfo>;
}) {
  const { t } = useT();
  const [repo, setRepo] = useState("ChipLG08/YW1MESP");
  const [branch, setBranch] = useState("main");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [detailIdx, setDetailIdx] = useState<number | null>(null);

  async function addZip() {
    const sel = await open({ multiple: false, filters: [{ name: "ZIP", extensions: ["zip"] }] });
    if (typeof sel !== "string" || !sel) return;
    props.onChange([...props.mods, { kind: "zip", repo: "", branch: "", path: sel, enabled: true }]);
  }

  function addGithub() {
    const clean = repo.trim();
    if (!clean.includes("/")) {
      setMsg(t("mods.badRepo"));
      return;
    }
    setMsg(null);
    props.onChange([
      ...props.mods,
      { kind: "github", repo: clean, branch: branch.trim() || "main", path: "", enabled: true },
    ]);
  }

  async function probe(mod: ModSpec) {
    setBusy(true);
    setMsg(null);
    try {
      const info = await props.onRefresh(mod);
      setMsg(t("mods.probed", { repo: modLabel(mod), n: info.fileCount }));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  }

  const enabledGithub = props.mods
    .map((m, i) => ({ m, i }))
    .filter(({ m }) => m.enabled && m.kind === "github");
  const conflictPairs: [number, number][] = [];
  for (let a = 0; a < enabledGithub.length; a++) {
    for (let b = a + 1; b < enabledGithub.length; b++) {
      const ma = enabledGithub[a];
      const mb = enabledGithub[b];
      const ia = props.modInfos[modInfoKey(ma.m)];
      const ib = props.modInfos[modInfoKey(mb.m)];
      if (!ia || !ib) continue;
      const ra = normRepo(ma.m.repo);
      const rb = normRepo(mb.m.repo);
      const bad =
        ia.config.incompatibleMods.map(normRepo).includes(rb) ||
        ib.config.incompatibleMods.map(normRepo).includes(ra);
      if (bad) conflictPairs.push([ma.i, mb.i]);
    }
  }
  const conflicted = new Set(conflictPairs.flat());

  return (
    <div className="space-y-4">
      <SectionTitle title={t("mods.title")} sub={t("mods.sub")} />
      {conflictPairs.length > 0 && (
        <Card className="space-y-1.5 border-amber-600/25 bg-amber-50 p-4 text-[13px] dark:bg-amber-500/10">
          {conflictPairs.map(([a, b]) => (
            <p key={`${a}-${b}`} className="flex items-center gap-2 text-amber-800 dark:text-amber-200">
              <TriangleAlert size={14} className="shrink-0" />
              {t("mods.conflict", { a: modLabel(props.mods[a]), b: modLabel(props.mods[b]) })}
            </p>
          ))}
        </Card>
      )}
      {props.mods.length === 0 && (
        <Card className="p-5 text-sm text-ink-500 dark:text-ink-400">{t("mods.empty")}</Card>
      )}
      <div className="space-y-2">
        {props.mods.map((m, i) => {
          const info = props.modInfos[modInfoKey(m)];
          const cfg = info?.config;
          const healthy = m.enabled && cfg && !conflicted.has(i);
          return (
            <Card key={i} className="space-y-2 p-3">
              <div className="flex items-center gap-3">
                <button
                  className="pressable text-ink-400 hover:text-ink-700 dark:hover:text-ink-200 disabled:pointer-events-none disabled:opacity-40"
                  title={info?.blocked ? t("mods.blocked") : m.enabled ? t("mods.disable") : t("mods.enable")}
                  disabled={info?.blocked}
                  onClick={() => {
                    const next = [...props.mods];
                    next[i] = { ...m, enabled: !m.enabled };
                    props.onChange(next);
                  }}
                >
                  {m.enabled ? <ToggleRight size={22} /> : <ToggleLeft size={22} />}
                </button>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-ink-900 dark:text-white">
                    {m.kind === "github" ? `${m.repo}@${m.branch}` : m.path.split(/[/\\]/).pop()}
                  </p>
                  <p className="text-xs text-ink-500 dark:text-ink-400">
                    {m.kind === "github" ? m.path || "GitHub" : m.path}
                  </p>
                </div>
                <Badge tone={m.enabled ? "ok" : "neutral"}>
                  {m.enabled ? t("mods.active") : t("mods.paused")}
                </Badge>
                <button
                  className="pressable text-ink-400 hover:text-ink-700 dark:hover:text-ink-200"
                  title={t("mods.details")}
                  onClick={() => setDetailIdx(i)}
                >
                  <Info size={16} />
                </button>
                <button
                  className="pressable text-ink-400 hover:text-red-600"
                  title={t("mods.remove")}
                  onClick={() => props.onChange(props.mods.filter((_, j) => j !== i))}
                >
                  <Trash2 size={16} />
                </button>
              </div>
              <div
                className="flex cursor-pointer flex-wrap items-center gap-1.5 pl-9"
                title={t("mods.details")}
                onClick={() => setDetailIdx(i)}
              >
                <span className="flex items-center gap-1 text-ink-400">
                  <AndroidMark
                    size={13}
                    className={cfg && !cfg.platforms.android ? "opacity-30" : ""}
                  />
                  <AppleMark size={12} className={cfg && !cfg.platforms.ios ? "opacity-30" : ""} />
                </span>
                {info?.blocked && <Badge tone="bad">{t("mods.blocked")}</Badge>}
                {cfg?.console && <Badge tone="info">{consoleLabel(cfg.console)}</Badge>}
                {info?.hasPatches && <Badge tone="warn">{t("mods.usesPatches")}</Badge>}
                {info?.hasNestedMods && <Badge tone="info">{t("mods.usesNestedMods")}</Badge>}
                {cfg?.aiContent && <Badge tone="warn">AI Usage Present</Badge>}
                {cfg?.license && <Badge tone="neutral">{cfg.license}</Badge>}
                {cfg?.recommendedVersion && (
                  <Badge tone="info">rec {cfg.recommendedVersion}</Badge>
                )}
                {cfg && cfg.testedVersions.length > 0 && (
                  <span
                    className="block min-w-0 max-w-[220px] truncate"
                    title={cfg.testedVersions.join(", ")}
                  >
                    <Badge tone="info">tested {cfg.testedVersions.join(", ")}</Badge>
                  </span>
                )}
                {healthy ? (
                  <Check size={14} className="text-emerald-600 dark:text-emerald-400" />
                ) : null}
                <span className="flex-1" />
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={(e) => {
                    e.stopPropagation();
                    void probe(m);
                  }}
                >
                  {t("mods.test")}
                </Button>
              </div>
            </Card>
          );
        })}
      </div>

      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-2 text-sm font-medium text-ink-900 dark:text-white">
          <Github size={16} /> {t("mods.addRepo")}
        </div>
        <div className="grid grid-cols-[1fr_140px] gap-2">
          <TextInput value={repo} onChange={(e) => setRepo(e.target.value)} placeholder={t("mods.repoPh")} />
          <TextInput value={branch} onChange={(e) => setBranch(e.target.value)} placeholder={t("mods.branchPh")} />
        </div>
        <div className="flex gap-2">
          <Button size="sm" variant="secondary" onClick={addGithub}>
            <Plus size={14} /> {t("mods.add")}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => void addZip()}>
            <FileArchive size={14} /> {t("mods.addZip")}
          </Button>
        </div>
        {msg && <p className="text-xs text-ink-500 dark:text-ink-400">{msg}</p>}
      </Card>
      {detailIdx !== null && props.mods[detailIdx] && (
        <ModDetails
          mod={props.mods[detailIdx]}
          info={props.modInfos[modInfoKey(props.mods[detailIdx])]}
          onClose={() => setDetailIdx(null)}
        />
      )}
    </div>
  );
}
