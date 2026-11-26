import React, { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Eraser, Loader2, Move, PenTool, Upload } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderSinglePage, type PdfPageRender } from "~/lib/pdfjs";

interface DragOp {
  mode: "move" | "resize";
  startX: number;
  startY: number;
  origX: number;
  origY: number;
  origW: number;
  origH: number;
}

export default function SignPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const previewBoxRef = useRef<HTMLDivElement>(null);
  const dragOpRef = useRef<DragOp | null>(null);

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
    renderSinglePage(f, targetPage, 480)
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

  const pageW = pagePreview?.width || 595;
  const pageH = pagePreview?.height || 842;
  const totalPages = pagePreview?.totalPages || 1;

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

  const beginBoxDrag = (mode: "move" | "resize", e: React.PointerEvent<HTMLElement>) => {
    e.stopPropagation();
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    dragOpRef.current = {
      mode,
      startX: e.clientX,
      startY: e.clientY,
      origX: xPt,
      origY: yPt,
      origW: wPt,
      origH: hPt,
    };
  };

  const moveBoxDrag = (e: React.PointerEvent<HTMLElement>) => {
    const op = dragOpRef.current;
    const box = previewBoxRef.current;
    if (!op || !box) return;
    const rect = box.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const dxPt = ((e.clientX - op.startX) / rect.width) * pageW;
    const dyPt = ((e.clientY - op.startY) / rect.height) * pageH;

    if (op.mode === "move") {
      const nextX = Math.max(0, Math.min(pageW - op.origW, op.origX + dxPt));
      const nextY = Math.max(0, Math.min(pageH - op.origH, op.origY - dyPt));
      setXPt(Math.round(nextX));
      setYPt(Math.round(nextY));
    } else {
      const nextW = Math.max(24, Math.min(pageW - op.origX, op.origW + dxPt));
      const nextH = Math.max(16, Math.min(op.origY + op.origH, op.origH + dyPt));
      const nextY = Math.max(0, op.origY + (op.origH - nextH));
      setWPt(Math.round(nextW));
      setHPt(Math.round(nextH));
      setYPt(Math.round(nextY));
    }
  };

  const endBoxDrag = () => {
    dragOpRef.current = null;
  };

  const handlePageClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!previewBoxRef.current || !(sigDataUrl || sigText.trim())) return;
    const rect = previewBoxRef.current.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const clickXPt = ((e.clientX - rect.left) / rect.width) * pageW;
    const clickYPtFromBottom = (1 - (e.clientY - rect.top) / rect.height) * pageH;
    setXPt(Math.round(Math.max(0, Math.min(pageW - wPt, clickXPt - wPt / 2))));
    setYPt(Math.round(Math.max(0, Math.min(pageH - hPt, clickYPtFromBottom - hPt / 2))));
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

  return (
    <ToolShell
      title="Sign PDF"
      description="Draw a visible signature, upload an image, or type text and drag it into position on your PDF page."
      category="Edit"
    >
      <div className="grid gap-6 lg:grid-cols-[1.05fr_0.95fr]">
        <div className="surface-card space-y-5 rounded-2xl p-6">
          <FileDropzone
            accept=".pdf"
            files={files}
            onFilesChange={(next) => {
              setFiles(next);
              setTargetPage(1);
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
          <div className="mb-3 flex w-full items-center justify-between gap-2">
            <p className="text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
              Placement Preview · Page {targetPage}
              {totalPages > 1 ? ` / ${totalPages}` : ""}
            </p>
            {totalPages > 1 && (
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  disabled={targetPage <= 1}
                  onClick={() => setTargetPage((p) => Math.max(1, p - 1))}
                  aria-label="Previous page"
                  className="rounded-lg border border-[var(--border-subtle)] p-1 disabled:opacity-40"
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                </button>
                <button
                  type="button"
                  disabled={targetPage >= totalPages}
                  onClick={() => setTargetPage((p) => Math.min(totalPages, p + 1))}
                  aria-label="Next page"
                  className="rounded-lg border border-[var(--border-subtle)] p-1 disabled:opacity-40"
                >
                  <ChevronRight className="h-3.5 w-3.5" />
                </button>
              </div>
            )}
          </div>

          <div
            ref={previewBoxRef}
            onClick={handlePageClick}
            className="relative inline-block select-none overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-white shadow-sm"
          >
            {pagePreview ? (
              <img
                src={pagePreview.dataUrl}
                alt={`Page ${targetPage}`}
                draggable={false}
                className="block max-h-[480px] w-auto"
              />
            ) : (
              <div className="grid h-[420px] w-[300px] place-items-center text-xs text-neutral-400">
                Upload a PDF to preview signature placement
              </div>
            )}
            {(sigDataUrl || sigText.trim()) && (
              <div
                data-testid="sig-drag-box"
                onClick={(e) => e.stopPropagation()}
                onPointerDown={(e) => beginBoxDrag("move", e)}
                onPointerMove={moveBoxDrag}
                onPointerUp={endBoxDrag}
                onPointerCancel={endBoxDrag}
                className="group absolute flex cursor-grab touch-none items-center justify-center rounded border-2 border-dashed border-[#e63228]/80 bg-[#e63228]/5 active:cursor-grabbing"
                style={{
                  left: `${Math.max(0, Math.min(95, (xPt / pageW) * 100))}%`,
                  bottom: `${Math.max(0, Math.min(95, (yPt / pageH) * 100))}%`,
                  width: `${Math.max(5, Math.min(90, (wPt / pageW) * 100))}%`,
                  height: `${Math.max(4, Math.min(50, (hPt / pageH) * 100))}%`,
                }}
              >
                {sigDataUrl ? (
                  <img
                    src={sigDataUrl}
                    alt="Signature preview"
                    draggable={false}
                    className="h-full w-full object-contain"
                  />
                ) : (
                  <span className="truncate px-1 text-xs font-semibold text-neutral-900">
                    {sigText}
                  </span>
                )}
                <span className="pointer-events-none absolute -top-2.5 left-1 hidden items-center gap-0.5 rounded bg-[#e63228] px-1 py-0.5 text-[9px] font-bold text-white group-hover:inline-flex">
                  <Move className="h-2.5 w-2.5" /> Drag
                </span>
                <div
                  data-testid="sig-resize-handle"
                  title="Drag to resize"
                  onPointerDown={(e) => beginBoxDrag("resize", e)}
                  onPointerMove={moveBoxDrag}
                  onPointerUp={endBoxDrag}
                  onPointerCancel={endBoxDrag}
                  className="absolute -right-1.5 -bottom-1.5 h-3.5 w-3.5 cursor-nwse-resize rounded-full bg-[#e63228] ring-2 ring-white"
                />
              </div>
            )}
          </div>

          <p className="mt-3 text-center text-[11px] text-[var(--ink-muted)]">
            Drag signature to reposition · drag bottom-right dot to resize · click anywhere on page to place
          </p>
        </div>
      </div>
    </ToolShell>
  );
}
