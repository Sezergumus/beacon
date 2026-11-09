import React from "react";
import { ChevronDown, X, type LucideIcon } from "lucide-react";

const cx = (...parts: (string | false | null | undefined)[]) =>
  parts.filter(Boolean).join(" ");

/* ---------- Layout ---------- */

export const Page: React.FC<{
  title: string;
  subtitle?: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}> = ({ title, subtitle, action, children }) => (
  <div className="h-full overflow-y-auto">
    <div className="max-w-4xl mx-auto px-8 py-8 flex flex-col gap-5">
      <header className="flex items-end justify-between gap-4 mb-1">
        <div>
          <h1 className="text-2xl font-extrabold tracking-tight">{title}</h1>
          {subtitle && <p className="text-soft mt-1">{subtitle}</p>}
        </div>
        {action}
      </header>
      {children}
    </div>
  </div>
);

export const Card: React.FC<{
  title?: string;
  hint?: string;
  action?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}> = ({ title, hint, action, className, children }) => (
  <section className={cx("rounded-3xl bg-surface p-6", className)}>
    {(title || action) && (
      <div className="flex items-start justify-between gap-3 mb-4">
        <div>
          {title && <h2 className="text-lg font-bold">{title}</h2>}
          {hint && <p className="text-sm text-soft mt-0.5">{hint}</p>}
        </div>
        {action}
      </div>
    )}
    {children}
  </section>
);

/** A window over the page. Closes with Esc, the X button, or a click outside it. */
export const Modal: React.FC<{
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: React.ReactNode;
}> = ({ title, subtitle, onClose, children }) => {
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-40 bg-black/60 flex items-center justify-center p-6"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-3xl max-h-[90vh] overflow-y-auto rounded-3xl bg-surface shadow-2xl"
      >
        <header className="flex items-start justify-between gap-4 p-6 pb-4 sticky top-0 bg-surface z-10">
          <div className="min-w-0">
            <h2 className="text-xl font-extrabold tracking-tight truncate">{title}</h2>
            {subtitle && <p className="text-sm text-soft mt-0.5">{subtitle}</p>}
          </div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="p-2 rounded-xl text-soft hover:text-ink hover:bg-raised cursor-pointer shrink-0"
          >
            <X className="w-5 h-5" />
          </button>
        </header>
        <div className="px-6 pb-6 flex flex-col gap-6">{children}</div>
      </div>
    </div>
  );
};

/* ---------- Buttons ---------- */

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

const variantClass: Record<ButtonVariant, string> = {
  primary: "bg-brand text-brand-ink hover:brightness-110",
  secondary: "bg-raised text-ink hover:bg-line",
  ghost: "text-soft hover:text-ink hover:bg-raised",
  danger: "bg-danger/15 text-danger hover:bg-danger/25",
};

const sizeClass: Record<ButtonSize, string> = {
  sm: "h-9 px-3.5 text-sm gap-1.5 rounded-xl",
  md: "h-11 px-5 text-[15px] gap-2 rounded-xl",
  lg: "h-14 px-8 text-lg gap-2.5 rounded-2xl",
};

export const Button: React.FC<
  React.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: ButtonVariant;
    size?: ButtonSize;
    icon?: React.ReactNode;
  }
> = ({ variant = "secondary", size = "md", icon, className, children, ...rest }) => (
  <button
    {...rest}
    className={cx(
      "inline-flex items-center justify-center font-bold whitespace-nowrap transition-all cursor-pointer active:scale-[0.98] disabled:opacity-40 disabled:active:scale-100",
      variantClass[variant],
      sizeClass[size],
      className
    )}
  >
    {icon}
    {children}
  </button>
);

/* ---------- Inputs ---------- */

export const TextInput: React.FC<React.InputHTMLAttributes<HTMLInputElement>> = ({
  className,
  ...rest
}) => (
  <input
    {...rest}
    className={cx(
      "h-11 w-full rounded-xl bg-raised px-4 text-[15px] text-ink placeholder:text-faint focus:outline-none focus:ring-2 focus:ring-brand/60 disabled:opacity-50",
      className
    )}
  />
);

export const Toggle: React.FC<{
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
}> = ({ checked, onChange, label }) => (
  <button
    role="switch"
    aria-checked={checked}
    aria-label={label}
    onClick={() => onChange(!checked)}
    className={cx(
      "relative w-12 h-7 rounded-full transition-colors cursor-pointer shrink-0",
      checked ? "bg-brand" : "bg-line"
    )}
  >
    <span
      className={cx(
        "absolute top-1 left-1 w-5 h-5 rounded-full bg-white shadow transition-transform",
        checked && "translate-x-5"
      )}
    />
  </button>
);

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled,
}: {
  value: T;
  options: { id: T; label: string }[];
  onChange: (next: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="inline-flex p-1 rounded-xl bg-raised gap-1">
      {options.map((o) => (
        <button
          key={o.id}
          disabled={disabled}
          onClick={() => onChange(o.id)}
          className={cx(
            "h-9 px-4 rounded-lg text-sm font-bold transition-colors cursor-pointer disabled:opacity-50",
            value === o.id ? "bg-surface text-ink shadow-sm" : "text-soft hover:text-ink"
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Rows & misc ---------- */

/** A simple settings row: text on the left, control on the right. */
export const Row: React.FC<{
  title: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ title, hint, children }) => (
  <div className="flex items-center justify-between gap-6 py-4 border-b border-line/60 last:border-0 first:pt-0 last:pb-0">
    <div className="min-w-0">
      <div className="font-bold">{title}</div>
      {hint && <div className="text-sm text-soft mt-0.5">{hint}</div>}
    </div>
    {children}
  </div>
);

export const EmptyState: React.FC<{
  icon: LucideIcon;
  title: string;
  text: string;
  action?: React.ReactNode;
}> = ({ icon: Icon, title, text, action }) => (
  <div className="py-10 flex flex-col items-center text-center">
    <div
      className="w-14 h-14 rounded-2xl bg-raised text-soft flex items-center justify-center mb-3"
      aria-hidden
    >
      <Icon className="w-7 h-7" />
    </div>
    <div className="font-bold text-lg">{title}</div>
    <p className="text-soft max-w-sm mt-1">{text}</p>
    {action && <div className="mt-5">{action}</div>}
  </div>
);

export const StatusDot: React.FC<{ tone: "on" | "busy" | "off" }> = ({ tone }) => (
  <span
    className={cx(
      "inline-block w-2.5 h-2.5 rounded-full shrink-0",
      tone === "on" && "bg-brand",
      tone === "busy" && "bg-sun animate-pulse",
      tone === "off" && "bg-faint"
    )}
  />
);

export const ProgressBar: React.FC<{ percent: number; label?: string; detail?: string }> = ({
  percent,
  label,
  detail,
}) => (
  <div>
    {(label || detail) && (
      <div className="flex justify-between text-sm mb-2">
        <span className="font-bold">{label}</span>
        <span className="text-soft">{detail}</span>
      </div>
    )}
    <div className="h-2.5 rounded-full bg-raised overflow-hidden">
      <div
        className="h-full rounded-full bg-brand transition-all duration-300"
        style={{ width: `${Math.max(0, Math.min(100, percent))}%` }}
      />
    </div>
  </div>
);

/** Hides rarely-needed things behind a quiet "Show more" line. */
export const Disclosure: React.FC<{
  label: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
}> = ({ label, children, defaultOpen = false }) => {
  const [open, setOpen] = React.useState(defaultOpen);
  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-sm font-bold text-soft hover:text-ink cursor-pointer"
        aria-expanded={open}
      >
        <ChevronDown className={cx("w-4 h-4 transition-transform", open && "rotate-180")} />
        {label}
      </button>
      {open && <div className="mt-4">{children}</div>}
    </div>
  );
};

export const PlayerHead: React.FC<{ name: string; size?: number }> = ({ name, size = 44 }) => (
  <div
    className="relative rounded-xl bg-raised overflow-hidden shrink-0 flex items-center justify-center font-extrabold text-soft"
    style={{ width: size, height: size }}
  >
    <span>{name.slice(0, 1).toUpperCase()}</span>
    <img
      src={`https://mc-heads.net/avatar/${encodeURIComponent(name)}/${size}`}
      alt=""
      onError={(e) => {
        (e.currentTarget as HTMLImageElement).style.display = "none";
      }}
      className="absolute inset-0 w-full h-full"
    />
  </div>
);
