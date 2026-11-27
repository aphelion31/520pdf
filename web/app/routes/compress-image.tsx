import { useEffect, useState } from "react";
import { ImageDown, Loader2 } from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { formatBytes, runTool, type ToolResponse } from "~/lib/api";

export default function CompressImage() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".png,.jpg,.jpeg,.webp")[0];
    return first ? [first.file] : [];
  });
  const [quality, setQuality] = useState(78);
  const [maxWidth, setMaxWidth] = useState(1920);
  const [outputFormat, setOutputFormat] = useState<"auto" | "jpeg" | "webp" | "png">("jpeg");
  const [targetKb, setTargetKb] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);
  const [origPreview, setOrigPreview] = useState("");
  const [outPreview, setOutPreview] = useState("");

  useEffect(() => {
    if (!files[0]) {
      setOrigPreview("");
      return;
    }
    const u = URL.createObjectURL(files[0]);
    setOrigPreview(u);
    return () => URL.revokeObjectURL(u);
  }, [files]);

  useEffect(() => {
    if (!result) {
      setOutPreview("");
      return;
    }
    const u = URL.createObjectURL(result.blob);
    setOutPreview(u);
    return () => URL.revokeObjectURL(u);
  }, [result]);

  const handleCompress = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const opts: Record<string, string> = {
        max_width: String(maxWidth),
        output_format: outputFormat,
      };
      if (targetKb.trim()) {
        opts.target_kb = targetKb.trim();
      } else {
        opts.quality = String(quality);
      }
      const res = await runTool("compress-image", files, opts);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Image compression failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Compress Image"
      description="Resize and compress JPG, PNG, or WebP images locally with live before/after preview."
      category="Optimize"
    >
      <div className="surface-card space-y-6 rounded-2xl p-6">
        <FileDropzone
          accept=".png,.jpg,.jpeg,.webp"
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
          }}
        />

        <div className="grid gap-5 sm:grid-cols-2">
          <div>
            <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
              <span>Quality</span>
              <span className="tabular-nums">{quality}%</span>
            </div>
            <input
              type="range"
              min={10}
              max={100}
              value={quality}
              onChange={(e) => setQuality(Number(e.target.value))}
              className="mt-2 w-full accent-[var(--accent)]"
            />
          </div>

          <div>
            <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
              <span>Max width</span>
              <span className="tabular-nums">{maxWidth} px</span>
            </div>
            <input
              type="range"
              min={320}
              max={3840}
              step={80}
              value={maxWidth}
              onChange={(e) => setMaxWidth(Number(e.target.value))}
              className="mt-2 w-full accent-[var(--accent)]"
            />
          </div>

          <div>
            <label className="mb-1.5 block text-xs font-semibold text-[var(--ink-secondary)]">
              Output format
            </label>
            <BasecoatSelect
              ariaLabel="Output format"
              value={outputFormat}
              onChange={(v) => setOutputFormat(v as typeof outputFormat)}
              options={[
                { value: "jpeg", label: "JPEG (best photo ratio)" },
                { value: "webp", label: "WebP (lossless modern)" },
                { value: "png", label: "PNG (lossless)" },
                { value: "auto", label: "Keep source format" },
              ]}
            />
          </div>

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Optional target KB (overrides quality)
            </label>
            <input
              type="number"
              min={1}
              max={100000}
              value={targetKb}
              onChange={(e) => setTargetKb(e.target.value)}
              placeholder="Auto from quality slider"
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            />
          </div>
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
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <ImageDown className="h-4 w-4" />}
          {busy ? "Compressing image…" : "Compress Image"}
        </button>

        {result && files[0] && (
          <div className="space-y-4">
            <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0].size} />

            <div className="grid gap-4 sm:grid-cols-2">
              {origPreview && (
                <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3">
                  <p className="mb-2 text-xs font-semibold text-[var(--ink-muted)] tabular-nums">
                    Original · {formatBytes(files[0].size)}
                  </p>
                  <img src={origPreview} alt="Original" className="max-h-64 w-full rounded-lg object-contain" />
                </div>
              )}
              {outPreview && (
                <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3">
                  <p className="mb-2 text-xs font-semibold text-[var(--ink-muted)] tabular-nums">
                    Result · {formatBytes(result.blob.size)}
                  </p>
                  <img src={outPreview} alt="Compressed" className="max-h-64 w-full rounded-lg object-contain" />
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </ToolShell>
  );
}
