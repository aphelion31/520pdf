import { useEffect, useState } from "react";
import { Gauge, Loader2 } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { formatBytes, runTool, type ToolResponse } from "~/lib/api";

export default function CompressPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [preset, setPreset] = useState<"low" | "medium" | "high">("high");
  const [targetKb, setTargetKb] = useState("250");
  const [removeMetadata, setRemoveMetadata] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);
  const [compareOpen, setCompareOpen] = useState(false);
  const [origUrl, setOrigUrl] = useState("");
  const [outUrl, setOutUrl] = useState("");

  useEffect(() => {
    if (!files[0]) {
      setOrigUrl("");
      return;
    }
    const u = URL.createObjectURL(files[0]);
    setOrigUrl(u);
    return () => URL.revokeObjectURL(u);
  }, [files]);

  useEffect(() => {
    if (!result) {
      setOutUrl("");
      return;
    }
    const u = URL.createObjectURL(result.blob);
    setOutUrl(u);
    return () => URL.revokeObjectURL(u);
  }, [result]);

  const handleCompress = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const opts: Record<string, string> = {
        quality_preset: preset,
        remove_metadata: String(removeMetadata),
      };
      if (targetKb.trim()) opts.target_kb = targetKb.trim();
      const res = await runTool("compress", files, opts);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Compression failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Compress PDF"
      description="Shrink PDF file size using mozjpeg + Ghostscript optimization with side-by-side verification."
      category="Optimize"
    >
      <div className="surface-card space-y-6 rounded-2xl p-6">
        <FileDropzone
          accept=".pdf"
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
            setCompareOpen(false);
          }}
        />

        <div className="grid gap-3 sm:grid-cols-3">
          {[
            {
              id: "low" as const,
              title: "Low compression",
              ratio: 0.75,
              desc: "75% target · Highest visual fidelity",
            },
            {
              id: "medium" as const,
              title: "Balanced",
              ratio: 0.45,
              desc: "45% target · Recommended mix",
            },
            {
              id: "high" as const,
              title: "Maximum shrink",
              ratio: 0.25,
              desc: "250 KB lowest · Smallest file size",
            },
          ].map((item) => {
            const active =
              preset === item.id && (!targetKb.trim() || (item.id === "high" && targetKb.trim() === "250"));
            const estKb = files[0]
              ? item.id === "high"
                ? 250
                : Math.max(250, Math.round((files[0].size / 1000) * item.ratio))
              : null;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => {
                  setPreset(item.id);
                  setTargetKb(item.id === "high" ? "250" : "");
                }}
                className={`rounded-xl border p-3.5 text-left transition-colors ${
                  active
                    ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                    : "border-[var(--border-subtle)] bg-[var(--bg-page)]"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <p className="text-sm font-bold text-[var(--ink-primary)]">{item.title}</p>
                  {estKb !== null && (
                    <span className="rounded-md bg-[var(--bg-surface)] px-2 py-0.5 text-[11px] font-bold text-[var(--accent)] tabular-nums">
                      ≤ {estKb} KB
                    </span>
                  )}
                </div>
                <p className="mt-0.5 text-xs text-[var(--ink-muted)]">{item.desc}</p>
              </button>
            );
          })}
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Custom target size (KB, min 250 KB)
            </label>
            <input
              type="number"
              min={250}
              max={100000}
              value={targetKb}
              onChange={(e) => setTargetKb(e.target.value)}
              placeholder="250"
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            />
            <p className="mt-1 text-[11px] text-[var(--ink-muted)]">
              250 KB is our lowest compression floor.
            </p>
          </div>

          <label className="flex items-center gap-3 self-end rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-3 text-sm">
            <input
              type="checkbox"
              checked={removeMetadata}
              onChange={(e) => setRemoveMetadata(e.target.checked)}
              className="h-4 w-4 accent-[var(--accent)]"
            />
            <span className="font-medium text-[var(--ink-primary)]">Strip Info metadata</span>
          </label>
        </div>

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || !files[0]}
          onClick={handleCompress}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Gauge className="h-4 w-4" />}
          {busy ? "Compressing PDF…" : "Compress PDF"}
        </button>

        {result && files[0] && (
          <div className="space-y-4">
            <ResultBanner
              blob={result.blob}
              filename={result.filename}
              originalBytes={files[0].size}
              note={
                result.compressStatus
                  ? `${result.compressStatus} (${result.compressDpi || 0} dpi, q${result.compressQuality || 0})`
                  : undefined
              }
            />

            <button
              type="button"
              onClick={() => setCompareOpen((v) => !v)}
              className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-2 text-xs font-semibold text-[var(--ink-primary)] hover:border-[var(--accent)]"
            >
              {compareOpen ? "Hide side-by-side comparison" : "Compare original vs compressed"}
            </button>

            {compareOpen && origUrl && outUrl && (
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-1.5">
                  <p className="text-xs font-semibold text-[var(--ink-muted)]">
                    Original ({formatBytes(files[0].size)})
                  </p>
                  <iframe title="Original PDF" src={origUrl} className="h-96 w-full rounded-xl border border-[var(--border-subtle)] bg-white" />
                </div>
                <div className="space-y-1.5">
                  <p className="text-xs font-semibold text-[var(--ink-muted)]">
                    Compressed ({formatBytes(result.blob.size)})
                  </p>
                  <iframe title="Compressed PDF" src={outUrl} className="h-96 w-full rounded-xl border border-[var(--border-subtle)] bg-white" />
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </ToolShell>
  );
}
