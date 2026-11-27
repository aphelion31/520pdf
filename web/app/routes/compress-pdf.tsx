import { useEffect, useState } from "react";
import { Gauge, Loader2 } from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { FileDropzone } from "~/components/FileDropzone";
import { HelpTip } from "~/components/HelpTip";
import { ResultBanner } from "~/components/ResultBanner";
import { useActiveToolProgress } from "~/components/ToolProgressBar";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { formatBytes, runTool, type ToolResponse } from "~/lib/api";

export default function CompressPdf() {
  const { match } = useFileVault();
  const activeJob = useActiveToolProgress();
  const compressPct =
    activeJob?.toolId === "compress" ? Math.round(activeJob.percent) : null;
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [preset, setPreset] = useState<"low" | "medium" | "high">("high");
  const [targetKb, setTargetKb] = useState("250");
  const [dpiCap, setDpiCap] = useState("");
  const [qualityCap, setQualityCap] = useState("");
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
      if (dpiCap.trim()) opts.dpi = dpiCap.trim();
      if (qualityCap.trim()) opts.quality = qualityCap.trim();
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
      description="Reduce PDF file size while keeping pages sharp, with instant before/after preview."
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
                className={`cm-btn rounded-xl border p-3.5 text-left ${
                  active
                    ? "border-[var(--ink-primary)] bg-[var(--bg-subtle)]"
                    : "border-[var(--border-subtle)] bg-[var(--bg-page)] hover:border-[var(--border-strong)]"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <p className="text-sm font-semibold text-[var(--ink-primary)]">{item.title}</p>
                  {estKb !== null && (
                    <span className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2 py-0.5 text-[11px] font-medium text-[var(--ink-secondary)] tabular-nums">
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
            <label className="inline-flex items-center gap-1.5 text-xs font-medium text-[var(--ink-secondary)]">
              <span>Target size (KB)</span>
              <HelpTip text="Desired maximum file size in KB. Lowest supported target is 250 KB." />
            </label>
            <input
              type="number"
              min={250}
              max={100000}
              value={targetKb}
              onChange={(e) => setTargetKb(e.target.value)}
              placeholder="250"
              className="cm-input mt-1.5 h-9 px-3 text-sm tabular-nums"
            />
          </div>

          <div className="flex items-center justify-between gap-3 self-end rounded-[10px] border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 h-9 text-sm">
            <label className="flex flex-1 cursor-pointer items-center gap-2.5">
              <input
                type="checkbox"
                checked={removeMetadata}
                onChange={(e) => setRemoveMetadata(e.target.checked)}
                className="h-4 w-4 accent-[var(--accent)]"
              />
              <span className="text-xs font-medium text-[var(--ink-primary)]">
                Remove document info
              </span>
            </label>
            <HelpTip text="Removes hidden properties like author, title, and software name from the PDF." />
          </div>
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <label className="mb-1.5 inline-flex items-center gap-1.5 text-xs font-medium text-[var(--ink-secondary)]">
              <span>Image resolution</span>
              <HelpTip text="Caps how sharp photos remain. Lower DPI makes smaller files; leave on Auto to decide automatically." />
            </label>
            <BasecoatSelect
              ariaLabel="Image resolution"
              value={dpiCap}
              onChange={setDpiCap}
              options={[
                { value: "", label: "Auto (Recommended)" },
                { value: "300", label: "300 DPI" },
                { value: "200", label: "200 DPI" },
                { value: "150", label: "150 DPI" },
                { value: "120", label: "120 DPI" },
                { value: "110", label: "110 DPI" },
                { value: "96", label: "96 DPI" },
                { value: "72", label: "72 DPI" },
                { value: "48", label: "48 DPI" },
              ]}
            />
          </div>
          <div>
            <label className="inline-flex items-center gap-1.5 text-xs font-medium text-[var(--ink-secondary)]">
              <span>Image quality</span>
              <HelpTip text="Controls photo compression quality from 15 (smallest) to 95 (sharpest). Leave blank for automatic tuning." />
            </label>
            <input
              type="number"
              min={15}
              max={95}
              value={qualityCap}
              onChange={(e) => setQualityCap(e.target.value)}
              placeholder="Auto"
              className="cm-input mt-1.5 h-9 px-3 text-sm tabular-nums"
            />
          </div>
        </div>

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-[var(--state-bad-fg)]">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || !files[0]}
          onClick={handleCompress}
          className="cm-btn inline-flex h-10 w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Gauge className="h-4 w-4" />}
          {busy
            ? compressPct !== null
              ? `Compressing PDF… ${compressPct}%`
              : "Compressing PDF…"
            : "Compress PDF"}
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
