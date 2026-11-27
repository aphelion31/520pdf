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
    <div
      style={{
        backgroundColor: "var(--state-ok-bg)",
        borderColor: "var(--state-ok-border)",
      }}
      className="cm-lift flex flex-wrap items-center justify-between gap-4 rounded-xl border p-3.5"
    >
      <div className="flex items-start gap-3">
        <CheckCircle2
          className="mt-0.5 h-5 w-5 shrink-0"
          style={{ color: "var(--state-ok-fg)" }}
        />
        <div>
          <p className="text-sm font-semibold text-[var(--ink-primary)]">Ready: {filename}</p>
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
        className="cm-btn inline-flex items-center gap-2 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-semibold text-white hover:bg-[var(--accent-hover)]"
      >
        <Download className="h-3.5 w-3.5" /> Download
      </button>
    </div>
  );
}
