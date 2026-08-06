// 活動カレンダー・作業時間などで共通の「日/週/月」切り替えタブ

export type CalendarView = "day" | "week" | "month";

interface ViewTabsProps {
  value: CalendarView;
  onChange: (v: CalendarView) => void;
}

export function ViewTabs({ value, onChange }: ViewTabsProps) {
  return (
    <div
      className="flex items-center rounded-lg overflow-hidden"
      style={{ border: "1px solid var(--border)" }}
    >
      {(["day", "week", "month"] as const).map((v) => (
        <button
          key={v}
          onClick={() => onChange(v)}
          className="px-3 py-1 text-xs cursor-pointer"
          style={{
            background: value === v ? "var(--accent)" : "transparent",
            color: value === v ? "#000" : "var(--text-muted)",
            transition: "background 0.15s",
          }}
        >
          {v === "day" ? "日" : v === "week" ? "週" : "月"}
        </button>
      ))}
    </div>
  );
}
