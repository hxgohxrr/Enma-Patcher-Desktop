import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, X } from "lucide-react";
import { useT } from "../i18n";
import { Button, Card } from "./ui";

interface Step {
  target: string;
  titleKey: string;
  bodyKey: string;
}

const STEPS: Step[] = [
  { target: "nav-android", titleKey: "tour.s1t", bodyKey: "tour.s1b" },
  { target: "nav-ios", titleKey: "tour.s2t", bodyKey: "tour.s2b" },
  { target: "nav-mods", titleKey: "tour.s3t", bodyKey: "tour.s3b" },
  { target: "nav-cuenta", titleKey: "tour.s4t", bodyKey: "tour.s4b" },
  { target: "nav-settings", titleKey: "tour.s5t", bodyKey: "tour.s5b" },
];

interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}

export function Onboarding(props: { open: boolean; onDone: () => void }) {
  const { open, onDone } = props;
  const { t } = useT();
  const [idx, setIdx] = useState(0);
  const [box, setBox] = useState<Box | null>(null);
  const [tip, setTip] = useState({ top: 0, left: 0 });
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  function measure(target: string): boolean {
    const el = document.querySelector(`[data-tour="${target}"]`) as HTMLElement | null;
    if (!el) return false;
    el.scrollIntoView({ block: "nearest" });
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        const pad = 6;
        const b = { top: r.top - pad, left: r.left - pad, width: r.width + pad * 2, height: r.height + pad * 2 };
        setBox(b);
        const TW = 300;
        const TH = 230;
        const GAP = 12;
        let top = b.top + b.height + GAP;
        if (top + TH > window.innerHeight - 12) top = Math.max(12, b.top - TH - GAP);
        const left = Math.max(12, Math.min(b.left, window.innerWidth - TW - 12));
        setTip({ top, left });
      });
    });
    return true;
  }

  useEffect(() => {
    if (open) setIdx(0);
  }, [open ]);

  useEffect(() => {
    if (!open) return;
    if (!measure(STEPS[idx].target)) {
      if (idx + 1 < STEPS.length) setIdx(idx + 1);
      else doneRef.current();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, idx ]);

  useEffect(() => {
    if (!open) return;
    const onResize = () => measure(STEPS[idx].target);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doneRef.current();
    };
    window.addEventListener("resize", onResize);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("resize", onResize);
      window.removeEventListener("keydown", onKey);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, idx ]);

  if (!open || !box) return null;
  const last = idx === STEPS.length - 1;
  const step = STEPS[idx];

  return (
    <div className="fixed inset-0 z-50">
      <div
        className="absolute rounded-xl transition-[top,left,width,height] duration-300 ease-out"
        style={{
          top: box.top,
          left: box.left,
          width: box.width,
          height: box.height,
          boxShadow:
            "0 0 0 9999px rgba(0,0,0,0.55), 0 0 0 2px var(--ring), 0 8px 30px rgba(0,0,0,0.35)",
        }}
      />
      <Card className="animate-rise fixed z-10 w-[300px] p-4" style={{ top: tip.top, left: tip.left }}>
        <div className="flex items-center justify-between">
          <span className="tabular font-mono text-[11px] text-ink-400">
            {idx + 1} / {STEPS.length}
          </span>
          <button
            onClick={() => doneRef.current()}
            className="pressable rounded-md p-1 text-ink-400 hover:bg-ink-100 hover:text-ink-700 dark:hover:bg-white/10 dark:hover:text-ink-100"
            aria-label={t("tour.skip")}
          >
            <X size={14} />
          </button>
        </div>
        <h3 className="mt-1 text-sm font-semibold tracking-tight text-ink-900 dark:text-white">
          {t(step.titleKey)}
        </h3>
        <p className="mt-1 text-[13px] leading-relaxed text-ink-500 dark:text-ink-400">
          {t(step.bodyKey)}
        </p>
        <div className="mt-3 flex items-center justify-between gap-2">
          <Button size="sm" variant="ghost" disabled={idx === 0} onClick={() => setIdx(idx - 1)}>
            <ArrowLeft size={14} /> {t("tour.back")}
          </Button>
          {last ? (
            <Button size="sm" onClick={() => doneRef.current()}>
              {t("tour.done")}
            </Button>
          ) : (
            <Button size="sm" onClick={() => setIdx(idx + 1)}>
              {t("tour.next")} <ArrowRight size={14} />
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
}
