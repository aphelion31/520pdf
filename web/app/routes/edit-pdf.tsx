import React, { useEffect, useRef, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Circle,
  Highlighter,
  ImagePlus,
  Loader2,
  Minus,
  Pencil,
  Save,
  Square,
  Trash2,
  Type,
  Undo2,
} from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderSinglePage, type PdfPageRender } from "~/lib/pdfjs";

type EditToolMode = "text" | "highlight" | "rect" | "ellipse" | "line" | "path";

type EditAnnotation =
  | {
      id: string;
      type: "text";
      page: number;
      x: number;
      y: number;
      text: string;
      font_size: number;
      color: string;
      opacity: number;
    }
  | {
      id: string;
      type: "highlight" | "rect" | "ellipse";
      page: number;
      x: number;
      y: number;
      width: number;
      height: number;
      color: string;
      stroke_width: number;
      opacity: number;
    }
  | {
      id: string;
      type: "line";
      page: number;
      x1: number;
      y1: number;
      x2: number;
      y2: number;
      color: string;
      stroke_width: number;
      opacity: number;
    }
  | {
      id: string;
      type: "path";
      page: number;
      points: [number, number][];
      color: string;
      stroke_width: number;
      opacity: number;
    }
  | {
      id: string;
      type: "image";
      page: number;
      x: number;
      y: number;
      width: number;
      height: number;
      image_base64: string;
      opacity: number;
    };

const COLORS = [
  { hex: "#111318", name: "Ink" },
  { hex: "#e63228", name: "Crimson" },
  { hex: "#2563eb", name: "Royal Blue" },
  { hex: "#16a34a", name: "Emerald" },
  { hex: "#facc15", name: "Highlighter" },
  { hex: "#7c3aed", name: "Violet" },
];

export default function EditPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [pageNum, setPageNum] = useState(1);
  const [pagePreview, setPagePreview] = useState<PdfPageRender | null>(null);

  const [mode, setMode] = useState<EditToolMode>("text");
  const [textDraft, setTextDraft] = useState("Approved");
  const [fontSize, setFontSize] = useState(18);
  const [color, setColor] = useState("#e63228");
  const [strokeWidth, setStrokeWidth] = useState(2.5);
  const [opacity, setOpacity] = useState(1);

  const [items, setItems] = useState<EditAnnotation[]>([]);
  const [draftShape, setDraftShape] = useState<EditAnnotation | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const imgInputRef = useRef<HTMLInputElement>(null);
  const drawStartRef = useRef<[number, number] | null>(null);

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
    renderSinglePage(f, pageNum, 560)
      .then((p) => {
        if (!cancelled) setPagePreview(p);
      })
      .catch(() => {
        if (!cancelled) setPagePreview(null);
      });
    return () => {
      cancelled = true;
    };
  }, [files, pageNum]);

  const pageW = pagePreview?.width || 595;
  const pageH = pagePreview?.height || 842;
  const totalPages = pagePreview?.totalPages || 1;

  const selectMode = (next: EditToolMode) => {
    setMode(next);
    if (next === "highlight") {
      setColor("#facc15");
      setOpacity(0.38);
    } else if (opacity < 0.5) {
      setOpacity(1);
    }
  };

  const clientToPdf = (e: React.PointerEvent<SVGSVGElement>): [number, number] => {
    const svg = svgRef.current!;
    const rect = svg.getBoundingClientRect();
    const nx = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const nyTop = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    return [
      Math.round(nx * pageW * 10) / 10,
      Math.round((1 - nyTop) * pageH * 10) / 10,
    ];
  };

  const makeId = () => `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

  const onPointerDown = (e: React.PointerEvent<SVGSVGElement>) => {
    if (!files[0]) return;
    const [px, py] = clientToPdf(e);
    if (mode === "text") {
      const label = textDraft.trim();
      if (!label) {
        setError("Enter text before clicking the page.");
        return;
      }
      setError(null);
      setItems((prev) => [
        ...prev,
        {
          id: makeId(),
          type: "text",
          page: pageNum,
          x: px,
          y: py,
          text: label,
          font_size: fontSize,
          color,
          opacity,
        },
      ]);
      return;
    }

    e.currentTarget.setPointerCapture(e.pointerId);
    drawStartRef.current = [px, py];
    if (mode === "path") {
      setDraftShape({
        id: "draft",
        type: "path",
        page: pageNum,
        points: [[px, py]],
        color,
        stroke_width: strokeWidth,
        opacity,
      });
    } else if (mode === "line") {
      setDraftShape({
        id: "draft",
        type: "line",
        page: pageNum,
        x1: px,
        y1: py,
        x2: px,
        y2: py,
        color,
        stroke_width: strokeWidth,
        opacity,
      });
    } else {
      setDraftShape({
        id: "draft",
        type: mode,
        page: pageNum,
        x: px,
        y: py,
        width: 1,
        height: 1,
        color,
        stroke_width: strokeWidth,
        opacity,
      });
    }
  };

  const onPointerMove = (e: React.PointerEvent<SVGSVGElement>) => {
    const start = drawStartRef.current;
    if (!start || !draftShape) return;
    const [px, py] = clientToPdf(e);
    if (draftShape.type === "path") {
      const last = draftShape.points[draftShape.points.length - 1];
      if (!last || Math.hypot(px - last[0], py - last[1]) >= 2.5) {
        const pt: [number, number] = [px, py];
        setDraftShape({
          ...draftShape,
          points: [...draftShape.points, pt].slice(0, 1000),
        });
      }
    } else if (draftShape.type === "line") {
      setDraftShape({ ...draftShape, x2: px, y2: py });
    } else if (
      draftShape.type === "highlight" ||
      draftShape.type === "rect" ||
      draftShape.type === "ellipse"
    ) {
      setDraftShape({
        ...draftShape,
        x: Math.min(start[0], px),
        y: Math.min(start[1], py),
        width: Math.max(4, Math.abs(px - start[0])),
        height: Math.max(4, Math.abs(py - start[1])),
      });
    }
  };

  const onPointerUp = () => {
    if (!drawStartRef.current || !draftShape) {
      drawStartRef.current = null;
      return;
    }
    drawStartRef.current = null;
    const finalized: EditAnnotation | null =
      draftShape.type === "path" && draftShape.points.length < 2
        ? null
        : draftShape.type === "line" &&
            Math.hypot(draftShape.x2 - draftShape.x1, draftShape.y2 - draftShape.y1) < 3
          ? null
          : { ...draftShape, id: makeId() };
    if (finalized) {
      setItems((prev) => [...prev, finalized]);
    }
    setDraftShape(null);
  };

  const handleImagePick = (file?: File) => {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = typeof reader.result === "string" ? reader.result : "";
      if (!dataUrl) return;
      const img = new window.Image();
      img.onload = () => {
        const w = Math.min(200, Math.round(pageW * 0.34));
        const h = Math.max(24, Math.round((w * img.height) / Math.max(1, img.width)));
        setItems((prev) => [
          ...prev,
          {
            id: makeId(),
            type: "image",
            page: pageNum,
            x: Math.round((pageW - w) / 2),
            y: Math.round((pageH - h) / 2),
            width: w,
            height: h,
            image_base64: dataUrl,
            opacity,
          },
        ]);
      };
      img.src = dataUrl;
    };
    reader.readAsDataURL(file);
  };

  const handleSave = async () => {
    if (!files[0]) return;
    if (!items.length) {
      setError("Add at least one text, highlight, shape, freehand stroke, or image on the page.");
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const payload = items.map(({ id: _id, ...rest }) =>
        rest.type === "image"
          ? { ...rest, image_base64: rest.image_base64.split(",")[1] || rest.image_base64 }
          : rest,
      );
      const res = await runTool("edit-pdf", files, {
        annotations: JSON.stringify(payload),
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Edit failed");
    } finally {
      setBusy(false);
    }
  };

  const pageItems = [
    ...items.filter((i) => i.page === pageNum),
    ...(draftShape && draftShape.page === pageNum ? [draftShape] : []),
  ];

  const renderSvgItem = (item: EditAnnotation) => {
    if (item.type === "text") {
      return (
        <text
          key={item.id}
          x={item.x}
          y={pageH - item.y}
          fill={item.color}
          fillOpacity={item.opacity}
          fontSize={item.font_size}
          fontFamily="Helvetica, Arial, sans-serif"
        >
          {item.text}
        </text>
      );
    }
    if (item.type === "highlight") {
      return (
        <rect
          key={item.id}
          x={item.x}
          y={pageH - item.y - item.height}
          width={item.width}
          height={item.height}
          fill={item.color}
          fillOpacity={item.opacity}
        />
      );
    }
    if (item.type === "rect") {
      return (
        <rect
          key={item.id}
          x={item.x}
          y={pageH - item.y - item.height}
          width={item.width}
          height={item.height}
          fill="none"
          stroke={item.color}
          strokeOpacity={item.opacity}
          strokeWidth={item.stroke_width}
        />
      );
    }
    if (item.type === "ellipse") {
      return (
        <ellipse
          key={item.id}
          cx={item.x + item.width / 2}
          cy={pageH - item.y - item.height / 2}
          rx={item.width / 2}
          ry={item.height / 2}
          fill="none"
          stroke={item.color}
          strokeOpacity={item.opacity}
          strokeWidth={item.stroke_width}
        />
      );
    }
    if (item.type === "line") {
      return (
        <line
          key={item.id}
          x1={item.x1}
          y1={pageH - item.y1}
          x2={item.x2}
          y2={pageH - item.y2}
          stroke={item.color}
          strokeOpacity={item.opacity}
          strokeWidth={item.stroke_width}
          strokeLinecap="round"
        />
      );
    }
    if (item.type === "path") {
      const pts = item.points.map(([x, y]) => `${x},${pageH - y}`).join(" ");
      return (
        <polyline
          key={item.id}
          points={pts}
          fill="none"
          stroke={item.color}
          strokeOpacity={item.opacity}
          strokeWidth={item.stroke_width}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      );
    }
    if (item.type === "image") {
      return (
        <image
          key={item.id}
          href={item.image_base64}
          x={item.x}
          y={pageH - item.y - item.height}
          width={item.width}
          height={item.height}
          opacity={item.opacity}
          preserveAspectRatio="none"
        />
      );
    }
    return null;
  };

  return (
    <ToolShell
      title="Edit PDF"
      description="Add text, highlights, shapes, freehand annotations, and images directly onto your PDF pages."
      category="Edit"
    >
      <div className="grid gap-6 lg:grid-cols-[1.25fr_0.75fr]">
        <div className="surface-card space-y-4 rounded-2xl p-6">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap gap-1.5" role="toolbar" aria-label="PDF annotation tools">
              {(
                [
                  { id: "text", label: "Text", icon: Type },
                  { id: "highlight", label: "Highlight", icon: Highlighter },
                  { id: "rect", label: "Rectangle", icon: Square },
                  { id: "ellipse", label: "Circle", icon: Circle },
                  { id: "line", label: "Line", icon: Minus },
                  { id: "path", label: "Draw", icon: Pencil },
                ] as const
              ).map((tool) => {
                const Icon = tool.icon;
                const active = mode === tool.id;
                return (
                  <button
                    key={tool.id}
                    type="button"
                    aria-pressed={active}
                    onClick={() => selectMode(tool.id)}
                    className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold transition-colors ${
                      active
                        ? "bg-[var(--accent)] text-white"
                        : "border border-[var(--border-subtle)] bg-[var(--bg-page)] text-[var(--ink-secondary)] hover:border-[var(--accent)]"
                    }`}
                  >
                    <Icon className="h-3.5 w-3.5" />
                    {tool.label}
                  </button>
                );
              })}
              <button
                type="button"
                onClick={() => imgInputRef.current?.click()}
                className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs font-semibold text-[var(--ink-secondary)] hover:border-[var(--accent)]"
              >
                <ImagePlus className="h-3.5 w-3.5" />
                Image
              </button>
              <input
                ref={imgInputRef}
                type="file"
                accept="image/png,image/jpeg"
                className="hidden"
                onChange={(e) => {
                  handleImagePick(e.target.files?.[0]);
                  e.target.value = "";
                }}
              />
            </div>

            <div className="flex items-center gap-1.5">
              <button
                type="button"
                disabled={!items.length}
                onClick={() => setItems((prev) => prev.slice(0, -1))}
                className="inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] px-2.5 py-1.5 text-xs font-semibold disabled:opacity-40"
              >
                <Undo2 className="h-3.5 w-3.5" /> Undo
              </button>
              <button
                type="button"
                disabled={totalPages <= 1 || pageNum <= 1}
                onClick={() => setPageNum((p) => Math.max(1, p - 1))}
                aria-label="Previous page"
                className="rounded-lg border border-[var(--border-subtle)] p-1.5 disabled:opacity-40"
              >
                <ChevronLeft className="h-3.5 w-3.5" />
              </button>
              <span className="px-1 text-xs font-semibold tabular-nums text-[var(--ink-secondary)]">
                {pageNum}/{totalPages}
              </span>
              <button
                type="button"
                disabled={totalPages <= 1 || pageNum >= totalPages}
                onClick={() => setPageNum((p) => Math.min(totalPages, p + 1))}
                aria-label="Next page"
                className="rounded-lg border border-[var(--border-subtle)] p-1.5 disabled:opacity-40"
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>

          <div className="flex justify-center rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3">
            {pagePreview ? (
              <div className="relative inline-block overflow-hidden rounded-lg bg-white shadow-sm">
                <img
                  src={pagePreview.dataUrl}
                  alt={`PDF page ${pageNum}`}
                  draggable={false}
                  className="block max-h-[540px] w-auto select-none"
                />
                <svg
                  ref={svgRef}
                  viewBox={`0 0 ${pageW} ${pageH}`}
                  onPointerDown={onPointerDown}
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  onPointerCancel={onPointerUp}
                  className="absolute inset-0 h-full w-full cursor-crosshair touch-none"
                >
                  {pageItems.map(renderSvgItem)}
                </svg>
              </div>
            ) : (
              <div className="grid h-[420px] w-full place-items-center text-xs text-[var(--ink-muted)]">
                Upload a PDF on the right to begin editing interactively
              </div>
            )}
          </div>
          <p className="text-center text-[11px] text-[var(--ink-muted)]">
            {mode === "text"
              ? "Click anywhere on the page to stamp your text."
              : "Click and drag on the page canvas to draw your annotation."}
          </p>
        </div>

        <div className="surface-card space-y-5 rounded-2xl p-6">
          <FileDropzone
            accept=".pdf"
            files={files}
            onFilesChange={(next) => {
              setFiles(next);
              setPageNum(1);
              setItems([]);
              setResult(null);
            }}
          />

          {mode === "text" && (
            <div className="grid gap-3 sm:grid-cols-[1fr_110px]">
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Text content
                </label>
                <input
                  type="text"
                  value={textDraft}
                  onChange={(e) => setTextDraft(e.target.value)}
                  placeholder="Enter text to place…"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-2 text-sm"
                />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-semibold text-[var(--ink-secondary)]">
                  Font size
                </label>
                <BasecoatSelect
                  ariaLabel="Annotation font size"
                  value={String(fontSize)}
                  onChange={(v) => setFontSize(Number(v))}
                  options={[
                    { value: "12", label: "12 pt" },
                    { value: "16", label: "16 pt" },
                    { value: "18", label: "18 pt" },
                    { value: "24", label: "24 pt" },
                    { value: "36", label: "36 pt" },
                  ]}
                />
              </div>
            </div>
          )}

          <div>
            <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
              Color
            </label>
            <div className="mt-2 flex flex-wrap items-center gap-2">
              {COLORS.map((c) => (
                <button
                  key={c.hex}
                  type="button"
                  aria-label={`Color ${c.name}`}
                  onClick={() => setColor(c.hex)}
                  className={`h-7 w-7 rounded-full border-2 transition-transform ${
                    color.toLowerCase() === c.hex ? "scale-110 border-[var(--accent)]" : "border-transparent"
                  }`}
                  style={{ backgroundColor: c.hex }}
                />
              ))}
              <input
                type="color"
                aria-label="Custom annotation color"
                value={color}
                onChange={(e) => setColor(e.target.value)}
                className="h-7 w-9 cursor-pointer rounded border border-[var(--border-subtle)] bg-transparent p-0.5"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                Stroke ({strokeWidth} pt)
              </label>
              <input
                type="range"
                min={1}
                max={12}
                step={0.5}
                value={strokeWidth}
                onChange={(e) => setStrokeWidth(Number(e.target.value))}
                className="mt-2 w-full accent-[var(--accent)]"
              />
            </div>
            <div>
              <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                Opacity ({Math.round(opacity * 100)}%)
              </label>
              <input
                type="range"
                min={0.15}
                max={1}
                step={0.05}
                value={opacity}
                onChange={(e) => setOpacity(Number(e.target.value))}
                className="mt-2 w-full accent-[var(--accent)]"
              />
            </div>
          </div>

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <span className="text-xs font-semibold text-[var(--ink-secondary)]">
                Annotations ({items.length})
              </span>
              {items.length > 0 && (
                <button
                  type="button"
                  onClick={() => setItems([])}
                  className="text-xs font-semibold text-[var(--ink-muted)] hover:text-[var(--accent)]"
                >
                  Clear all
                </button>
              )}
            </div>
            <div className="max-h-36 space-y-1.5 overflow-y-auto pr-1">
              {items.map((item, idx) => (
                <div
                  key={item.id}
                  className="flex items-center justify-between gap-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs"
                >
                  <span className="truncate font-medium">
                    #{idx + 1} · P{item.page} ·{" "}
                    {item.type === "text" ? `"${item.text}"` : item.type}
                  </span>
                  <button
                    type="button"
                    aria-label={`Delete annotation ${idx + 1}`}
                    onClick={() => setItems((prev) => prev.filter((x) => x.id !== item.id))}
                    className="text-[var(--ink-muted)] hover:text-[var(--accent)]"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
              {items.length === 0 && (
                <p className="rounded-lg border border-dashed border-[var(--border-subtle)] px-3 py-2 text-xs text-[var(--ink-muted)]">
                  No edits placed yet.
                </p>
              )}
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
            onClick={handleSave}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            {busy ? "Saving Edited PDF…" : "Save Edited PDF"}
          </button>

          {result && (
            <ResultBanner
              blob={result.blob}
              filename={result.filename}
              originalBytes={files[0]?.size}
            />
          )}
        </div>
      </div>
    </ToolShell>
  );
}
