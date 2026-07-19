interface SwitchProps {
  label: string;
  checked: boolean;
  onChange: () => void;
  title?: string;
}


export function Switch({ label, checked, onChange, title }: SwitchProps) {
  return (
    <button
      className="switch"
      role="switch"
      aria-checked={checked}
      onClick={onChange}
      title={title}
    >
      <span className="switch__label">{label}</span>
      <span className={`switch__track ${checked ? "switch__track--on" : ""}`}>
        <span className="switch__knob" />
      </span>
    </button>
  );
}
