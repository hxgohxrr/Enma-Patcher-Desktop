import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import { ProgressEvent, stepLabel } from "../lib/tauri";

export interface StepEntry {
  key: string;
  label: string;
  detail: string;
  state: "running" | "done" | "error";
}

export function reduceProgress(
  log: StepEntry[],
  ev: ProgressEvent,
  t: (key: string) => string
): StepEntry[] {
  const label = stepLabel(t, ev.step);
  const settled = log.map((s) =>
    s.state === "running" && s.key !== ev.step ? { ...s, state: "done" as const } : s
  );
  const idx = settled.findIndex((s) => s.key === ev.step);
  const entry: StepEntry = {
    key: ev.step,
    label,
    detail: ev.detail,
    state: ev.step === "done" ? "done" : "running",
  };
  if (idx >= 0) settled[idx] = entry;
  else settled.push(entry);
  if (ev.step === "done") {
    return settled.map((s) => (s.state === "running" ? { ...s, state: "done" as const } : s));
  }
  return settled;
}

export function finalizeSteps(log: StepEntry[], ok: boolean): StepEntry[] {
  return log.map((s) =>
    s.state === "running" ? { ...s, state: ok ? ("done" as const) : ("error" as const) } : s
  );
}

export function StepsLog(props: { steps: StepEntry[]; error: string | null }) {
  if (props.steps.length === 0 && !props.error) return null;
  return (
    <div className="rounded-xl border border-ink-200/80 bg-ink-50/60 dark:border-white/10 dark:bg-white/[0.02]">
      <div className="thin-scroll max-h-56 overflow-y-auto p-3 space-y-1">
        {props.steps.map((s) => (
          <div key={s.key} className="flex items-start gap-2.5 text-[13px] animate-rise">
            {s.state === "running" ? (
              <Loader2 size={15} className="mt-0.5 shrink-0 animate-spin text-primary" />
            ) : s.state === "error" ? (
              <XCircle size={15} className="mt-0.5 shrink-0 text-red-600 dark:text-red-400" />
            ) : s.key === "done" ? (
              <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-emerald-600 dark:text-emerald-400" />
            ) : (
              <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-emerald-600/60 dark:text-emerald-400/60" />
            )}
            <div className="min-w-0">
              <span className="font-medium text-ink-800 dark:text-ink-100">{s.label}</span>
              <span className="text-ink-500 dark:text-ink-400"> — {s.detail}</span>
            </div>
          </div>
        ))}
        {props.error && (
          <div className="flex items-start gap-2.5 text-[13px] animate-rise">
            <XCircle size={15} className="mt-0.5 shrink-0 text-red-600 dark:text-red-400" />
            <p className="text-red-700 dark:text-red-300 whitespace-pre-wrap">{props.error}</p>
          </div>
        )}
      </div>
    </div>
  );
}
