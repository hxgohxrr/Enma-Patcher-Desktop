import { useEffect } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Card } from "./ui";

export function Modal(props: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
}) {
  const { open, onClose } = props;
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/55" onClick={onClose} />
      <Card className="animate-rise thin-scroll relative z-10 max-h-[80vh] w-full max-w-lg overflow-y-auto p-5">
        <div className="mb-2 flex items-center gap-2">
          <h3 className="min-w-0 flex-1 truncate text-sm font-semibold tracking-tight text-ink-900 dark:text-white">
            {props.title}
          </h3>
          <button
            onClick={onClose}
            aria-label="Close"
            className="pressable shrink-0 rounded-md p-1 text-ink-400 hover:bg-ink-100 hover:text-ink-700 dark:hover:bg-white/10 dark:hover:text-ink-100"
          >
            <X size={15} />
          </button>
        </div>
        {props.children}
      </Card>
    </div>,
    document.body
  );
}
