import { useMemo, useRef, useState } from "react";
import katex from "katex";
import { toJpeg, toPng } from "html-to-image";
import { Check, Copy, Download, Sigma } from "lucide-react";
import { ToolShell } from "~/components/ToolShell";

const EXAMPLES = [
  { label: "Quadratic", latex: "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}" },
  { label: "Gaussian", latex: "\\int_{-\\infty}^{\\infty} e^{-x^2}\\,dx = \\sqrt{\\pi}" },
  { label: "Matrix", latex: "A = \\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}" },
  { label: "Basel Sum", latex: "\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}" },
  { label: "Bayes", latex: "P(A \\mid B) = \\frac{P(B \\mid A)\\,P(A)}{P(B)}" },
  { label: "Derivative", latex: "f'(x) = \\lim_{h \\to 0} \\frac{f(x+h) - f(x)}{h}" },
  { label: "Schrödinger", latex: "i\\hbar \\frac{\\partial}{\\partial t}\\Psi = \\hat{H}\\Psi" },
  { label: "Chemistry", latex: "\\mathrm{6CO_2 + 6H_2O \\rightarrow C_6H_{12}O_6 + 6O_2}" },
];

const PALETTE = [
  {
    group: "Core",
    items: [
      { label: "a/b", insert: "\\frac{a}{b}" },
      { label: "√x", insert: "\\sqrt{x}" },
      { label: "xⁿ", insert: "x^{n}" },
      { label: "xᵢ", insert: "x_{i}" },
      { label: "Σ", insert: "\\sum_{i=1}^{n}" },
      { label: "∫", insert: "\\int_{a}^{b}" },
      { label: "lim", insert: "\\lim_{x \\to 0}" },
      { label: "matrix", insert: "\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}" },
    ],
  },
  {
    group: "Greek",
    items: [
      { label: "α", insert: "\\alpha" },
      { label: "β", insert: "\\beta" },
      { label: "γ", insert: "\\gamma" },
      { label: "Δ", insert: "\\Delta" },
      { label: "θ", insert: "\\theta" },
      { label: "λ", insert: "\\lambda" },
      { label: "π", insert: "\\pi" },
      { label: "Ω", insert: "\\Omega" },
    ],
  },
  {
    group: "Relations",
    items: [
      { label: "≤", insert: "\\le" },
      { label: "≥", insert: "\\ge" },
      { label: "≠", insert: "\\neq" },
      { label: "≈", insert: "\\approx" },
      { label: "→", insert: "\\to" },
      { label: "⇒", insert: "\\Rightarrow" },
      { label: "∞", insert: "\\infty" },
      { label: "±", insert: "\\pm" },
    ],
  },
];

export default function EquationToImage() {
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [latex, setLatex] = useState("x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}");
  const [displayMode, setDisplayMode] = useState(true);
  const [transparent, setTransparent] = useState(true);
  const [format, setFormat] = useState<"png" | "jpg">("png");
  const [scale, setScale] = useState(3);
  const [color, setColor] = useState("#111318");
  const [copied, setCopied] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<{ url: string; width: number; height: number } | null>(
    null,
  );

  const { html, parseError } = useMemo(() => {
    if (!latex.trim()) return { html: "", parseError: null };
    try {
      const rendered = katex.renderToString(latex, {
        throwOnError: true,
        displayMode,
        strict: false,
      });
      return { html: rendered, parseError: null };
    } catch (e) {
      return {
        html: katex.renderToString(latex, { throwOnError: false, displayMode, strict: false }),
        parseError: e instanceof Error ? e.message.replace(/^KaTeX parse error:\s*/, "") : "Invalid LaTeX",
      };
    }
  }, [latex, displayMode]);

  const insertSnippet = (snippet: string) => {
    const el = inputRef.current;
    if (!el) {
      setLatex((prev) => `${prev}${snippet}`);
      return;
    }
    const start = el.selectionStart ?? latex.length;
    const end = el.selectionEnd ?? latex.length;
    const next = latex.slice(0, start) + snippet + latex.slice(end);
    setLatex(next);
    setExported(null);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(start + snippet.length, start + snippet.length);
    });
  };

  const handleExport = async () => {
    if (!latex.trim() || parseError) return;
    setExporting(true);
    const host = document.createElement("div");
    host.style.position = "fixed";
    host.style.left = "0";
    host.style.top = "0";
    host.style.zIndex = "-1";
    host.style.pointerEvents = "none";
    host.style.display = "inline-block";
    host.style.padding = "18px 22px";
    host.style.fontSize = "44px";
    host.style.color = color;
    host.style.background = transparent && format === "png" ? "transparent" : "#ffffff";
    host.innerHTML = html;
    document.body.append(host);
    try {
      if (document.fonts?.ready) await document.fonts.ready;
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      const rect = host.getBoundingClientRect();
      const opts = {
        pixelRatio: scale,
        cacheBust: true,
        backgroundColor: transparent && format === "png" ? undefined : "#ffffff",
        style: { position: "static", left: "0", top: "0", zIndex: "auto" },
      };
      const dataUrl = format === "jpg" ? await toJpeg(host, { ...opts, quality: 0.95 }) : await toPng(host, opts);
      setExported({
        url: dataUrl,
        width: Math.max(1, Math.ceil(rect.width * scale)),
        height: Math.max(1, Math.ceil(rect.height * scale)),
      });
    } finally {
      host.remove();
      setExporting(false);
    }
  };

  const downloadExport = () => {
    if (!exported) return;
    const link = document.createElement("a");
    link.href = exported.url;
    link.download = `520pdf-equation.${format}`;
    document.body.append(link);
    link.click();
    link.remove();
  };

  return (
    <ToolShell
      title="Equation to Image"
      description="Write LaTeX math with live KaTeX preview, symbol palette, and crisp cropped PNG/JPG export."
      category="Create"
    >
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="surface-card space-y-5 rounded-2xl p-6">
          <div>
            <p className="mb-2 text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
              Examples
            </p>
            <div className="flex flex-wrap gap-1.5">
              {EXAMPLES.map((ex) => (
                <button
                  key={ex.label}
                  type="button"
                  onClick={() => {
                    setLatex(ex.latex);
                    setExported(null);
                  }}
                  className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1 text-xs font-medium hover:border-[var(--accent)] hover:text-[var(--accent)]"
                >
                  {ex.label}
                </button>
              ))}
            </div>
          </div>

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <label htmlFor="eq-input" className="text-xs font-semibold text-[var(--ink-secondary)]">
                LaTeX expression
              </label>
              <button
                type="button"
                onClick={() => {
                  navigator.clipboard?.writeText(latex);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 1500);
                }}
                className="inline-flex items-center gap-1 text-xs font-semibold text-[var(--ink-muted)] hover:text-[var(--accent)]"
              >
                {copied ? <Check className="h-3.5 w-3.5 text-emerald-500" /> : <Copy className="h-3.5 w-3.5" />}
                {copied ? "Copied" : "Copy"}
              </button>
            </div>
            <textarea
              id="eq-input"
              ref={inputRef}
              rows={4}
              value={latex}
              onChange={(e) => {
                setLatex(e.target.value);
                setExported(null);
              }}
              className="w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3 font-mono text-sm"
            />
            {parseError && <p className="mt-1.5 text-xs text-red-500">{parseError}</p>}
          </div>

          <div className="space-y-2.5">
            {PALETTE.map((grp) => (
              <div key={grp.group} className="flex flex-wrap items-center gap-1.5">
                <span className="w-16 text-[11px] font-bold text-[var(--ink-muted)] uppercase">
                  {grp.group}
                </span>
                {grp.items.map((item) => (
                  <button
                    key={item.label}
                    type="button"
                    onClick={() => insertSnippet(item.insert)}
                    className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2 py-1 text-xs font-medium hover:border-[var(--accent)]"
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            ))}
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div>
              <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">Format</label>
              <select
                value={format}
                onChange={(e) => setFormat(e.target.value as "png" | "jpg")}
                className="mt-1 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-2 text-xs"
              >
                <option value="png">PNG</option>
                <option value="jpg">JPG</option>
              </select>
            </div>
            <div>
              <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">Scale</label>
              <select
                value={scale}
                onChange={(e) => setScale(Number(e.target.value))}
                className="mt-1 w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-2 text-xs"
              >
                <option value={2}>2× HD</option>
                <option value={3}>3× Crisp</option>
                <option value={4}>4× Print</option>
              </select>
            </div>
            <div>
              <label className="block text-[11px] font-semibold text-[var(--ink-muted)]">Color</label>
              <input
                type="color"
                value={color}
                onChange={(e) => setColor(e.target.value)}
                className="mt-1 h-9 w-full cursor-pointer rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] p-1"
              />
            </div>
            <div className="flex flex-col justify-end gap-1.5 text-xs">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={displayMode}
                  onChange={(e) => setDisplayMode(e.target.checked)}
                  className="accent-[var(--accent)]"
                />
                Display mode
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={transparent}
                  onChange={(e) => setTransparent(e.target.checked)}
                  disabled={format === "jpg"}
                  className="accent-[var(--accent)]"
                />
                Transparent
              </label>
            </div>
          </div>

          <button
            type="button"
            disabled={exporting || !latex.trim() || Boolean(parseError)}
            onClick={handleExport}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            <Sigma className="h-4 w-4" />
            {exporting ? "Rendering image…" : `Render ${format.toUpperCase()} Image`}
          </button>
        </div>

        <div className="surface-card flex flex-col justify-between space-y-5 rounded-2xl p-6">
          <div>
            <p className="mb-3 text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
              Live KaTeX Preview
            </p>
            <div
              className="grid min-h-[240px] place-items-center overflow-x-auto rounded-2xl border border-[var(--border-subtle)] bg-white p-8 text-3xl"
              style={{ color }}
              dangerouslySetInnerHTML={{ __html: html || '<span class="text-sm text-neutral-400">Type LaTeX to preview</span>' }}
            />
          </div>

          {exported && (
            <div className="space-y-3 rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.06] p-4">
              <div className="flex items-center justify-between">
                <p className="text-xs font-bold text-[var(--ink-primary)] tabular-nums">
                  Exported {exported.width} × {exported.height} px ({format.toUpperCase()})
                </p>
                <button
                  type="button"
                  onClick={downloadExport}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-3.5 py-1.5 text-xs font-semibold text-white hover:bg-[var(--accent-hover)]"
                >
                  <Download className="h-3.5 w-3.5" /> Download
                </button>
              </div>
              <div className="grid place-items-center rounded-xl bg-white p-4">
                <img src={exported.url} alt="Rendered equation" className="max-h-40 w-auto object-contain" />
              </div>
            </div>
          )}
        </div>
      </div>
    </ToolShell>
  );
}
