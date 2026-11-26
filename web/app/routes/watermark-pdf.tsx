import { useEffect, useState } from "react";
import { Loader2, Stamp } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderSinglePage, type PdfPageRender } from "~/lib/pdfjs";

export default function WatermarkPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [text, setText] = useState("CONFIDENTIAL");
  const [fontSize, setFontSize] = useState(44);
  const [rotation, setRotation] = useState(45);
  const [opacity, setOpacity] = useState(0.25);
  const [pagePreview, setPagePreview] = useState<PdfPageRender | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  useEffect(() => {
    const f = files[0];
    if (!f) {
      setPagePreview(null);
      return;
    }
    let cancelled = false;
    renderSinglePage(f, 1, 420)
      .then((p) => {
        if (!cancelled) setPagePreview(p);
      })
      .catch(() => {
        if (!cancelled) setPagePreview(null);
      });
    return () => {
      cancelled = true;
    };
  }, [files]);

  const handleWatermark = async () => {
    if (!files[0] || !text.trim()) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const cx = pagePreview ? Math.round(pagePreview.width * 0.25) : 140;
      const cy = pagePreview ? Math.round(pagePreview.height * 0.45) : 360;
      const res = await runTool("watermark", files, {
        text: text.trim(),
        font_size: String(fontSize),
        rotation: String(rotation),
        opacity: String(opacity),
        x: String(cx),
        y: String(cy),
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Watermark failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Watermark PDF"
      description="Stamp customizable text watermarks onto every page of your PDF."
      category="Edit"
    >
      <div className="grid gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <div className="surface-card space-y-5 rounded-2xl p-6">
          <FileDropzone
            accept=".pdf"
            files={files}
            onFilesChange={(next) => {
              setFiles(next);
              setResult(null);
            }}
          />

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Watermark text
            </label>
            <input
              type="text"
              value={text}
              onChange={(e) => setText(e.target.value)}
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-3">
            <div>
              <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
                <span>Size</span>
                <span className="tabular-nums">{fontSize} pt</span>
              </div>
              <input
                type="range"
                min={14}
                max={96}
                value={fontSize}
                onChange={(e) => setFontSize(Number(e.target.value))}
                className="mt-2 w-full accent-[var(--accent)]"
              />
            </div>

            <div>
              <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
                <span>Angle</span>
                <span className="tabular-nums">{rotation}°</span>
              </div>
              <input
                type="range"
                min={-90}
                max={90}
                value={rotation}
                onChange={(e) => setRotation(Number(e.target.value))}
                className="mt-2 w-full accent-[var(--accent)]"
              />
            </div>

            <div>
              <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
                <span>Opacity</span>
                <span className="tabular-nums">{Math.round(opacity * 100)}%</span>
              </div>
              <input
                type="range"
                min={0.05}
                max={1}
                step={0.05}
                value={opacity}
                onChange={(e) => setOpacity(Number(e.target.value))}
                className="mt-2 w-full accent-[var(--accent)]"
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
            disabled={busy || !files[0] || !text.trim()}
            onClick={handleWatermark}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Stamp className="h-4 w-4" />}
            {busy ? "Applying watermark…" : "Apply Watermark"}
          </button>

          {result && (
            <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
          )}
        </div>

        <div className="surface-card flex flex-col items-center justify-center rounded-2xl p-6">
          <p className="mb-3 self-start text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
            Live Page 1 Preview
          </p>
          <div className="relative flex min-h-[340px] w-full items-center justify-center overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-white p-4">
            {pagePreview ? (
              <img
                src={pagePreview.dataUrl}
                alt="PDF page 1"
                className="max-h-96 w-auto object-contain"
              />
            ) : (
              <p className="text-xs text-neutral-400">Upload a PDF to preview watermark overlay</p>
            )}
            {text.trim() && (
              <span
                className="pointer-events-none absolute font-bold text-neutral-700 select-none"
                style={{
                  fontSize: `${Math.min(44, fontSize * 0.6)}px`,
                  opacity,
                  transform: `rotate(${-rotation}deg)`,
                }}
              >
                {text}
              </span>
            )}
          </div>
        </div>
      </div>
    </ToolShell>
  );
}
