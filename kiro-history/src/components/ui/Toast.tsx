import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { CheckCircle, XCircle, Info, X } from "lucide-react";

export type ToastType = "success" | "error" | "info";
interface Toast { id: string; type: ToastType; message: string; }
interface ToastContextValue {
  success: (msg: string) => void;
  error: (msg: string) => void;
  info: (msg: string) => void;
}
const ToastContext = createContext<ToastContextValue>({ success: () => {}, error: () => {}, info: () => {} });
export function useToast() { return useContext(ToastContext); }

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const timer = timers.current.get(id);
    if (timer) { clearTimeout(timer); timers.current.delete(id); }
  }, []);

  const add = useCallback((type: ToastType, message: string) => {
    const id = `${Date.now()}-${Math.random()}`;
    setToasts((prev) => [...prev.slice(-4), { id, type, message }]);
    const timer = setTimeout(() => dismiss(id), type === "error" ? 5000 : 3000);
    timers.current.set(id, timer);
  }, [dismiss]);

  const ctx: ToastContextValue = {
    success: (msg) => add("success", msg),
    error: (msg) => add("error", msg),
    info: (msg) => add("info", msg),
  };

  return (
    <ToastContext.Provider value={ctx}>
      {children}
      <ToastContainer toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

function ToastContainer({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: string) => void }) {
  if (toasts.length === 0) return null;
  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 pointer-events-none" role="region" aria-label="Notifications" aria-live="polite">
      {toasts.map((t) => <ToastItem key={t.id} toast={t} onDismiss={onDismiss} />)}
    </div>
  );
}

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: (id: string) => void }) {
  const [visible, setVisible] = useState(false);
  useEffect(() => { const t = setTimeout(() => setVisible(true), 10); return () => clearTimeout(t); }, []);

  const cfg = {
    success: { icon: <CheckCircle size={15} />, color: "#22C55E", border: "rgba(34,197,94,0.35)" },
    error:   { icon: <XCircle size={15} />,    color: "#EF4444", border: "rgba(239,68,68,0.35)" },
    info:    { icon: <Info size={15} />,        color: "#60A5FA", border: "rgba(96,165,250,0.35)" },
  }[toast.type];

  return (
    <div
      className="pointer-events-auto flex items-start gap-3 px-4 py-3 rounded-lg text-sm max-w-sm"
      style={{
        background: "var(--surface)",
        border: `1px solid ${cfg.border}`,
        boxShadow: "0 4px 24px rgba(0,0,0,0.4)",
        transform: visible ? "translateX(0)" : "translateX(120%)",
        opacity: visible ? 1 : 0,
        transition: "transform 0.25s cubic-bezier(0.16,1,0.3,1), opacity 0.2s ease",
      }}
      role="alert"
    >
      <span style={{ color: cfg.color, flexShrink: 0, marginTop: 1 }}>{cfg.icon}</span>
      <span className="flex-1 leading-snug" style={{ color: "var(--text-primary)" }}>{toast.message}</span>
      <button onClick={() => onDismiss(toast.id)} className="cursor-pointer flex-shrink-0 mt-0.5" style={{ color: "var(--text-muted)" }} aria-label="Dismiss">
        <X size={13} />
      </button>
    </div>
  );
}
