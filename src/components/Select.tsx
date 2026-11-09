import React, { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
  /** Quiet text on the right, e.g. "newest". */
  hint?: string;
  icon?: React.ReactNode;
  /** Options with the same group sit under one small heading. */
  group?: string;
  /** Draw a thin line above this option. */
  dividerBefore?: boolean;
}

interface SelectProps<T extends string | number> {
  value: T;
  options: SelectOption<T>[];
  onChange: (next: T) => void;
  disabled?: boolean;
  /** Shown when `value` matches no option (e.g. while a list is loading). */
  placeholder?: string;
  size?: "sm" | "md";
  className?: string;
  "aria-label"?: string;
}

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

/** A dropdown that looks like the rest of the app, with full keyboard support. */
export function Select<T extends string | number>({
  value,
  options,
  onChange,
  disabled,
  placeholder = "Select…",
  size = "md",
  className,
  "aria-label": ariaLabel,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [openUp, setOpenUp] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();

  const selectedIndex = options.findIndex((o) => o.value === value);
  const selected = selectedIndex >= 0 ? options[selectedIndex] : undefined;

  const openMenu = useCallback(() => {
    if (disabled) return;
    setActive(Math.max(0, selectedIndex));
    setOpen(true);
  }, [disabled, selectedIndex]);

  const choose = (i: number) => {
    const opt = options[i];
    if (!opt) return;
    setOpen(false);
    if (opt.value !== value) onChange(opt.value);
  };

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  // Open upwards when there's more room above than below.
  useLayoutEffect(() => {
    if (!open || !root.current) return;
    const rect = root.current.getBoundingClientRect();
    const below = window.innerHeight - rect.bottom;
    setOpenUp(below < 300 && rect.top > below);
  }, [open]);

  // Keep the highlighted row in view while moving with the keyboard.
  useEffect(() => {
    if (!open) return;
    list.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        openMenu();
      }
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setActive((a) => Math.min(options.length - 1, a + 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setActive((a) => Math.max(0, a - 1));
        break;
      case "Home":
        e.preventDefault();
        setActive(0);
        break;
      case "End":
        e.preventDefault();
        setActive(options.length - 1);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        choose(active);
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        setOpen(false);
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  };

  return (
    <div ref={root} className={cx("relative inline-block", className)}>
      <button
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-activedescendant={open ? `${id}-${active}` : undefined}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => (open ? setOpen(false) : openMenu())}
        onKeyDown={onKeyDown}
        className={cx(
          "w-full inline-flex items-center justify-between gap-2 bg-raised font-bold text-ink cursor-pointer transition-colors hover:bg-line focus:outline-none focus-visible:ring-2 focus-visible:ring-brand/60 disabled:opacity-50 disabled:hover:bg-raised",
          open && "bg-line",
          size === "sm" ? "h-9 px-3 text-sm rounded-xl" : "h-11 px-4 text-[15px] rounded-xl"
        )}
      >
        <span className={cx("truncate", !selected && "text-faint")}>{selected?.label ?? placeholder}</span>
        <ChevronDown
          className={cx("w-4 h-4 shrink-0 text-soft transition-transform", open && "rotate-180")}
          aria-hidden
        />
      </button>

      {open && (
        <div
          ref={list}
          id={`${id}-list`}
          role="listbox"
          className={cx(
            "absolute left-0 z-30 min-w-full w-max max-w-[22rem] max-h-72 overflow-y-auto p-1.5 rounded-2xl bg-raised shadow-2xl shadow-black/50 ring-1 ring-line/70 animate-toast-in",
            openUp ? "bottom-full mb-2" : "top-full mt-2"
          )}
        >
          {options.map((o, i) => {
            const isSelected = o.value === value;
            const showGroup = o.group && o.group !== options[i - 1]?.group;
            return (
              <React.Fragment key={String(o.value)}>
                {o.dividerBefore && i > 0 && <div className="my-1.5 h-px bg-line/70" role="separator" />}
                {showGroup && (
                  <div className="px-3 pt-2 pb-1 text-xs font-bold uppercase tracking-wide text-faint" role="presentation">
                    {o.group}
                  </div>
                )}
                <div
                  id={`${id}-${i}`}
                  data-index={i}
                  role="option"
                  aria-selected={isSelected}
                  onMouseEnter={() => setActive(i)}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => choose(i)}
                  className={cx(
                    "flex items-center gap-2.5 h-10 px-3 rounded-xl cursor-pointer text-[15px] font-bold select-none",
                    i === active ? "bg-line" : "",
                    isSelected ? "text-brand" : "text-ink"
                  )}
                >
                  {o.icon && <span className="shrink-0 text-soft">{o.icon}</span>}
                  <span className="flex-1 truncate">{o.label}</span>
                  {o.hint && <span className="text-xs font-semibold text-faint shrink-0">{o.hint}</span>}
                  {isSelected && <Check className="w-4 h-4 shrink-0" aria-hidden />}
                </div>
              </React.Fragment>
            );
          })}
        </div>
      )}
    </div>
  );
}
