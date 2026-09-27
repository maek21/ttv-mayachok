import type { ReactNode, CSSProperties, ButtonHTMLAttributes } from "react";

const PATHS = {
  mic: <><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0" /><path d="M12 18v3" /></>,
  home: <><path d="M4 11l8-7 8 7" /><path d="M6 10v10h12V10" /></>,
  clock: <><circle cx="12" cy="12" r="8.5" /><path d="M12 7.5V12l3 2" /></>,
  book: <><path d="M5 4.5h10a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3z" /><path d="M5 17a3 3 0 0 1 3-3h10" /></>,
  gear: <><circle cx="12" cy="12" r="3" /><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" /></>,
  search: <><circle cx="11" cy="11" r="6.5" /><path d="M20 20l-4.2-4.2" /></>,
  check: <path d="M5 12l5 5L20 7" />,
  copy: <><rect x="8" y="8" width="12" height="12" rx="2.5" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></>,
  insert: <><path d="M4 12h12" /><path d="M12 7l5 5-5 5" /><path d="M20 5v14" /></>,
  trash: <><path d="M4 7h16" /><path d="M9 7V4.5h6V7" /><path d="M6.5 7l1 13h9l1-13" /></>,
  star: <path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" />,
  play: <path d="M8 5.5v13l11-6.5z" />,
  pause: <path d="M9 5.5v13M15 5.5v13" />,
  chev: <path d="M9 6l6 6-6 6" />,
  down: <path d="M6 9l6 6 6-6" />,
  plus: <path d="M12 5v14M5 12h14" />,
  lock: <><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></>,
  alert: <><path d="M12 4l9 16H3z" /><path d="M12 10v4M12 17.2v.1" /></>,
  x: <path d="M6 6l12 12M18 6L6 18" />,
  min: <path d="M5 12h14" />,
  max: <rect x="5.5" y="5.5" width="13" height="13" rx="1.5" />,
  shield: <path d="M12 3l7.5 3v6c0 4.5-3.2 7.8-7.5 9-4.3-1.2-7.5-4.5-7.5-9V6z" />,
  keyboard: <><rect x="2.5" y="6" width="19" height="12" rx="2.5" /><path d="M6.5 10h.01M10 10h.01M13.5 10h.01M17 10h.01M7.5 14h9" /></>,
  info: <><circle cx="12" cy="12" r="8.5" /><path d="M12 11v5M12 8h.01" /></>,
  download: <><path d="M12 4v11" /><path d="M7 10.5l5 5 5-5" /><path d="M5 20h14" /></>,
  cloud: <path d="M7 18h10a4 4 0 0 0 .6-8A6 6 0 0 0 6 9.5 4.3 4.3 0 0 0 7 18z" />,
  cpu: <><rect x="6" y="6" width="12" height="12" rx="2" /><path d="M9.5 9.5h5v5h-5zM9 2.5v3.5M15 2.5v3.5M9 18v3.5M15 18v3.5M2.5 9h3.5M2.5 15h3.5M18 9h3.5M18 15h3.5" /></>,
  arrowr: <><path d="M5 12h14" /><path d="M13 6l6 6-6 6" /></>,
  folder: <path d="M3.5 6.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v8.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />,
  stop: <rect x="6.5" y="6.5" width="11" height="11" rx="2" />,
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 18, stroke = 2, style }: { name: IconName; size?: number; stroke?: number; style?: CSSProperties }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={stroke}
      strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0, ...style }} aria-hidden="true">
      {PATHS[name]}
    </svg>
  );
}

export function Logo({ size = 28 }: { size?: number }) {
  const r = Math.round(size * 0.28);
  return (
    <div style={{
      width: size, height: size, borderRadius: Math.round(size * 0.3), background: "#000", border: "1px solid #34343a",
      boxSizing: "border-box", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0,
    }}>
      <div style={{
        width: r, height: r, borderRadius: r, background: "var(--accent)",
        boxShadow: `0 0 ${Math.round(size * 0.35)}px var(--accent)`, animation: "pulse 2.4s ease-in-out infinite",
      }} />
    </div>
  );
}

type BtnKind = "primary" | "accent" | "ghost" | "subtle" | "danger" | "link";
interface BtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  kind?: BtnKind;
  icon?: IconName;
  size?: "sm" | "md" | "lg";
}
export function Button({ kind = "primary", icon, size = "md", children, className = "", ...rest }: BtnProps) {
  return (
    <button type="button" className={`btn ${kind} ${size !== "md" ? size : ""} ${className}`} {...rest}>
      {icon && <Icon name={icon} size={16} />}
      {children && <span>{children}</span>}
    </button>
  );
}

export function IconButton({ icon, label, on, onClick }: { icon: IconName; label: string; on?: boolean; onClick?: () => void }) {
  return (
    <button type="button" className={`iconbtn ${on ? "on" : ""}`} aria-label={label} title={label} onClick={onClick}>
      <Icon name={icon} size={16} />
    </button>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label?: string }) {
  return (
    <button type="button" role="switch" aria-checked={checked} aria-label={label ?? "Переключатель"} className="switch"
      onClick={() => onChange(!checked)} />
  );
}

export function Select<T extends string>({ value, options, onChange, width = 260, label }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; width?: number; label?: string;
}) {
  return (
    <select className="select" style={{ width }} value={value} aria-label={label} onChange={(e) => onChange(e.target.value as T)}>
      {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  );
}

export function Segmented<T extends string>({ value, options, onChange }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void;
}) {
  return (
    <div className="seg" role="radiogroup">
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value}
          className={o.value === value ? "on" : ""} onClick={() => onChange(o.value)}>{o.label}</button>
      ))}
    </div>
  );
}

export function Keys({ keys, big }: { keys: string[]; big?: boolean }) {
  if (big) {
    return (
      <div className="row" style={{ gap: 16 }}>
        {keys.map((k, i) => (
          <span key={k} className="row" style={{ gap: 16 }}>
            {i > 0 && <span style={{ fontSize: 28, color: "var(--txt3)" }}>+</span>}
            <span className="key big">{k}</span>
          </span>
        ))}
      </div>
    );
  }
  return (
    <span className="keys">
      {keys.map((k, i) => (
        <span key={k} className="keys">
          {i > 0 && <span className="plus">+</span>}
          <span className="key">{k}</span>
        </span>
      ))}
    </span>
  );
}

export function Group({ children }: { children: ReactNode }) {
  return <div className="group">{children}</div>;
}

export function SRow({ label, desc, children }: { label: ReactNode; desc?: ReactNode; children?: ReactNode }) {
  return (
    <div className="srow">
      <div className="grow col" style={{ gap: 3 }}>
        <span className="label">{label}</span>
        {desc && <span className="desc">{desc}</span>}
      </div>
      {children}
    </div>
  );
}

export function Meter({ level, width = 280 }: { level: number; width?: number | string }) {
  const pct = Math.min(100, Math.round(Math.sqrt(level) * 260));
  return <div className="meter" style={{ width }}><div style={{ width: `${pct}%` }} /></div>;
}

export function Progress({ value }: { value: number }) {
  return <div className="progress"><div style={{ width: `${Math.round(value * 100)}%` }} /></div>;
}
