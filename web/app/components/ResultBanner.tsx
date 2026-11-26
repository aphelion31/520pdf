import { CheckCircle2, Download } from "lucide-react";
import { downloadBlob, formatBytes } from "~/lib/api";

interface ResultBannerProps {
  blob: Blob;
  filename: string;
  originalBytes?: number;
  note?: string;
}

export function ResultBanner({ blob, filename, originalBytes, note }: ResultBannerProps) {
  const deltaPct =
    originalBytes && originalBytes > 0
      ? Math.round(((originalBytes - blob.size) / originalBytes) * 100)
      : null;

  return (
    <div className="surface-card flex flex-wrap items-center justify-between gap-4 rounded-2xl border-emerald-500/30 bg-emerald-500/[0.06] p-4">
      <div className="flex items-start gap-3">
        <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-600 dark:text-emerald-400" />
        <div>
          <p className="text-sm font-bold text-[var(--ink-primary)]">Ready: {filename}</p>
          <p className="text-xs text-[var(--ink-secondary)] tabular-nums">
            Output {formatBytes(blob.size)}
            {originalBytes ? ` (from ${formatBytes(originalBytes)}` : ""}
            {deltaPct !== null
              ? deltaPct >= 0
                ? `, ${deltaPct}% smaller)`
                : `, +${Math.abs(deltaPct)}%)`
              : originalBytes
                ? ")"
                : ""}
            {note ? ` · ${note}` : ""}
          </p>
        </div>
      </div>

      <button
        type="button"
        onClick={() => downloadBlob(blob, filename)}
        className="inline-flex items-center gap-2 rounded-xl bg-[var(--accent)] px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-[var(--accent-hover)]"
      >
        <Download className="h-4 w-4" /> Download
      </button>
    </div>
  );
}
