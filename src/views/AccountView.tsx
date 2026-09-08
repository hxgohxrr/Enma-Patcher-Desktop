import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, FileKey, KeyRound, Pencil, Plus, ShieldCheck, Smartphone, Trash2, X } from "lucide-react";
import { AppleAccount, api, humanizeError } from "../lib/tauri";
import { useT } from "../i18n";
import { Badge, Button, Card, Field, SectionTitle, TextInput } from "../components/ui";

const EMPTY: AppleAccount = { id: "", label: "", appleId: "", appPassword: "", note: "", p12Password: "", hasIdentity: false };

export function AccountView(props: { lastIpa: string | null }) {
  const { t } = useT();
  const [accounts, setAccounts] = useState<AppleAccount[]>([]);
  const [activeId, setActiveId] = useState("");
  const [form, setForm] = useState<AppleAccount>(EMPTY);
  const [editing, setEditing] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [p12Pass, setP12Pass] = useState("");
  const [signBusy, setSignBusy] = useState("");

  async function reload() {
    try {
      const s = await api.listAppleAccounts();
      setAccounts(s.accounts);
      setActiveId(s.activeId);
    } catch (e) {
      setStatus(humanizeError(t, e));
    }
  }

  useEffect(() => {
    void reload();
  }, []);

  function startNew() {
    setForm(EMPTY);
    setEditing(true);
    setStatus(null);
  }

  function startEdit(a: AppleAccount) {
    setForm({ ...a });
    setEditing(true);
    setStatus(null);
  }

  async function save() {
    setStatus(null);
    try {
      const s = await api.saveAppleAccount(form);
      setAccounts(s.accounts);
      setActiveId(s.activeId);
      setEditing(false);
      setForm(EMPTY);
      setStatus(t("account.saved"));
    } catch (e) {
      setStatus(humanizeError(t, e));
    }
  }

  async function remove(id: string) {
    setStatus(null);
    try {
      const s = await api.deleteAppleAccount(id);
      setAccounts(s.accounts);
      setActiveId(s.activeId);
      if (form.id === id) {
        setForm(EMPTY);
        setEditing(false);
      }
      setStatus(t("account.deleted"));
    } catch (e) {
      setStatus(humanizeError(t, e));
    }
  }

  async function use(id: string) {
    try {
      const s = await api.setActiveAppleAccount(id);
      setAccounts(s.accounts);
      setActiveId(s.activeId);
    } catch (e) {
      setStatus(humanizeError(t, e));
    }
  }

  async function importIdentity(a: AppleAccount) {
    const p12 = await open({
      multiple: false,
      filters: [{ name: "PKCS#12", extensions: ["p12", "pfx"] }],
    });
    if (typeof p12 !== "string" || !p12) return;
    const prov = await open({
      multiple: false,
      filters: [{ name: "Provisioning", extensions: ["mobileprovision"] }],
    });
    if (typeof prov !== "string" || !prov) return;
    setSignBusy(a.id);
    setStatus(null);
    try {
      const s = await api.importSigningIdentity(a.id, p12, prov, p12Pass);
      setAccounts(s.accounts);
      setActiveId(s.activeId);
      setP12Pass("");
      setStatus(t("account.identitySaved"));
    } catch (e) {
      setStatus(humanizeError(t, e));
    } finally {
      setSignBusy("");
    }
  }

  async function removeIdentity(a: AppleAccount) {
    setSignBusy(a.id);
    setStatus(null);
    try {
      const s = await api.deleteSigningIdentity(a.id);
      setAccounts(s.accounts);
      setActiveId(s.activeId);
      setStatus(t("account.identityDeleted"));
    } catch (e) {
      setStatus(humanizeError(t, e));
    } finally {
      setSignBusy("");
    }
  }
  async function install() {
    if (!props.lastIpa) {
      setStatus(t("account.needIpa"));
      return;
    }
    setInstalling(true);
    setStatus(null);
    try {
      const r = await api.iosInstallAttempt(props.lastIpa);
      setStatus(r.message);
    } catch (e) {
      setStatus(humanizeError(t, e));
    } finally {
      setInstalling(false);
    }
  }

  return (
    <div className="space-y-5 max-w-2xl">
      <SectionTitle title={t("account.title")} sub={t("account.sub")} />

      <Card className="p-5 space-y-3">
        {accounts.length === 0 && !editing && (
          <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("account.empty")}</p>
        )}
        {accounts.map((a) => (
          <div
            key={a.id}
            className={[
              "flex items-center gap-3 rounded-lg border p-3",
              a.id === activeId
                ? "border-primary/60 bg-primary/5"
                : "border-ink-200 dark:border-white/10",
            ].join(" ")}
          >
            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-2 truncate text-[13.5px] font-medium text-ink-900 dark:text-white">
                {a.label || a.appleId}
                {a.id === activeId && <Badge tone="ok">{t("account.active")}</Badge>}
              </p>
              <p className="truncate font-mono text-xs text-ink-500 dark:text-ink-400">{a.appleId}</p>
              {a.note && (
                <p className="truncate text-xs text-ink-400 dark:text-ink-500">{a.note}</p>
              )}
            </div>
            {a.id !== activeId && (
              <Button size="sm" variant="ghost" onClick={() => void use(a.id)}>
                <Check size={13} /> {t("account.use")}
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={() => startEdit(a)}>
              <Pencil size={13} />
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void remove(a.id)}>
              <Trash2 size={13} />
            </Button>
          </div>
        ))}
        {!editing && (
          <Button size="sm" variant="secondary" onClick={startNew}>
            <Plus size={14} /> {t("account.add")}
          </Button>
        )}
      </Card>

      <Card className="p-5 space-y-3">
        <div>
          <h3 className="flex items-center gap-2 text-sm font-semibold text-ink-900 dark:text-white">
            <FileKey size={15} /> {t("account.signTitle")}
          </h3>
          <p className="mt-0.5 text-[13px] leading-relaxed text-ink-500 dark:text-ink-400">
            {t("account.signSub")}
          </p>
        </div>
        {accounts.length === 0 && (
          <p className="text-[13px] text-ink-500 dark:text-ink-400">{t("account.empty")}</p>
        )}
        {accounts.map((a) => (
          <div
            key={a.id}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-ink-200 p-3 dark:border-white/10"
          >
            <p className="min-w-0 flex-1 truncate text-[13px] font-medium text-ink-900 dark:text-white">
              {a.label || a.appleId}
            </p>
            {a.hasIdentity ? (
              <>
                <Badge tone="ok">{t("account.identityReady")}</Badge>
                <Button size="sm" variant="ghost" disabled={signBusy === a.id} onClick={() => void removeIdentity(a)}>
                  <Trash2 size={13} /> {t("account.identityRemove")}
                </Button>
              </>
            ) : (
              <>
                <Badge tone="neutral">{t("account.identityMissing")}</Badge>
                <Button size="sm" variant="secondary" disabled={signBusy === a.id} onClick={() => void importIdentity(a)}>
                  <ShieldCheck size={13} /> {signBusy === a.id ? "…" : t("account.identityImport")}
                </Button>
              </>
            )}
          </div>
        ))}
        <Field label={t("account.p12PassLabel")} hint={t("account.p12PassHint")}>
          <TextInput
            type="password"
            value={p12Pass}
            onChange={(e) => setP12Pass(e.target.value)}
            placeholder={t("account.p12PassPh")}
            autoComplete="off"
          />
        </Field>
      </Card>

      {editing && (
        <Card className="p-5 space-y-4">
          <Field label={t("account.labelLabel")} hint={t("account.labelHint")}>
            <TextInput
              value={form.label}
              onChange={(e) => setForm({ ...form, label: e.target.value })}
              placeholder={t("account.labelPh")}
            />
          </Field>
          <Field label={t("account.idLabel")} hint={t("account.idHint")}>
            <TextInput
              value={form.appleId}
              onChange={(e) => setForm({ ...form, appleId: e.target.value })}
              placeholder={t("account.idPh")}
              autoComplete="username"
            />
          </Field>
          <Field label={t("account.passLabel")} hint={t("account.passHint")}>
            <TextInput
              type="password"
              value={form.appPassword}
              onChange={(e) => setForm({ ...form, appPassword: e.target.value })}
              placeholder={t("account.passPh")}
              autoComplete="current-password"
            />
          </Field>
          <Field label={t("account.noteLabel")}>
            <TextInput
              value={form.note}
              onChange={(e) => setForm({ ...form, note: e.target.value })}
              placeholder={t("account.notePh")}
            />
          </Field>
          <div className="flex gap-2">
            <Button onClick={() => void save()}>
              <KeyRound size={15} /> {form.id ? t("account.update") : t("account.save")}
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setEditing(false);
                setForm(EMPTY);
              }}
            >
              <X size={15} /> {t("account.cancel")}
            </Button>
          </div>
        </Card>
      )}

      {status && <p className="text-[13px] text-ink-600 dark:text-ink-300">{status}</p>}

      <Card className="p-5 space-y-3">
        <h3 className="text-sm font-semibold text-ink-900 dark:text-white flex items-center gap-2">
          <Smartphone size={15} /> {t("account.installTitle")}
        </h3>
        <p className="text-[13px] leading-relaxed text-ink-500 dark:text-ink-400">
          {props.lastIpa ?? t("account.noIpa")}
        </p>
        <div>
          <Button variant="secondary" disabled={!props.lastIpa || installing} onClick={() => void install()}>
            {installing ? t("account.installing") : t("account.installBtn")}
          </Button>
        </div>
      </Card>
    </div>
  );
}
