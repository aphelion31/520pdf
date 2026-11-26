import { useState } from "react";
import { Image as ImageIcon, Loader2 } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";

export default function PdfToImage() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [pages, setPages] = useState("");
  const [dpi, setDpi] = useState(150);
  const [format, setFormat] = useState<"png" | "jpeg" | "webp">("png");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const handleConvert = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await runTool("pdf-to-image", files, {
        pages: pages.trim(),
        dpi: String(dpi),
        format,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Conversion failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="PDF to Image"
      description="Rasterize PDF pages into PNG, JPEG, or WebP images bundled in a ZIP archive."
      category="Convert"
    >
      <div className="surface-card space-y-6 rounded-2xl p-6">
        <FileDropzone
          accept=".pdf"
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
          }}
        />

        <div className="grid gap-4 sm:grid-cols-3">
          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Pages (blank = all)
            </label>
            <input
              type="text"
              value={pages}
              onChange={(e) => setPages(e.target.value)}
              placeholder="e.g. 1,3-5"
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            />
          </div>

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Resolution (DPI)
            </label>
            <select
              value={dpi}
              onChange={(e) => setDpi(Number(e.target.value))}
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            >
              <option value={72}>72 DPI (Screen)</option>
              <option value={150}>150 DPI (Standard)</option>
              <option value={200}>200 DPI (High)</option>
              <option value={300}>300 DPI (Print)</option>
            </select>
          </div>

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Format
            </label>
            <select
              value={format}
              onChange={(e) => setFormat(e.target.value as typeof format)}
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            >
              <option value="png">PNG</option>
              <option value="jpeg">JPEG</option>
              <option value="webp">WebP</option>
            </select>
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
          onClick={handleConvert}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <ImageIcon className="h-4 w-4" />}
          {busy ? "Rendering pages…" : "Export Pages as ZIP"}
        </button>

        {result && (
          <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
        )}
      </div>
    </ToolShell>
  );
}
