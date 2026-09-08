import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow, PhysicalPosition, type Window } from "@tauri-apps/api/window";
import { useT } from "../i18n";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function Titlebar() {
  const { t } = useT();
  const [enabled, setEnabled] = useState(false);
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!inTauri()) return;
    setEnabled(true);
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const win = getCurrentWindow();
        setMaximized(await win.isMaximized());
        unlisten = await win.onResized(async () => {
          try {
            setMaximized(await win.isMaximized());
          } catch {
          }
        });
      } catch {
      }
    })();
    return () => unlisten?.();
  }, []);

  async function withWin(fn: (w: Window) => Promise<void>): Promise<void> {
    try {
      await fn(getCurrentWindow());
    } catch (e) {
      console.error("[titlebar]", e);
    }
  }

  function onDragStart(e: React.PointerEvent): void {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button")) return;
    const target = e.currentTarget as HTMLElement;
    const startX = e.clientX;
    const startY = e.clientY;
    const scale = window.devicePixelRatio || 1;
    const baseX = window.screenX * scale;
    const baseY = window.screenY * scale;
    try {
      target.setPointerCapture(e.pointerId);
    } catch {
    }
    let win: Window | null = null;
    let ended = false;
    let raf = 0;
    let latest: { x: number; y: number } | null = null;
    const cleanup = () => {
      ended = true;
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
      latest = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
    const flush = () => {
      raf = 0;
      if (ended || !win || !latest) return;
      const d = latest;
      latest = null;
      void win
        .setPosition(new PhysicalPosition(baseX + d.x * scale, baseY + d.y * scale))
        .catch(() => {});
    };
    const move = (ev: PointerEvent) => {
      if ((ev.buttons & 1) === 0) {
        cleanup();
        return;
      }
      latest = { x: ev.clientX - startX, y: ev.clientY - startY };
      if (!raf) raf = requestAnimationFrame(flush);
    };
    const up = () => cleanup();
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    void withWin(async (w) => {
      try {
        if (await w.isMaximized()) {
          await w.unmaximize();
          cleanup();
          return;
        }
      } catch {
        cleanup();
        return;
      }
      if (ended) return;
      win = w;
    });
  }

  function onDoubleClick(e: React.MouseEvent): void {
    if ((e.target as HTMLElement).closest("button")) return;
    void withWin(async (w) => {
      if (await w.isMaximized()) await w.unmaximize();
      else await w.maximize();
    });
  }

  const btn =
    "flex h-full w-12 items-center justify-center text-ink-500 transition-colors duration-100 hover:bg-ink-100 hover:text-ink-900 dark:text-ink-400 dark:hover:bg-white/10 dark:hover:text-white";
  const closeBtn =
    "flex h-full w-12 items-center justify-center text-ink-500 transition-colors duration-100 hover:bg-[#e81123] hover:text-white dark:text-ink-400 dark:hover:bg-[#e81123] dark:hover:text-white";

  return (
    <header className="flex h-9 shrink-0 select-none items-center justify-end border-b border-ink-200/80 bg-white dark:border-white/10 dark:bg-ink-900">
      <div
        data-tauri-drag-region
        onPointerDown={onDragStart}
        onDoubleClick={onDoubleClick}
        className="flex h-full flex-1 items-center"
      />
      {enabled && (
        <div className="flex h-full items-stretch">
          <button className={btn} title={t("window.min")} onClick={() => void withWin((w) => w.minimize())}>
            <Minus size={15} />
          </button>
          {maximized ? (
            <button className={btn} title={t("window.restore")} onClick={() => void withWin((w) => w.unmaximize())}>
              <Copy size={13} />
            </button>
          ) : (
            <button className={btn} title={t("window.max")} onClick={() => void withWin((w) => w.maximize())}>
              <Square size={13} />
            </button>
          )}
          <button className={closeBtn} title={t("window.close")} onClick={() => void withWin((w) => w.close())}>
            <X size={16} />
          </button>
        </div>
      )}
    </header>
  );
}
