import React, { useEffect, useId, useRef, useState } from "react";
import { Check, ChevronsUpDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
  description?: string;
}

interface BasecoatSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  ariaLabel?: string;
  placeholder?: string;
  disabled?: boolean;
  size?: "sm" | "md";
  className?: string;
}

export function BasecoatSelect({
  value,
  onChange,
  options,
  ariaLabel,
  placeholder = "Select option…",
  disabled = false,
  size = "md",
  className = "",
}: BasecoatSelectProps) {
  const [open, setOpen] = useState(false);
  const [activeIdx, setActiveIdx] = useState(-1);
  const rootRef = useRef<HTMLDivElement>(null);
  const listboxId = useId();

  const selectedIndex = options.findIndex((o) => o.value === value);
  const selected = selectedIndex >= 0 ? options[selectedIndex] : undefined;

  useEffect(() => {
    if (!open) return;
    setActiveIdx(selectedIndex >= 0 ? selectedIndex : 0);
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open, selectedIndex]);

  const commit = (idx: number) => {
    const opt = options[idx];
    if (!opt) return;
    onChange(opt.value);
    setOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (disabled || !options.length) return;
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        setOpen(true);
      }
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIdx((i) => (i + 1) % options.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIdx((i) => (i - 1 + options.length) % options.length);
    } else if (e.key === "Home") {
      e.preventDefault();
      setActiveIdx(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setActiveIdx(options.length - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      commit(activeIdx >= 0 ? activeIdx : 0);
    } else if (e.key === "Escape" || e.key === "Tab") {
      setOpen(false);
    }
  };

  return (
    <div ref={rootRef} className={`select relative w-full ${className}`}>
      <button
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => setOpen((prev) => !prev)}
        onKeyDown={handleKeyDown}
        className={`bc-select-trigger w-full ${
          size === "sm" ? "h-8 px-2.5 text-xs" : "h-9 px-3 text-sm"
        }`}
      >
        <span className="truncate text-left font-medium text-[var(--ink-primary)]">
          {selected ? selected.label : placeholder}
        </span>
        <ChevronsUpDown
          className="h-3.5 w-3.5 shrink-0 text-[var(--ink-muted)] opacity-75"
          aria-hidden="true"
        />
      </button>

      {open && (
        <div
          id={listboxId}
          role="listbox"
          data-popover=""
          aria-hidden="false"
          className="bc-select-popover"
        >
          {options.map((opt, idx) => {
            const isSelected = opt.value === value;
            const isActive = idx === activeIdx;
            return (
              <div
                key={opt.value}
                role="option"
                data-value={opt.value}
                aria-selected={isSelected}
                onMouseEnter={() => setActiveIdx(idx)}
                onClick={() => commit(idx)}
                className={`bc-select-option ${isActive ? "bg-[var(--bg-subtle)]" : ""}`}
              >
                <Check
                  className={`h-3.5 w-3.5 shrink-0 text-[var(--accent)] transition-opacity ${
                    isSelected ? "opacity-100" : "opacity-0"
                  }`}
                  aria-hidden="true"
                />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium text-[var(--ink-primary)]">{opt.label}</div>
                  {opt.description && (
                    <div className="truncate text-[11px] text-[var(--ink-muted)]">
                      {opt.description}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
