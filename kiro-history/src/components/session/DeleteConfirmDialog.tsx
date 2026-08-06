import { AlertTriangle } from "lucide-react";
import type { SessionSummary } from "../../types";
import { ModalShell } from "../ui/ModalShell";

interface DeleteConfirmDialogProps {
  sessions: SessionSummary[];
  onConfirm: () => void;
  onCancel: () => void;
}

export function DeleteConfirmDialog({
  sessions,
  onConfirm,
  onCancel,
}: DeleteConfirmDialogProps) {
  return (
    <ModalShell
      icon={<AlertTriangle size={18} />}
      iconColor="#EF4444"
      title="セッションを削除"
      maxWidth="md"
      rounded="lg"
      onClose={onCancel}
      footer={
        <>
          <button
            onClick={onCancel}
            className="text-sm px-4 py-2 rounded cursor-pointer"
            style={{ background: "var(--border)", color: "var(--text-secondary)" }}
          >
            キャンセル
          </button>
          <button
            onClick={onConfirm}
            className="text-sm px-4 py-2 rounded cursor-pointer font-medium"
            style={{ background: "#EF4444", color: "#fff" }}
          >
            削除する ({sessions.length}件)
          </button>
        </>
      }
    >
      <p className="text-sm mb-3" style={{ color: "var(--text-secondary)" }}>
        以下の{" "}
        <strong style={{ color: "#EF4444" }}>{sessions.length} 件</strong>
        のセッションを<strong>元ファイルごと完全削除</strong>します。
        この操作は取り消せません。
      </p>

      {/* Session list */}
      <div
        className="rounded max-h-48 overflow-y-auto"
        style={{ background: "var(--bg)", border: "1px solid var(--border)" }}
      >
        {sessions.map((s) => (
          <div
            key={s.session_id}
            className="flex items-center gap-2 px-3 py-2"
            style={{ borderBottom: "1px solid var(--border)" }}
          >
            <span
              className="text-xs font-mono flex-shrink-0"
              style={{ color: "var(--text-muted)" }}
            >
              {s.session_id.slice(0, 8)}…
            </span>
            <span
              className="text-xs truncate flex-1"
              style={{ color: "var(--text-primary)" }}
              title={s.title}
            >
              {s.title || "Untitled"}
            </span>
            <span
              className="text-xs flex-shrink-0"
              style={{ color: "var(--text-muted)" }}
            >
              {s.source === "jsonl" ? "JSONL" : "SQLite"}
            </span>
          </div>
        ))}
      </div>

      <p className="text-xs mt-3" style={{ color: "#F59E0B" }}>
        ⚠️ kiro-cli の元ファイルも削除されます。
        使用中のセッション（.lock ファイルあり）はスキップされます。
      </p>
    </ModalShell>
  );
}
