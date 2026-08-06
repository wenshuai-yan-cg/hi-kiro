import { useState } from "react";
import { X, ChevronDown, ChevronUp } from "lucide-react";
import type { FilterParams } from "../../types";

interface FilterBarProps {
  filters: FilterParams;
  onChange: (f: FilterParams) => void;
  availableModels: string[];
  availableTags: string[];
}

export function FilterBar({ filters, onChange, availableModels, availableTags }: FilterBarProps) {
  const [expanded, setExpanded] = useState(false);

  const update = (patch: Partial<FilterParams>) => onChange({ ...filters, ...patch });

  const activeCount = [
    filters.date_from,
    filters.model_name,
    (filters.tags?.length ?? 0) > 0,
    filters.starred_only,
  ].filter(Boolean).length;

  return (
    <div style={{ borderBottom: "1px solid var(--border)" }}>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-3 py-1.5 cursor-pointer text-xs"
        style={{ color: "var(--text-secondary)" }}
      >
        <span>
          Filters
          {activeCount > 0 && (
            <span
              className="ml-1.5 px-1.5 py-0.5 rounded-full text-xs"
              style={{ background: "var(--accent)", color: "#000" }}
            >
              {activeCount}
            </span>
          )}
        </span>
        {expanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
      </button>

      {expanded && (
        <div className="px-3 pb-2 space-y-2">
          {/* Date range */}
          <div className="flex gap-2">
            <div className="flex-1">
              <label className="block text-xs mb-0.5" style={{ color: "var(--text-muted)" }}>
                From
              </label>
              <input
                type="date"
                className="w-full text-xs px-2 py-1 rounded"
                style={{
                  background: "var(--bg)",
                  border: "1px solid var(--border)",
                  color: "var(--text-primary)",
                }}
                onChange={(e) =>
                  update({ date_from: e.target.value ? new Date(e.target.value).getTime() : undefined })
                }
              />
            </div>
            <div className="flex-1">
              <label className="block text-xs mb-0.5" style={{ color: "var(--text-muted)" }}>
                To
              </label>
              <input
                type="date"
                className="w-full text-xs px-2 py-1 rounded"
                style={{
                  background: "var(--bg)",
                  border: "1px solid var(--border)",
                  color: "var(--text-primary)",
                }}
                onChange={(e) =>
                  update({ date_to: e.target.value ? new Date(e.target.value).getTime() + 86400000 : undefined })
                }
              />
            </div>
          </div>

          {/* Model */}
          {availableModels.length > 0 && (
            <div>
              <label className="block text-xs mb-0.5" style={{ color: "var(--text-muted)" }}>
                Model
              </label>
              <select
                className="w-full text-xs px-2 py-1 rounded cursor-pointer"
                style={{
                  background: "var(--bg)",
                  border: "1px solid var(--border)",
                  color: "var(--text-primary)",
                }}
                value={filters.model_name ?? ""}
                onChange={(e) => update({ model_name: e.target.value || undefined })}
              >
                <option value="">All models</option>
                {availableModels.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
            </div>
          )}

          {/* Tags */}
          {availableTags.length > 0 && (
            <div>
              <label className="block text-xs mb-0.5" style={{ color: "var(--text-muted)" }}>
                Tags
              </label>
              <div className="flex flex-wrap gap-1">
                {availableTags.slice(0, 20).map((tag) => {
                  const active = filters.tags?.includes(tag);
                  return (
                    <button
                      key={tag}
                      onClick={() => {
                        const current = filters.tags ?? [];
                        update({
                          tags: active ? current.filter((t) => t !== tag) : [...current, tag],
                        });
                      }}
                      className="text-xs px-2 py-0.5 rounded-full cursor-pointer"
                      style={{
                        background: active ? "var(--accent)" : "var(--border)",
                        color: active ? "#000" : "var(--text-secondary)",
                      }}
                    >
                      {tag}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Starred */}
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={filters.starred_only ?? false}
              onChange={(e) => update({ starred_only: e.target.checked || undefined })}
            />
            <span className="text-xs" style={{ color: "var(--text-secondary)" }}>
              Starred only
            </span>
          </label>

          {/* Clear all */}
          {activeCount > 0 && (
            <button
              onClick={() => onChange({})}
              className="flex items-center gap-1 text-xs cursor-pointer"
              style={{ color: "var(--text-muted)" }}
            >
              <X size={11} /> Clear filters
            </button>
          )}
        </div>
      )}
    </div>
  );
}
