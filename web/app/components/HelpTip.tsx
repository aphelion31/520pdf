import { HelpCircle } from "lucide-react";

interface HelpTipProps {
  text?: string;
}

export function HelpTip({ text }: HelpTipProps) {
  if (!text) return null;
  return (
    <span className="group/tip relative inline-flex items-center">
      <button
        type="button"
        aria-label={text}
        onClick={(e) => e.preventDefault()}
        className="inline-flex h-4 w-4 items-center justify-center rounded-full text-[var(--ink-muted)] transition-colors hover:text-[var(--ink-primary)] focus-visible:text-[var(--ink-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
      >
        <HelpCircle className="h-3.5 w-3.5" aria-hidden="true" />
      </button>
      <span
        role="tooltip"
        className="pointer-events-none invisible absolute bottom-full left-1/2 z-50 mb-2 w-64 -translate-x-1/2 rounded-lg bg-[var(--ink-primary)] px-2.5 py-1.5 text-[11px] font-normal normal-case leading-snug text-[var(--bg-surface)] opacity-0 shadow-lg transition-opacity duration-150 group-hover/tip:visible group-hover/tip:opacity-100 group-focus-within/tip:visible group-focus-within/tip:opacity-100"
      >
        {text}
      </span>
    </span>
  );
}
