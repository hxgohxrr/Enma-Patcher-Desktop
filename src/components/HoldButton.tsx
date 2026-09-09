import { useRef, useState } from "react";

export function HoldButton(props: {
  durationMs?: number;
  disabled?: boolean;
  onDone: () => void;
  children: React.ReactNode;
}) {
  const duration = props.durationMs ?? 5000;
  const [holding, setHolding] = useState(false);
  const [cycle, setCycle] = useState(0);
  const timer = useRef<number | null>(null);

  function cancel() {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    setHolding(false);
    setCycle((c) => c + 1);
  }

  function start() {
    if (props.disabled || holding) return;
    setHolding(true);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      setHolding(false);
      props.onDone();
    }, duration);
  }

  return (
    <button
      type="button"
      disabled={props.disabled}
      onPointerDown={start}
      onPointerUp={cancel}
      onPointerLeave={cancel}
      onKeyDown={(e) => {
        if ((e.key === " " || e.key === "Enter") && !e.repeat) {
          e.preventDefault();
          start();
        }
      }}
      onKeyUp={cancel}
      onContextMenu={(e) => e.preventDefault()}
      className="pressable relative inline-flex h-11 select-none items-center justify-center gap-2 overflow-hidden rounded-lg border border-red-700 bg-red-600 px-5 text-[15px] font-medium text-white disabled:pointer-events-none disabled:opacity-45"
    >
      <span
        key={cycle}
        aria-hidden="true"
        className="absolute inset-0 origin-left bg-white/25"
        style={{
          transform: holding ? "scaleX(1)" : "scaleX(0)",
          transitionProperty: holding ? "transform" : "none",
          transitionDuration: `${duration}ms`,
          transitionTimingFunction: "linear",
        }}
      />
      <span className="relative">{props.children}</span>
    </button>
  );
}
