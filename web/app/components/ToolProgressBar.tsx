import { useSyncExternalStore } from "react";
import { CheckCircle2, X } from "lucide-react";
import {
  getActiveToolProgress,
  subscribeToolProgress,
  type ToolJobProgress,
} from "~/lib/api";

export function useActiveToolProgress(): ToolJobProgress | null {
  return useSyncExternalStore(subscribeToolProgress, getActiveToolProgress, () => null);
}

interface ToolProgressBarProps {
  toolId?: string;
  className?: string;
}

export function ToolProgressBar({ toolId, className = "" }: ToolProgressBarProps) {
  const job = useActiveToolProgress();
  if (!job) return null;
  if (toolId && job.toolId !== toolId) return null;

  const pct = Math.min(100, Math.max(0, Math.round(job.percent)));
  const elapsedSec = (job.elapsedMs / 1000).toFixed(1);

  return (
    <div
      role="progressbar"
      aria-label={job.stage}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
      className={`surface-card cm-lift rounded-xl px-4 py-3 ${className}`}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          {job.done ? (
            <CheckCircle2 className="h-4 w-4 shrink-0 text-[var(--state-ok-fg)]" />
          ) : (
            <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-[var(--accent)]" />
          )}
          <span className="truncate text-xs font-medium text-[var(--ink-primary)]">
            {job.stage}
          </span>
          <span className="shrink-0 text-[11px] text-[var(--ink-muted)] tabular-nums">
            {elapsedSec}s
          </span>
        </div>

        <div className="flex shrink-0 items-center gap-2.5">
          <span className="text-xs font-semibold text-[var(--ink-primary)] tabular-nums">
            {pct}%
          </span>
          {!job.done && job.cancel && (
            <button
              type="button"
              onClick={job.cancel}
              aria-label="Cancel processing"
              className="cm-btn inline-flex items-center gap-1 rounded-md border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2 py-0.5 text-[11px] font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
            >
              <X className="h-3 w-3" /> Cancel
            </button>
          )}
        </div>
      </div>

      <div className="mt-2.5 h-2 w-full overflow-hidden rounded-full bg-[var(--bg-subtle)]">
        <div
          style={{ width: `${pct}%` }}
          className={`h-full rounded-full transition-[width] duration-150 ease-out ${
            job.done ? "bg-[var(--state-ok-fg)]" : "cm-progress-fill bg-[var(--accent)]"
          }`}
        />
      </div>
    </div>
  );
}
