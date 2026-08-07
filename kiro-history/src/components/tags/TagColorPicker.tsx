import { Check } from "lucide-react";

export const TAG_COLORS = [
  { value: "#22C55E", label: "Green" },
  { value: "#60A5FA", label: "Blue" },
  { value: "#F59E0B", label: "Amber" },
  { value: "#EF4444", label: "Red" },
  { value: "#A78BFA", label: "Purple" },
  { value: "#F472B6", label: "Pink" },
  { value: "#34D399", label: "Teal" },
  { value: "#FB923C", label: "Orange" },
  { value: "#94A3B8", label: "Slate" },
  { value: "#FCD34D", label: "Yellow" },
  { value: "#818CF8", label: "Indigo" },
  { value: "#6EE7B7", label: "Emerald" },
];

interface TagColorPickerProps {
  value: string;
  onChange: (color: string) => void;
}

export function TagColorPicker({ value, onChange }: TagColorPickerProps) {
  return (
    <div className="flex flex-wrap gap-2">
      {TAG_COLORS.map((c) => (
        <button
          key={c.value}
          onClick={() => onChange(c.value)}
          className="w-7 h-7 rounded-full cursor-pointer flex items-center justify-center transition-transform hover:scale-110 focus-visible:ring-2"
          style={{ background: c.value }}
          title={c.label}
          aria-label={c.label}
          aria-pressed={value === c.value}
        >
          {value === c.value && <Check size={14} style={{ color: "#fff" }} strokeWidth={3} />}
        </button>
      ))}
    </div>
  );
}
