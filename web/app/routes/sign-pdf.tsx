import React, { useEffect, useRef, useState } from "react";
import { Eraser, Loader2, PenTool, Upload } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderSinglePage, type PdfPageRender } from "~/lib/pdfjs";

export default function SignPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [drawing, setDrawing] = useState(false);
  const [penColor, setPenColor] = useState("#111318");
  const [penWidth, setPenWidth] = useState(2.5);
  const [sigDataUrl, setSigDataUrl] = useState("");
  const [sigText, setSigText] = useState("");
  const [pagePreview, setPagePreview] = useState<PdfPageRender | null>(null);
  const [xPt, setXPt] = useState(60);
  const [yPt, setYPt] = useState(90);
  const [wPt, setWPt] = useState(160);
  const [hPt, setHPt] = useState(55);
  const [targetPage, setTargetPage] = useState(1);
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
    renderSinglePage(f, targetPage, 460)
      .then((p) => {
        if (!cancelled) setPagePreview(p);
      })
      .catch(() => {
        if (!cancelled) setPagePreview(null);
      });
    return () => {
      cancelled = true;
    };
  }, [files, targetPage]);

  const point = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current!;
    const rect = canvas.getBoundingClientRect();
    return [
      ((e.clientX - rect.left) * canvas.width) / rect.width,
      ((e.clientY - rect.top) * canvas.height) / rect.height,
    ] as const;
  };

  const onDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    setDrawing(true);
    canvas.setPointerCapture(e.pointerId);
    ctx.strokeStyle = penColor;
    ctx.lineWidth = penWidth;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    const [x, y] = point(e);
    ctx.moveTo(x, y);
  };

  const onMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing || !canvasRef.current) return;
    const ctx = canvasRef.current.getContext("2d");
    if (!ctx) return;
    const [x, y] = point(e);
    ctx.lineTo(x, y);
    ctx.stroke();
  };

  const onUp = () => {
    if (!drawing || !canvasRef.current) return;
    setDrawing(false);
    setSigDataUrl(canvasRef.current.toDataURL("image/png"));
  };

  const clearPad = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
    setSigDataUrl("");
  };

  const handleUploadSig = async (file?: File) => {
    if (!file || !canvasRef.current) return;
    const bmp = await createImageBitmap(file);
    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    const scale = Math.min(canvas.width / bmp.width, canvas.height / bmp.height);
    ctx.drawImage(bmp, 0, 0, bmp.width * scale, bmp.height * scale);
    bmp.close();
    setSigDataUrl(canvas.toDataURL("image/png"));
  };

  const handleApply = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const opts: Record<string, string> = {
        pages: String(targetPage),
        x: String(xPt),
        y: String(yPt),
        width: String(wPt),
        height: String(hPt),
      };
      if (sigDataUrl) {
        opts.image_base64 = sigDataUrl.split(",")[1] || "";
      } else if (sigText.trim()) {
        opts.text = sigText.trim();
        opts.font_size = "22";
      } else {
        throw new Error("Draw a signature, upload an image, or type signature text first.");
      }
      const res = await runTool("sign", files, opts);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to sign PDF");
    } finally {
      setBusy(false);
    }
  };

  const pageW = pagePreview?.width || 595;
  const pageH = pagePreview?.height || 842;

  return (
    <ToolShell
      title="Sign PDF"
      description="Draw a visible signature, upload an image, or type text and place it accurately on your PDF page."
      category="Edit"
    >
      <div className="grid gap-6 lg:grid-cols-[1.05fr_0.95fr]">
        <div className="surface-card space-y-5 rounded-2xl p-6">
          <FileDropzone
            accept=".pdf"
            files={files}
            onFilesChange={(next) => {
              setFiles(next);
              setResult(null);
            }}
          />

          <div className="space-y-2.5">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="text-xs font-semibold tracking-wider text-[var(--ink-secondary)] uppercase">
                Draw signature
              </p>
              <div className="flex items-center gap-2">
                {["#111318", "#1d4ed8", "#e63228"].map((c) => (
                  <button
                    key={c}
                    type="button"
                    aria-label={`Pen color ${c}`}
                    onClick={() => setPenColor(c)}
                    className={`h-5 w-5 rounded-full border-2 ${
                      penColor === c ? "border-[var(--accent)] scale-110" : "border-transparent"
                    }`}
                    style={{ backgroundColor: c }}
                  />
                ))}
                <input
                  type="range"
                  min={1}
                  max={6}
                  step={0.5}
                  value={penWidth}
                  onChange={(e) => setPenWidth(Number(e.target.value))}
                  aria-label="Pen thickness"
                  className="w-16 accent-[var(--accent)]"
                />
              </div>
            </div>

            <canvas
              ref={canvasRef}
              width={420}
              height={130}
              onPointerDown={onDown}
              onPointerMove={onMove}
              onPointerUp={onUp}
              onPointerCancel={() => setDrawing(false)}
              className="w-full cursor-crosshair touch-none rounded-xl border border-[var(--border-subtle)] bg-white"
            />

            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={clearPad}
                className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs font-semibold hover:border-[var(--accent)]"
              >
                <Eraser className="h-3.5 w-3.5" /> Clear pad
              </button>
              <label className="inline-flex cursor-pointer items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs font-semibold hover:border-[var(--accent)]">
                <Upload className="h-3.5 w-3.5" /> Upload PNG/JPG
                <input
                  type="file"
                  accept="image/png,image/jpeg"
                  className="hidden"
                  onChange={(e) => handleUploadSig(e.target.files?.[0])}
                />
              </label>
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Or typed fallback signature (ASCII)
            </label>
            <input
              type="text"
              value={sigText}
              onChange={(e) => setSigText(e.target.value)}
              placeholder="e.g. Jane Doe"
              className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2 text-sm"
            />
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
            {[
              { label: "Page", val: targetPage, set: setTargetPage, min: 1 },
              { label: "X (pt)", val: xPt, set: setXPt, min: 0 },
              { label: "Y (pt)", val: yPt, set: setYPt, min: 0 },
              { label: "W (pt)", val: wPt, set: setWPt, min: 10 },
              { label: "H (pt)", val: hPt, set: setHPt, min: 10 },
            ].map((ctl) => (
              <div key={ctl.label}>
                <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">
                  {ctl.label}
                </label>
                <input
                  type="number"
                  min={ctl.min}
                  value={ctl.val}
                  onChange={(e) => ctl.set(Number(e.target.value))}
                  className="mt-1 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1.5 text-xs tabular-nums"
                />
              </div>
            ))}
          </div>

          {error && (
            <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
              {error}
            </p>
          )}

          <button
            type="button"
            disabled={busy || !files[0]}
            onClick={handleApply}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <PenTool className="h-4 w-4" />}
            {busy ? "Signing PDF…" : "Sign & Download PDF"}
          </button>

          {result && (
            <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
          )}
        </div>

        <div className="surface-card flex flex-col items-center rounded-2xl p-6">
          <p className="mb-3 self-start text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
            Placement Preview (Page {targetPage})
          </p>
          <div className="relative inline-block overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-white">
            {pagePreview ? (
              <img src={pagePreview.dataUrl} alt={`Page ${targetPage}`} className="max-h-[460px] w-auto" />
            ) : (
              <div className="grid h-[420px] w-[300px] place-items-center text-xs text-neutral-400">
                Upload a PDF to preview signature placement
              </div>
            )}
            {(sigDataUrl || sigText.trim()) && (
              <div
                className="pointer-events-none absolute border border-dashed border-[#e63228]/70 bg-[#e63228]/5 flex items-center justify-center"
                style={{
                  left: `${Math.max(0, Math.min(95, (xPt / pageW) * 100))}%`,
                  bottom: `${Math.max(0, Math.min(95, (yPt / pageH) * 100))}%`,
                  width: `${Math.max(5, Math.min(90, (wPt / pageW) * 100))}%`,
                  height: `${Math.max(4, Math.min(50, (hPt / pageH) * 100))}%`,
                }}
              >
                {sigDataUrl ? (
                  <img src={sigDataUrl} alt="Signature preview" className="h-full w-full object-contain" />
                ) : (
                  <span className="truncate px-1 text-xs font-semibold text-neutral-900">{sigText}</span>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </ToolShell>
  );
}
