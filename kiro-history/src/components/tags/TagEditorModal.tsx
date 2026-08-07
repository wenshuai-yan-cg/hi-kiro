import { useState, useRef } from "react";
import { X, Zap, Tag } from "lucide-react";
import { TagColorPicker } from "./TagColorPicker";
import { api } from "../../api";
import { useToast } from "../ui/Toast";
import type { CreateTagParams, SmartTagRule, TagMeta } from "../../types";

type EditorMode = "create" | "edit" | "smart";

interface TagEditorModalProps {
  mode: EditorMode;
  existing?: TagMeta;
  onClose: () => void;
  onSuccess: () => void;
}

const SMART_RULE_PRESETS = [
  { label: "今週 (7日以内)", rule_type: "recent_days" as const, rule_value: JSON.stringify({ days: 7 }), color: "#60A5FA" },
  { label: "長時間作業 (10分以上)", rule_type: "min_duration" as const, rule_value: JSON.stringify({ seconds: 600 }), color: "#F59E0B" },
  { label: "エージェント活用 (3ループ以上)", rule_type: "min_cycles" as const, rule_value: JSON.stringify({ cycles: 3 }), color: "#A78BFA" },
  { label: "未整理 (タグなし)", rule_type: "no_tags" as const, rule_value: JSON.stringify({}), color: "#EF4444" },
];

export function TagEditorModal({ mode, existing, onClose, onSuccess }: TagEditorModalProps) {
  const toast = useToast();
  const composingRef = useRef(false);

  const [tagName, setTagName] = useState(existing?.tag?.replace(/^#/, "") ?? "");
  const [color, setColor] = useState(existing?.color ?? "#22C55E");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [ruleType, setRuleType] = useState<SmartTagRule["rule_type"]>("recent_days");
  const [ruleParams, setRuleParams] = useState<Record<string, number | string>>({ days: 7 });
  const [loading, setLoading] = useState(false);

  const handlePreset = (preset: typeof SMART_RULE_PRESETS[0]) => {
    const tag = preset.label.split(" ")[0];
    setTagName(tag);
    setColor(preset.color);
    setRuleType(preset.rule_type as SmartTagRule["rule_type"]);
    setRuleParams(JSON.parse(preset.rule_value));
  };

  const handleSubmit = async () => {
    const trimmedTag = tagName.trim();
    if (!trimmedTag) { toast.error("タグ名を入力してください"); return; }
    if (trimmedTag.includes(" ")) { toast.error("タグ名にスペースは使えません"); return; }

    setLoading(true);
    try {
      if (mode === "smart") {
        const ruleValue = JSON.stringify(ruleParams);
        await api.createSmartTag(
          { tag: `#${trimmedTag}`, rule_type: ruleType, rule_value: ruleValue },
          color,
          description
        );
        toast.success(`スマートタグ #${trimmedTag} を作成しました`);
      } else if (mode === "edit" && existing) {
        await api.updateTag(existing.tag, color, description);
        toast.success(`#${trimmedTag} を更新しました`);
      } else {
        const params: CreateTagParams = { tag: trimmedTag, color, description };
        await api.createTag(params);
        toast.success(`#${trimmedTag} を作成しました`);
      }
      onSuccess();
      onClose();
    } catch (e) {
      toast.error(`エラー: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const title = mode === "smart" ? "スマートタグを作成" : mode === "edit" ? "タグを編集" : "新規タグを作成";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="w-full max-w-md rounded-xl shadow-2xl overflow-hidden"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>

        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4" style={{ borderBottom: "1px solid var(--border)" }}>
          <div className="flex items-center gap-2">
            {mode === "smart" ? <Zap size={16} style={{ color: "#F59E0B" }} /> : <Tag size={16} style={{ color: "var(--accent)" }} />}
            <span className="font-semibold text-sm" style={{ color: "var(--text-primary)" }}>{title}</span>
          </div>
          <button onClick={onClose} className="cursor-pointer" style={{ color: "var(--text-muted)" }}><X size={16} /></button>
        </div>

        <div className="px-5 py-4 space-y-4">
          {/* Smart tag presets */}
          {mode === "smart" && (
            <div>
              <label className="block text-xs mb-2 font-medium" style={{ color: "var(--text-muted)" }}>プリセット</label>
              <div className="grid grid-cols-2 gap-2">
                {SMART_RULE_PRESETS.map((p) => (
                  <button key={p.rule_type} onClick={() => handlePreset(p)}
                    className="text-xs px-3 py-2 rounded-lg text-left cursor-pointer transition-colors"
                    style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}
                    onMouseEnter={(e) => (e.currentTarget.style.borderColor = p.color)}
                    onMouseLeave={(e) => (e.currentTarget.style.borderColor = "var(--border)")}>
                    <span className="font-medium">{p.label}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Smart tag rule config */}
          {mode === "smart" && (
            <div>
              <label className="block text-xs mb-2 font-medium" style={{ color: "var(--text-muted)" }}>ルール詳細</label>
              <select value={ruleType} onChange={(e) => setRuleType(e.target.value as SmartTagRule["rule_type"])}
                className="w-full text-xs px-3 py-2 rounded-lg cursor-pointer mb-2"
                style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }}>
                <option value="recent_days">直近N日以内</option>
                <option value="min_duration">N秒以上の作業</option>
                <option value="min_cycles">Nループ以上</option>
                <option value="no_tags">タグなし</option>
                <option value="cwd_prefix">ディレクトリが一致</option>
              </select>
              {ruleType === "recent_days" && (
                <input type="number" min={1} max={365} value={ruleParams.days ?? 7}
                  onChange={(e) => setRuleParams({ days: Number(e.target.value) })}
                  placeholder="日数"
                  className="w-full text-xs px-3 py-2 rounded-lg outline-none"
                  style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
              )}
              {ruleType === "min_duration" && (
                <input type="number" min={60} value={ruleParams.seconds ?? 600}
                  onChange={(e) => setRuleParams({ seconds: Number(e.target.value) })}
                  placeholder="秒数 (例: 600 = 10分)"
                  className="w-full text-xs px-3 py-2 rounded-lg outline-none"
                  style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
              )}
              {ruleType === "min_cycles" && (
                <input type="number" min={1} value={ruleParams.cycles ?? 3}
                  onChange={(e) => setRuleParams({ cycles: Number(e.target.value) })}
                  placeholder="ループ数"
                  className="w-full text-xs px-3 py-2 rounded-lg outline-none"
                  style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
              )}
              {ruleType === "cwd_prefix" && (
                <input type="text" value={ruleParams.prefix ?? ""}
                  onChange={(e) => setRuleParams({ prefix: e.target.value })}
                  placeholder="/home/user/myproject"
                  className="w-full text-xs px-3 py-2 rounded-lg outline-none"
                  style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
              )}
            </div>
          )}

          {/* Tag name */}
          <div>
            <label className="block text-xs mb-1.5 font-medium" style={{ color: "var(--text-muted)" }}>タグ名</label>
            <div className="flex items-center gap-2 px-3 py-2 rounded-lg"
              style={{ background: "var(--bg)", border: "1px solid var(--border)" }}>
              <span className="text-sm font-mono" style={{ color: "var(--text-muted)" }}>#</span>
              <div contentEditable suppressContentEditableWarning
                className="flex-1 text-sm outline-none"
                style={{ color: "var(--text-primary)", minHeight: "1.25rem" }}
                onCompositionStart={() => { composingRef.current = true; }}
                onCompositionEnd={(e) => { composingRef.current = false; setTagName((e.target as HTMLDivElement).innerText.trim()); }}
                onInput={(e) => { if (!composingRef.current) setTagName((e.target as HTMLDivElement).innerText.trim()); }}
                onKeyDown={(e) => { if (!composingRef.current && e.key === "Enter") { e.preventDefault(); handleSubmit(); } }}
                aria-label="タグ名">
                {existing?.tag?.replace(/^#/, "") ?? ""}
              </div>
            </div>
          </div>

          {/* Color */}
          <div>
            <label className="block text-xs mb-2 font-medium" style={{ color: "var(--text-muted)" }}>カラー</label>
            <div className="flex items-center gap-3">
              <div className="w-6 h-6 rounded-full flex-shrink-0" style={{ background: color }} />
              <TagColorPicker value={color} onChange={setColor} />
            </div>
          </div>

          {/* Description */}
          <div>
            <label className="block text-xs mb-1.5 font-medium" style={{ color: "var(--text-muted)" }}>説明（任意）</label>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)}
              placeholder="このタグの用途を説明..."
              rows={2}
              className="w-full text-xs px-3 py-2 rounded-lg outline-none resize-none"
              style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
          </div>

          {/* Preview */}
          <div className="flex items-center gap-2">
            <span className="text-xs" style={{ color: "var(--text-muted)" }}>プレビュー:</span>
            <span className="flex items-center gap-1 text-xs px-2.5 py-1 rounded-full font-medium"
              style={{ background: `${color}20`, color, border: `1px solid ${color}50` }}>
              {mode === "smart" && <Zap size={10} />}
              #{tagName || "タグ名"}
            </span>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 px-5 py-3" style={{ borderTop: "1px solid var(--border)" }}>
          <button onClick={onClose} className="text-sm px-4 py-2 rounded-lg cursor-pointer"
            style={{ background: "var(--border)", color: "var(--text-secondary)" }}>
            キャンセル
          </button>
          <button onClick={handleSubmit} disabled={loading}
            className="text-sm px-4 py-2 rounded-lg cursor-pointer font-medium"
            style={{ background: "var(--accent)", color: "#000", opacity: loading ? 0.6 : 1 }}>
            {loading ? "保存中..." : mode === "edit" ? "更新" : "作成"}
          </button>
        </div>
      </div>
    </div>
  );
}
