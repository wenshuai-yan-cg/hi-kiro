import { useRef, useEffect } from "react";
import { Search, X } from "lucide-react";

interface SearchBarProps {
  value: string;
  onChange: (v: string) => void;
  resultCount?: number;
}

export function SearchBar({ value, onChange, resultCount }: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  // Cmd/Ctrl+F focuses the search bar
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div
      className="flex items-center gap-2 px-3 py-2"
      style={{ borderBottom: "1px solid var(--border)" }}
    >
      <Search size={15} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onChange("")}
        placeholder="Search sessions... (Ctrl+F)"
        className="flex-1 bg-transparent outline-none text-sm"
        style={{ color: "var(--text-primary)" }}
        aria-label="Search sessions"
      />
      {value && (
        <button
          onClick={() => onChange("")}
          className="cursor-pointer rounded"
          style={{ color: "var(--text-muted)" }}
          aria-label="Clear search"
        >
          <X size={14} />
        </button>
      )}
      {resultCount !== undefined && (
        <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
          {resultCount}
        </span>
      )}
    </div>
  );
}
