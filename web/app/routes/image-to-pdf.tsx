import { useState } from "react";
import { FilePlus2, Loader2 } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";

export default function ImageToPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => match(".jpg,.jpeg,.png,.webp").map((v) => v.file));
  const [pageSize, setPageSize] = useState<"a4" | "letter" | "fit">("a4");
  const [orientation, setOrientation] = useState<"portrait" | "landscape">("portrait");
  const [margin, setMargin] = useState(24);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const handleBuild = async () => {
    if (!files.length) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await runTool("image-to-pdf", files, {
        page_size: pageSize,
        orientation,
        margin: String(margin),
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to build PDF");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Images to PDF"
      description="Combine JPG, PNG, or WebP images into a clean multi-page PDF document."
      category="Convert"
    >
      <div className="surface-card space-y-6 rounded-2xl p-6">
        <FileDropzone
          accept=".jpg,.jpeg,.png,.webp"
          multiple
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
          }}
        />

        <div className="grid gap-4 sm:grid-cols-3">
          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">Page size</label>
            <select
              value={pageSize}
              onChange={(e) => setPageSize(e.target.value as typeof pageSize)}
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            >
              <option value="a4">A4</option>
              <option value="letter">US Letter</option>
              <option value="fit">Fit to image</option>
            </select>
          </div>
          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">Orientation</label>
            <select
              value={orientation}
              onChange={(e) => setOrientation(e.target.value as typeof orientation)}
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            >
              <option value="portrait">Portrait</option>
              <option value="landscape">Landscape</option>
            </select>
          </div>
          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">Margin (pt)</label>
            <input
              type="number"
              min={0}
              max={150}
              value={margin}
              onChange={(e) => setMargin(Number(e.target.value))}
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
          disabled={busy || !files.length}
          onClick={handleBuild}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <FilePlus2 className="h-4 w-4" />}
          {busy ? "Creating PDF…" : `Create PDF from ${files.length} image${files.length === 1 ? "" : "s"}`}
        </button>

        {result && (
          <ResultBanner
            blob={result.blob}
            filename={result.filename}
            originalBytes={files.reduce((s, f) => s + f.size, 0)}
          />
        )}
      </div>
    </ToolShell>
  );
}
