import React from "react";
import { Check } from "lucide-react";
import { sound } from "../lib/sound";

function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function Button(
  props: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "sm" | "md" | "lg";
    silent?: boolean;
  }
) {
  const { variant = "primary", size = "md", className, silent, onClick, ...rest } = props;
  return (
    <button
      className={cx(
        "pressable inline-flex items-center justify-center gap-2 font-medium rounded-lg border select-none",
        "disabled:opacity-45 disabled:pointer-events-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
        size === "sm" && "h-8 px-3 text-[13px]",
        size === "md" && "h-10 px-4 text-sm",
        size === "lg" && "h-12 px-6 text-[15px]",
        variant === "primary" &&
          "bg-ink-900 text-white border-ink-900 hover:bg-ink-800 dark:bg-white dark:text-ink-950 dark:border-white dark:hover:bg-ink-100",
        variant === "secondary" &&
          "bg-white text-ink-900 border-ink-200 hover:border-ink-300 hover:bg-ink-50 dark:bg-ink-900 dark:text-ink-100 dark:border-white/10 dark:hover:bg-white/5 dark:hover:border-white/20",
        variant === "ghost" &&
          "bg-transparent text-ink-600 border-transparent hover:bg-ink-100 hover:text-ink-900 dark:text-ink-300 dark:hover:bg-white/5 dark:hover:text-white",
        variant === "danger" &&
          "bg-red-600 text-white border-red-600 hover:bg-red-500",
        className
      )}
      onClick={(e) => {
        if (!silent && !e.currentTarget.disabled) sound.press();
        onClick?.(e);
      }}
      {...rest}
    />
  );
}

export function Card(props: React.HTMLAttributes<HTMLDivElement>) {
  const { className, ...rest } = props;
  return (
    <div
      className={cx(
        "rounded-xl border border-ink-200/80 bg-white shadow-card",
        "dark:border-white/10 dark:bg-ink-900",
        className
      )}
      {...rest}
    />
  );
}

export function Field(props: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="block text-[13px] font-medium text-ink-700 dark:text-ink-200 mb-1.5">
        {props.label}
      </span>
      {props.children}
      {props.hint && (
        <span className="block mt-1.5 text-xs leading-relaxed text-ink-500 dark:text-ink-400">
          {props.hint}
        </span>
      )}
    </label>
  );
}

export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  const { className, ...rest } = props;
  return (
    <input
      className={cx(
        "h-10 w-full rounded-lg border border-ink-200 bg-white px-3 text-sm text-ink-900 placeholder:text-ink-400",
        "focus:outline-none focus:ring-2 focus:ring-ring/50 focus:border-ring",
        "dark:border-white/10 dark:bg-white/5 dark:text-ink-50 dark:placeholder:text-ink-500",
        className
      )}
      {...rest}
    />
  );
}

export function Badge(props: { tone?: "ok" | "warn" | "bad" | "neutral" | "info"; children: React.ReactNode }) {
  const tone = props.tone ?? "neutral";
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium tabular",
        tone === "ok" && "border-emerald-600/20 bg-emerald-50 text-emerald-700 dark:bg-emerald-500/10 dark:text-emerald-300 dark:border-emerald-400/20",
        tone === "warn" && "border-amber-600/20 bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:text-amber-300 dark:border-amber-400/20",
        tone === "bad" && "border-red-600/20 bg-red-50 text-red-700 dark:bg-red-500/10 dark:text-red-300 dark:border-red-400/20",
        tone === "info" && "border-primary/25 bg-primary/10 text-primary dark:border-primary/40",
        tone === "neutral" && "border-ink-200 bg-ink-50 text-ink-600 dark:border-white/10 dark:bg-white/5 dark:text-ink-300"
      )}
    >
      {props.children}
    </span>
  );
}

export function Checkbox(props: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  silent?: boolean;
}) {
  function flip() {
    const v = !props.checked;
    if (!props.silent) sound.toggle(v);
    props.onChange(v);
  }
  return (
    <span
      role="checkbox"
      aria-checked={props.checked}
      tabIndex={0}
      onClick={flip}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          flip();
        }
      }}
      className={cx(
        "pressable inline-flex size-4 shrink-0 cursor-pointer items-center justify-center rounded-[5px] border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        props.checked
          ? "border-ink-900 bg-ink-900 text-white dark:border-white dark:bg-white dark:text-ink-950"
          : "border-ink-300 bg-white hover:border-ink-400 dark:border-white/30 dark:bg-white/10 dark:hover:border-white/50"
      )}
    >
      {props.checked && <Check size={12} strokeWidth={3.5} />}
    </span>
  );
}

export function ProgressBar(props: { value: number }) {  const v = Math.max(0, Math.min(100, props.value));
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-ink-100 dark:bg-white/10">
      <div
        className="h-full rounded-full bg-ink-900 dark:bg-white transition-[width] duration-200"
        style={{ width: `${v}%` }}
      />
    </div>
  );
}

export function SectionTitle(props: { title: string; sub?: string }) {
  return (
    <div className="mb-4">
      <h2 className="text-[15px] font-semibold tracking-tight text-ink-900 dark:text-white">
        {props.title}
      </h2>
      {props.sub && (
        <p className="mt-0.5 text-[13px] leading-relaxed text-ink-500 dark:text-ink-400">{props.sub}</p>
      )}
    </div>
  );
}
