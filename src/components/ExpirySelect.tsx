import { useState } from "react";
import { Dropdown } from "./Dropdown";

const PRESETS = [
  { label: "1 hour", secs: 60 * 60 },
  { label: "4 hours", secs: 4 * 60 * 60 },
  { label: "24 hours", secs: 24 * 60 * 60 },
  { label: "Never", secs: null },
  { label: "Custom…", secs: "custom" as const },
];

const OPTIONS = PRESETS.map((p) => ({
  value: p.secs === null ? "never" : p.secs === "custom" ? "custom" : String(p.secs),
  label: p.label,
}));

interface ExpirySelectProps {
  value: number | null;
  onChange: (secs: number | null) => void;
}


export function ExpirySelect({ value, onChange }: ExpirySelectProps) {
  const [customHours, setCustomHours] = useState(12);
  const [showCustom, setShowCustom] = useState(false);

  function handleSelect(selected: string) {
    if (selected === "custom") {
      setShowCustom(true);
      onChange(customHours * 60 * 60);
      return;
    }
    setShowCustom(false);
    onChange(selected === "never" ? null : Number(selected));
  }

  function handleCustomHours(e: React.ChangeEvent<HTMLInputElement>) {
    const hours = Math.max(1, Number(e.target.value) || 1);
    setCustomHours(hours);
    onChange(hours * 60 * 60);
  }

  const selectValue = showCustom ? "custom" : value === null ? "never" : String(value);

  return (
    <div className="expiry-select">
      <span className="expiry-select__label">Trust for</span>
      <Dropdown
        className="expiry-select__input"
        options={OPTIONS}
        value={selectValue}
        onChange={handleSelect}
        aria-label="Trust for"
      />
      {showCustom ? (
        <input
          type="number"
          min={1}
          className="expiry-select__hours"
          value={customHours}
          onChange={handleCustomHours}
          aria-label="Custom hours"
        />
      ) : null}
    </div>
  );
}
