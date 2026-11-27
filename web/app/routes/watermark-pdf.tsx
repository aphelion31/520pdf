import React, { useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, ImagePlus, Loader2, Stamp, Trash2, Type } from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderSinglePage, type PdfPageRender } from "~/lib/pdfjs";

const COLOR_PRESETS = [
  { hex: "#B7281E", label: "Crimson" },
  { hex: "#475569", label: "Slate" },
  { hex: "#111318", label: "Obsidian" },
  { hex: "#2563EB", label: "Blue" },
  { hex: "#059669", label: "Emerald" },
  { hex: "#D97706", label: "Amber" },
];

type PatternPreset = "single" | "2x2" | "3x3" | "4x4" | "custom";
type CustomMode = "count" | "grid";

export default function WatermarkPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });

  const [mode, setMode] = useState<"text" | "image">("text");
  const [text, setText] = useState("CONFIDENTIAL");
  const [color, setColor] = useState("#475569");
  const [fontSize, setFontSize] = useState(34);

  const [imageDataUrl, setImageDataUrl] = useState("");
  const [imageName, setImageName] = useState("");
  const [imageDims, setImageDims] = useState<{ w: number; h: number } | null>(null);
  const [imageWidthPt, setImageWidthPt] = useState(130);

  const [pattern, setPattern] = useState<PatternPreset>("single");
  const [customMode, setCustomMode] = useState<CustomMode>("count");
  const [customCount, setCustomCount] = useState(6);
  const [customCols, setCustomCols] = useState(3);
  const [customRows, setCustomRows] = useState(4);

  const [rotation, setRotation] = useState(35);
  const [opacity, setOpacity] = useState(0.3);
  const [pagesFilter, setPagesFilter] = useState("");
  const [previewPageNum, setPreviewPageNum] = useState(1);
  const [pagePreview, setPagePreview] = useState<PdfPageRender | null>(null);
  const previewImgRef = useRef<HTMLImageElement>(null);
  const [previewPxW, setPreviewPxW] = useState(340);

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
    renderSinglePage(f, previewPageNum, 460)
      .then((p) => {
        if (!cancelled) setPagePreview(p);
      })
      .catch(() => {
        if (!cancelled) setPagePreview(null);
      });
    return () => {
      cancelled = true;
    };
  }, [files, previewPageNum]);

  useEffect(() => {
    const el = previewImgRef.current;
    if (!el) return;
    const update = () => {
      if (el.clientWidth > 0) setPreviewPxW(el.clientWidth);
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [pagePreview]);

  const resolvedGrid = useMemo(() => {
    if (pattern === "single") return { cols: 1, rows: 1, total: 1 };
    if (pattern === "2x2") return { cols: 2, rows: 2, total: 4 };
    if (pattern === "3x3") return { cols: 3, rows: 3, total: 9 };
    if (pattern === "4x4") return { cols: 4, rows: 4, total: 16 };
    if (customMode === "grid") {
      const c = Math.max(1, Math.min(8, Math.round(customCols || 1)));
      const r = Math.max(1, Math.min(10, Math.round(customRows || 1)));
      return { cols: c, rows: r, total: Math.min(64, c * r) };
    }
    const n = Math.max(1, Math.min(48, Math.round(customCount || 1)));
    const cols = Math.max(1, Math.ceil(Math.sqrt(n)));
    const rows = Math.max(1, Math.ceil(n / cols));
    return { cols, rows, total: n };
  }, [pattern, customMode, customCount, customCols, customRows]);

  const previewSlots = useMemo(() => {
    const slots: Array<{ leftPct: number; topPct: number }> = [];
    for (let r = 0; r < resolvedGrid.rows; r++) {
      for (let c = 0; c < resolvedGrid.cols; c++) {
        if (slots.length >= resolvedGrid.total) break;
        slots.push({
          leftPct: ((c + 0.5) / resolvedGrid.cols) * 100,
          topPct: ((r + 0.5) / resolvedGrid.rows) * 100,
        });
      }
    }
    return slots;
  }, [resolvedGrid]);

  const handleImagePick = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    setError(null);
    try {
      const bmp = await createImageBitmap(file);
      const maxDim = 1200;
      const scale = Math.min(1, maxDim / Math.max(bmp.width, bmp.height));
      const w = Math.max(1, Math.round(bmp.width * scale));
      const h = Math.max(1, Math.round(bmp.height * scale));
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("Canvas unavailable");
      ctx.drawImage(bmp, 0, 0, w, h);
      bmp.close();
      setImageDataUrl(canvas.toDataURL("image/png"));
      setImageName(file.name);
      setImageDims({ w, h });
      setResult(null);
    } catch {
      setError("Could not read watermark image. Choose a valid PNG, JPG, or WebP file.");
    }
  };

  const canApply = Boolean(files[0]) && (mode === "text" ? Boolean(text.trim()) : Boolean(imageDataUrl));

  const handleWatermark = async () => {
    if (!canApply || !files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const opts: Record<string, string> = {
        rotation: String(rotation),
        opacity: String(opacity),
        layout: resolvedGrid.total === 1 ? "center" : "grid",
      };
      if (pagesFilter.trim()) opts.pages = pagesFilter.trim();

      if (pattern === "custom") {
        if (customMode === "count") {
          opts.count = String(resolvedGrid.total);
        } else {
          opts.cols = String(resolvedGrid.cols);
          opts.rows = String(resolvedGrid.rows);
        }
      } else if (resolvedGrid.total > 1) {
        opts.cols = String(resolvedGrid.cols);
        opts.rows = String(resolvedGrid.rows);
      }

      if (mode === "image") {
        opts.image_base64 = imageDataUrl.split(",")[1] || "";
        opts.width = String(imageWidthPt);
      } else {
        opts.text = text.trim();
        opts.font_size = String(fontSize);
        opts.color = color;
      }

      const res = await runTool("watermark", files, opts);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Watermark failed");
    } finally {
      setBusy(false);
    }
  };

  const pageW = pagePreview?.width || 595;
  const totalPages = pagePreview?.totalPages || 1;
  const pxPerPt = previewPxW / pageW;

  return (
    <ToolShell
      title="Watermark PDF"
      description="Stamp single or multi-tiled custom color text or uploaded image/logo watermarks across PDF pages."
      category="Edit"
    >
      <div className="grid gap-6 lg:grid-cols-[1.08fr_0.92fr]">
        <div className="surface-card space-y-5 rounded-2xl p-6">
          <FileDropzone
            accept=".pdf"
            files={files}
            onFilesChange={(next) => {
              setFiles(next);
              setPreviewPageNum(1);
              setResult(null);
            }}
          />

          <div className="grid grid-cols-2 gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-1">
            {[
              { id: "text" as const, label: "Text Watermark", icon: Type },
              { id: "image" as const, label: "Upload Image / Logo", icon: ImagePlus },
            ].map((tab) => {
              const Icon = tab.icon;
              const active = mode === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => {
                    setMode(tab.id);
                    setResult(null);
                  }}
                  className={`inline-flex items-center justify-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold transition-colors ${
                    active
                      ? "bg-[var(--bg-surface)] text-[var(--accent)] shadow-xs"
                      : "text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]"
                  }`}
                >
                  <Icon className="h-3.5 w-3.5" aria-hidden="true" />
                  <span>{tab.label}</span>
                </button>
              );
            })}
          </div>

          {mode === "text" ? (
            <div className="space-y-4">
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Watermark text
                </label>
                <input
                  type="text"
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                  placeholder="CONFIDENTIAL"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <label className="mb-1.5 block text-xs font-semibold text-[var(--ink-secondary)]">
                    Text color
                  </label>
                  <div className="flex flex-wrap items-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-2">
                    {COLOR_PRESETS.map((c) => (
                      <button
                        key={c.hex}
                        type="button"
                        aria-label={`Watermark color ${c.label}`}
                        title={c.label}
                        onClick={() => setColor(c.hex)}
                        className={`h-5 w-5 rounded-full border-2 transition-transform ${
                          color.toUpperCase() === c.hex
                            ? "scale-110 border-[var(--accent)] ring-2 ring-[var(--accent)]/30"
                            : "border-transparent"
                        }`}
                        style={{ backgroundColor: c.hex }}
                      />
                    ))}
                    <label className="ml-auto inline-flex cursor-pointer items-center gap-1.5 text-[11px] font-mono text-[var(--ink-muted)]">
                      <input
                        type="color"
                        value={color}
                        onChange={(e) => setColor(e.target.value)}
                        aria-label="Custom watermark color"
                        className="h-6 w-7 cursor-pointer rounded border-0 bg-transparent p-0"
                      />
                      <span>{color.toUpperCase()}</span>
                    </label>
                  </div>
                </div>

                <div>
                  <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
                    <span>Font size</span>
                    <span className="tabular-nums">{fontSize} pt</span>
                  </div>
                  <input
                    type="range"
                    min={12}
                    max={96}
                    value={fontSize}
                    onChange={(e) => setFontSize(Number(e.target.value))}
                    className="mt-3 w-full accent-[var(--accent)]"
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              {!imageDataUrl ? (
                <label className="flex cursor-pointer flex-col items-center justify-center rounded-2xl border-2 border-dashed border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-6 text-center transition-colors hover:border-[var(--accent)]/60">
                  <ImagePlus className="h-6 w-6 text-[var(--accent)]" />
                  <span className="mt-2 text-xs font-semibold text-[var(--ink-primary)]">
                    Upload watermark PNG, JPG, or WebP
                  </span>
                  <span className="mt-0.5 text-[11px] text-[var(--ink-muted)]">
                    Transparent PNG logos work best across multiple tiles
                  </span>
                  <input
                    type="file"
                    accept="image/png,image/jpeg,image/webp"
                    className="hidden"
                    onChange={handleImagePick}
                  />
                </label>
              ) : (
                <div className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3">
                  <img
                    src={imageDataUrl}
                    alt="Uploaded watermark"
                    className="h-12 w-12 rounded-lg border border-[var(--border-subtle)] bg-white object-contain p-1"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-xs font-semibold text-[var(--ink-primary)]">
                      {imageName}
                    </p>
                    {imageDims && (
                      <p className="text-[11px] text-[var(--ink-muted)] tabular-nums">
                        {imageDims.w} × {imageDims.h} px
                      </p>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      setImageDataUrl("");
                      setImageName("");
                      setImageDims(null);
                    }}
                    aria-label="Remove watermark image"
                    className="rounded-lg p-1.5 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-red-500"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
              )}

              <div>
                <div className="flex justify-between text-xs font-semibold text-[var(--ink-secondary)]">
                  <span>Watermark image width</span>
                  <span className="tabular-nums">{imageWidthPt} pt</span>
                </div>
                <input
                  type="range"
                  min={40}
                  max={360}
                  step={5}
                  value={imageWidthPt}
                  onChange={(e) => setImageWidthPt(Number(e.target.value))}
                  className="mt-2 w-full accent-[var(--accent)]"
                />
              </div>
            </div>
          )}

          <div className="space-y-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-4">
            <div className="grid gap-3 sm:grid-cols-2">
              <div>
                <label className="mb-1.5 block text-xs font-semibold text-[var(--ink-secondary)]">
                  Quantity & Layout
                </label>
                <BasecoatSelect
                  ariaLabel="Watermark quantity and layout"
                  value={pattern}
                  onChange={(v) => setPattern(v as PatternPreset)}
                  options={[
                    { value: "single", label: "Single (1 · Center)" },
                    { value: "2x2", label: "2×2 Grid (4)" },
                    { value: "3x3", label: "3×3 Grid (9)" },
                    { value: "4x4", label: "4×4 Grid (16)" },
                    { value: "custom", label: "Custom Count / Grid…" },
                  ]}
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Pages (blank = all)
                </label>
                <input
                  type="text"
                  value={pagesFilter}
                  onChange={(e) => setPagesFilter(e.target.value)}
                  placeholder="e.g. 1,3-5"
                  className="mt-1.5 h-10 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 text-sm"
                />
              </div>
            </div>

            {pattern === "custom" && (
              <div className="grid gap-3 border-t border-[var(--border-subtle)] pt-3 sm:grid-cols-3">
                <div>
                  <label className="mb-1 block text-[11px] font-semibold text-[var(--ink-muted)]">
                    Specify by
                  </label>
                  <BasecoatSelect
                    size="sm"
                    ariaLabel="Custom quantity mode"
                    value={customMode}
                    onChange={(v) => setCustomMode(v as CustomMode)}
                    options={[
                      { value: "count", label: "Exact count (N)" },
                      { value: "grid", label: "Columns × Rows" },
                    ]}
                  />
                </div>

                {customMode === "count" ? (
                  <div className="sm:col-span-2">
                    <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">
                      Number of stamps (1–48)
                    </label>
                    <input
                      type="number"
                      min={1}
                      max={48}
                      value={customCount}
                      onChange={(e) => setCustomCount(Math.max(1, Math.min(48, Number(e.target.value) || 1)))}
                      className="mt-1 h-8 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2.5 text-xs tabular-nums"
                    />
                  </div>
                ) : (
                  <>
                    <div>
                      <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">
                        Columns (1–8)
                      </label>
                      <input
                        type="number"
                        min={1}
                        max={8}
                        value={customCols}
                        onChange={(e) => setCustomCols(Math.max(1, Math.min(8, Number(e.target.value) || 1)))}
                        className="mt-1 h-8 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2.5 text-xs tabular-nums"
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">
                        Rows (1–10)
                      </label>
                      <input
                        type="number"
                        min={1}
                        max={10}
                        value={customRows}
                        onChange={(e) => setCustomRows(Math.max(1, Math.min(10, Number(e.target.value) || 1)))}
                        className="mt-1 h-8 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2.5 text-xs tabular-nums"
                      />
                    </div>
                  </>
                )}
              </div>
            )}
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
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
            disabled={busy || !canApply}
            onClick={handleWatermark}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Stamp className="h-4 w-4" />}
            {busy
              ? "Applying watermark…"
              : `Apply ${resolvedGrid.total} ${mode === "image" ? "Logo" : "Text"} Watermark${resolvedGrid.total === 1 ? "" : "s"}`}
          </button>

          {result && (
            <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
          )}
        </div>

        <div className="surface-card flex flex-col items-center rounded-2xl p-6">
          <div className="mb-3 flex w-full items-center justify-between gap-2">
            <p className="text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
              Live Preview · Page {previewPageNum}
              {totalPages > 1 ? ` / ${totalPages}` : ""} · {resolvedGrid.total}×
            </p>
            {totalPages > 1 && (
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  disabled={previewPageNum <= 1}
                  onClick={() => setPreviewPageNum((p) => Math.max(1, p - 1))}
                  aria-label="Previous preview page"
                  className="rounded-lg border border-[var(--border-subtle)] p-1 disabled:opacity-40"
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                </button>
                <button
                  type="button"
                  disabled={previewPageNum >= totalPages}
                  onClick={() => setPreviewPageNum((p) => Math.min(totalPages, p + 1))}
                  aria-label="Next preview page"
                  className="rounded-lg border border-[var(--border-subtle)] p-1 disabled:opacity-40"
                >
                  <ChevronRight className="h-3.5 w-3.5" />
                </button>
              </div>
            )}
          </div>

          <div className="relative inline-block overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-white shadow-sm">
            {pagePreview ? (
              <img
                ref={previewImgRef}
                src={pagePreview.dataUrl}
                alt={`PDF page ${previewPageNum}`}
                className="block max-h-[480px] w-auto"
              />
            ) : (
              <div className="grid h-[420px] w-[300px] place-items-center text-xs text-neutral-400">
                Upload a PDF to preview watermark grid
              </div>
            )}

            {previewSlots.map((slot, idx) => (
              <div
                key={`${slot.leftPct}-${slot.topPct}-${idx}`}
                className="pointer-events-none absolute select-none"
                style={{
                  left: `${slot.leftPct}%`,
                  top: `${slot.topPct}%`,
                  opacity,
                  transform: `translate(-50%, -50%) rotate(${-rotation}deg)`,
                }}
              >
                {mode === "image" && imageDataUrl ? (
                  <img
                    src={imageDataUrl}
                    alt=""
                    style={{
                      width: `${Math.max(18, imageWidthPt * pxPerPt)}px`,
                      maxWidth: "none",
                      height: "auto",
                    }}
                  />
                ) : mode === "text" && text.trim() ? (
                  <span
                    className="whitespace-nowrap font-bold"
                    style={{
                      color,
                      fontSize: `${Math.max(8, fontSize * pxPerPt)}px`,
                      lineHeight: 1,
                    }}
                  >
                    {text}
                  </span>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      </div>
    </ToolShell>
  );
}
