import { useEffect } from "react";
import { X } from "lucide-react";

// ── ModalShell（共通骨格）────────────────────────────────────────────────────
// 確認/入力/プレビューなど全モーダルの共通ヘッダー・本文・フッター構造。
// 用途に応じて maxWidth・rounded を切り替える:
//   確認/警告系 → maxWidth="md" rounded="lg"
//   入力/編集系 → maxWidth="lg" rounded="xl"

export interface ModalShellProps {
  icon?: React.ReactNode;
  iconColor?: string;
  title: string;
  maxWidth?: "sm" | "md" | "lg" | "xl";
  rounded?: "md" | "lg" | "xl";
  onClose: () => void;
  children: React.ReactNode;
  footer: React.ReactNode;
}

export function ModalShell({
  icon,
  iconColor = "var(--accent)",
  title,
  maxWidth = "md",
  rounded = "xl",
  onClose,
  children,
  footer,
}: ModalShellProps) {
  // Escape キーで閉じる
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const maxWClass =
    maxWidth === "sm" ? "max-w-sm"
    : maxWidth === "lg" ? "max-w-lg"
    : maxWidth === "xl" ? "max-w-xl"
    : "max-w-md";

  const roundedClass =
    rounded === "md" ? "rounded-md"
    : rounded === "lg" ? "rounded-lg"
    : "rounded-xl";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className={`w-full ${maxWClass} ${roundedClass} shadow-2xl overflow-hidden`}
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-5 py-4"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          <div className="flex items-center gap-2">
            {icon && (
              <span style={{ color: iconColor, flexShrink: 0 }}>{icon}</span>
            )}
            <span
              className="font-semibold text-sm"
              style={{ color: "var(--text-primary)" }}
            >
              {title}
            </span>
          </div>
          <button
            onClick={onClose}
            className="cursor-pointer p-0.5 rounded"
            style={{ color: "var(--text-muted)" }}
            aria-label="閉じる"
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4">{children}</div>

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-3 px-5 py-3"
          style={{ borderTop: "1px solid var(--border)" }}
        >
          {footer}
        </div>
      </div>
    </div>
  );
}

// ── ConfirmDialog（確認ダイアログ）─────────────────────────────────────────────
// window.confirm() の代替。ModalShell の薄いラッパー。
// danger=true  → 赤ボタン（削除系）
// danger=false → アクセントカラーボタン（統合など、破壊的でない操作）

export interface ConfirmDialogProps {
  title: string;
  message: React.ReactNode;
  confirmLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  confirmLabel = "削除する",
  danger = true,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  // Escape / Enter キー対応
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
      if (e.key === "Enter") { e.preventDefault(); onConfirm(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onConfirm, onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div
        className="w-full max-w-md rounded-lg shadow-2xl overflow-hidden"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-5 py-4"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          <span
            className="font-semibold text-sm"
            style={{ color: "var(--text-primary)" }}
          >
            {title}
          </span>
          <button
            onClick={onCancel}
            className="cursor-pointer"
            style={{ color: "var(--text-muted)" }}
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-5">
          <p className="text-sm" style={{ color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {message}
          </p>
        </div>

        {/* Footer */}
        <div
          className="flex items-center justify-end gap-3 px-5 py-3"
          style={{ borderTop: "1px solid var(--border)" }}
        >
          <button
            onClick={onCancel}
            className="text-sm px-4 py-2 rounded-lg cursor-pointer"
            style={{ background: "var(--border)", color: "var(--text-secondary)" }}
          >
            キャンセル
          </button>
          <button
            onClick={onConfirm}
            className="text-sm px-4 py-2 rounded-lg cursor-pointer font-medium"
            style={{
              background: danger ? "#EF4444" : "var(--accent)",
              color: danger ? "#fff" : "#000",
            }}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
