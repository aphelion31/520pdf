import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

export interface SlidingTabItem {
  value: string;
  label: string;
  icon?: React.ReactNode;
  dot?: "done" | "none";
}

interface SlidingTabsProps {
  items: SlidingTabItem[];
  value: string;
  onChange: (value: string) => void;
  ariaLabel?: string;
  fullWidth?: boolean;
  radius?: number;
  className?: string;
}

export function SlidingTabs({
  items,
  value,
  onChange,
  ariaLabel,
  fullWidth = false,
  radius = 12,
  className = "",
}: SlidingTabsProps) {
  const barRef = useRef<HTMLDivElement>(null);
  const btnRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [pill, setPill] = useState<{ x: number; w: number; ready: boolean }>({
    x: 0,
    w: 0,
    ready: false,
  });

  const measure = useCallback(() => {
    const idx = items.findIndex((item) => item.value === value);
    const btn = btnRefs.current[idx >= 0 ? idx : 0];
    if (!btn) return;
    setPill({
      x: btn.offsetLeft,
      w: btn.offsetWidth,
      ready: true,
    });
  }, [items, value]);

  useLayoutEffect(() => {
    measure();
  }, [measure]);

  useEffect(() => {
    const bar = barRef.current;
    if (!bar || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(bar);
    return () => ro.disconnect();
  }, [measure]);

  const innerRadius = radius >= 9999 ? 9999 : Math.max(radius - 4, 6);

  return (
    <div
      role="group"
      aria-label={ariaLabel}
      ref={barRef}
      style={{ borderRadius: `${radius}px` }}
      className={`relative border border-[var(--border-subtle)] bg-[var(--bg-subtle)] p-1 overflow-x-auto max-w-full ${
        fullWidth ? "grid" : "inline-flex items-center gap-0.5"
      } ${className}`}
      // When fullWidth, match equal columns dynamically
      {...(fullWidth
        ? {
            style: {
              borderRadius: `${radius}px`,
              gridTemplateColumns: `repeat(${items.length}, minmax(0, 1fr))`,
            },
          }
        : undefined)}
    >
      {pill.ready && (
        <div
          aria-hidden="true"
          style={{
            borderRadius: `${innerRadius}px`,
            width: `${pill.w}px`,
            transform: `translate3d(${pill.x - 4}px, 0, 0)`,
            transition:
              "transform 260ms var(--ease-out), width 260ms var(--ease-out)",
          }}
          className="pointer-events-none absolute top-1 bottom-1 left-1 bg-[var(--ink-primary)] shadow-xs"
        />
      )}

      {items.map((item, idx) => {
        const active = item.value === value;
        return (
          <button
            key={item.value}
            ref={(el) => {
              btnRefs.current[idx] = el;
            }}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(item.value)}
            style={{ borderRadius: `${innerRadius}px` }}
            className={`relative z-10 inline-flex h-8 shrink-0 items-center justify-center gap-1.5 px-3 text-xs font-medium whitespace-nowrap transition-colors duration-150 ${
              active
                ? "text-[var(--bg-surface)]"
                : "text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]"
            }`}
          >
            {item.icon}
            <span>{item.label}</span>
            {item.dot === "done" && (
              <span
                aria-hidden="true"
                className={`h-1.5 w-1.5 rounded-full ${
                  active ? "bg-emerald-400 dark:bg-emerald-600" : "bg-emerald-500"
                }`}
              />
            )}
          </button>
        );
      })}
    </div>
  );
}
