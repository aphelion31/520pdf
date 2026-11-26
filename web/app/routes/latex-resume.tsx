import { useMemo, useState } from "react";
import { Code2, Copy, Download, Loader2 } from "lucide-react";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { runTool, type ToolResponse } from "~/lib/api";

const TEMPLATES: Record<string, { label: string; code: string }> = {
  classic: {
    label: "Classic SWE",
    code: `\\documentclass[11pt,a4paper]{article}
\\usepackage[margin=1in]{geometry}
\\begin{document}

\\begin{center}
{\\LARGE \\textbf{Alex Rivera}}\\\\[4pt]
alex.rivera@example.com $\\cdot$ +1 (555) 234-5678 $\\cdot$ San Francisco, CA\\\\
\\href{https://github.com/alexrivera}{github.com/alexrivera}
\\end{center}

\\hrule
\\section*{Summary}
Systems engineer with 6+ years building high-throughput Rust backends and React interfaces. Focused on reliability, latency, and clean tooling.

\\hrule
\\section*{Experience}
\\textbf{Staff Backend Engineer, Orbit Systems} \\hfill Jan 2023 -- Present\\\\
\\begin{itemize}
  \\item Ported multi-service Rails/Node pipeline to single-binary Rust Axum, cutting RAM by 94\\%
  \\item Reduced p99 API latency from 240ms to 6ms under 32 concurrent workers
  \\item Led release automation and zero-downtime migration tooling
\\end{itemize}

\\textbf{Software Engineer, MetricCloud} \\hfill Jun 2019 -- Dec 2022\\\\
\\begin{itemize}
  \\item Built real-time document processing workers handling 2M PDFs/month
  \\item Designed TypeScript + React Router workbench with live canvas previews
\\end{itemize}

\\hrule
\\section*{Education}
\\textbf{B.S. Computer Science} \\hfill 2015 -- 2019\\\\
\\textit{University of California} $\\cdot$ GPA: 3.9/4.0

\\hrule
\\section*{Skills}
\\textbf{Languages:} Rust, TypeScript, SQL, Python\\\\
\\textbf{Infrastructure:} Axum, PostgreSQL, Docker, Linux, React Router

\\end{document}`,
  },
  minimal: {
    label: "Minimal",
    code: `\\documentclass[10pt,a4paper]{article}
\\begin{document}

{\\huge \\textbf{Samira Chen}}\\\\[2pt]
{\\small samira@example.com $\\cdot$ Singapore $\\cdot$ samirachen.dev}

\\hrule
\\section*{Experience}
\\textbf{Product Engineer, Atelier Labs} \\hfill 2022 -- Present
\\begin{itemize}
  \\item Shipped browser-native PDF annotation & signature tools with zero external runtime
  \\item Cut bundle footprint by 78\\% after migrating to Tailwind v4 and Vite 8
\\end{itemize}

\\textbf{Frontend Developer, Northstar} \\hfill 2020 -- 2022
\\begin{itemize}
  \\item Built accessible design system components across 14 product surfaces
\\end{itemize}

\\hrule
\\section*{Education}
\\textbf{B.Eng. Software Engineering, NUS} \\hfill 2016 -- 2020

\\hrule
\\section*{Technical Skills}
\\textbf{Core:} TypeScript, React, Rust, WebAssembly, CSS

\\end{document}`,
  },
  academic: {
    label: "Academic CV",
    code: `\\documentclass[11pt,a4paper]{article}
\\begin{document}

\\begin{center}
{\\Large \\textbf{Dr. Nadia Kusuma}}\\\\[4pt]
Department of Computer Science $\\cdot$ Institute of Technology\\\\
nadia.kusuma@university.edu $\\cdot$ scholar.example.edu/nkusuma
\\end{center}

\\hrule
\\section*{Research Interests}
Document layout synthesis, formal verification of memory-safe systems, and low-footprint runtimes.

\\hrule
\\section*{Education}
\\textbf{Ph.D. Computer Science} \\hfill 2019 -- 2024\\\\
\\textit{Top Research University}\\\\
Thesis: Deterministic Compilation of Document Layout Primitives

\\hrule
\\section*{Selected Publications}
\\begin{itemize}
  \\item \\textbf{N. Kusuma}, A. Patel. Zero-Copy PDF Stream Transformations. \\textit{SysConf}, 2024.
  \\item \\textbf{N. Kusuma}, L. Tan. Memory Bounds in Web Tooling. \\textit{DocEng}, 2022.
\\end{itemize}

\\hrule
\\section*{Honors}
\\begin{itemize}
  \\item Best Systems Paper Award, 2024
  \\item Doctoral Research Fellowship, 2020 -- 2023
\\end{itemize}

\\end{document}`,
  },
};

function cleanPreviewInline(s: string): string {
  return s
    .replace(/\\href\{[^}]*\}\{([^}]*)\}/g, "$1")
    .replace(/\\textbf\{([^}]*)\}/g, "$1")
    .replace(/\\textit\{([^}]*)\}/g, "$1")
    .replace(/\\emph\{([^}]*)\}/g, "$1")
    .replace(/\\LARGE|\\Large|\\huge|\\Huge|\\small/g, "")
    .replace(/\$\\cdot\$/g, "·")
    .replace(/\\cdot/g, "·")
    .replace(/\\%/g, "%")
    .replace(/\\&/g, "&")
    .replace(/\\\\(\[\d+pt\])?/g, " ")
    .replace(/---/g, "—")
    .replace(/--/g, "–")
    .replace(/[{}]/g, "")
    .trim();
}

export default function LatexResume() {
  const [templateKey, setTemplateKey] = useState<string>("classic");
  const [code, setCode] = useState(TEMPLATES.classic.code);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const previewBlocks = useMemo(() => {
    const start = code.indexOf("\\begin{document}");
    const end = code.indexOf("\\end{document}");
    const slice = code.slice(
      start !== -1 ? start + 16 : 0,
      end !== -1 ? end : code.length,
    );
    return slice.split("\n").map((line) => line.trim()).filter(Boolean);
  }, [code]);

  const handleExportPdf = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await runTool("latex-resume", [], { latex: code });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "LaTeX PDF export failed");
    } finally {
      setBusy(false);
    }
  };

  const downloadTex = () => {
    const blob = new Blob([code], { type: "text/x-tex" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `520pdf-resume-${templateKey}.tex`;
    document.body.append(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  };

  return (
    <ToolShell
      title="LaTeX Resume"
      description="Write or customize your LaTeX resume with 3 templates and compile directly to A4 PDF in pure Rust (pdf-writer)."
      category="Create"
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap gap-2">
          {Object.entries(TEMPLATES).map(([key, tpl]) => (
            <button
              key={key}
              type="button"
              onClick={() => {
                setTemplateKey(key);
                setCode(tpl.code);
                setResult(null);
              }}
              className={`rounded-lg px-3 py-1.5 text-xs font-semibold transition-colors ${
                templateKey === key
                  ? "bg-[var(--ink-primary)] text-[var(--bg-surface)]"
                  : "border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--ink-secondary)]"
              }`}
            >
              {tpl.label}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => navigator.clipboard?.writeText(code)}
            className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-xs font-semibold"
          >
            <Copy className="h-3.5 w-3.5" /> Copy .tex
          </button>
          <button
            type="button"
            onClick={downloadTex}
            className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-xs font-semibold"
          >
            <Download className="h-3.5 w-3.5" /> Save .tex
          </button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <div className="surface-card flex flex-col space-y-4 rounded-2xl p-5">
          <div className="flex items-center justify-between">
            <span className="inline-flex items-center gap-1.5 text-xs font-bold text-[var(--ink-muted)] uppercase">
              <Code2 className="h-3.5 w-3.5" /> LaTeX Source
            </span>
            <span className="text-xs text-[var(--ink-muted)] tabular-nums">{code.length} chars</span>
          </div>
          <textarea
            value={code}
            onChange={(e) => setCode(e.target.value)}
            rows={22}
            spellCheck={false}
            className="w-full flex-1 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3.5 font-mono text-xs leading-relaxed"
          />

          {error && (
            <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600">
              {error}
            </p>
          )}

          <button
            type="button"
            disabled={busy || !code.trim()}
            onClick={handleExportPdf}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            {busy ? "Rendering PDF in Rust…" : "Export A4 PDF"}
          </button>

          {result && <ResultBanner blob={result.blob} filename={result.filename} />}
        </div>

        <div className="surface-card rounded-2xl p-5">
          <p className="mb-3 text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
            Live Document Preview
          </p>
          <div className="min-h-[540px] space-y-1.5 rounded-xl border border-[var(--border-subtle)] bg-white p-8 text-xs leading-relaxed text-neutral-800 shadow-inner">
            {previewBlocks.map((line, i) => {
              if (line.startsWith("%") || line.startsWith("\\documentclass") || line.startsWith("\\usepackage")) {
                return null;
              }
              if (line === "\\hrule" || line === "\\hline") {
                return <hr key={i} className="my-2 border-neutral-300" />;
              }
              if (line.startsWith("\\section")) {
                const title = cleanPreviewInline(line.replace(/^\\section\*?\{/, "").replace(/\}$/, ""));
                return (
                  <h3 key={i} className="pt-2 text-[11px] font-bold tracking-wider text-neutral-900 uppercase">
                    {title}
                  </h3>
                );
              }
              if (line.startsWith("\\item")) {
                return (
                  <p key={i} className="pl-4 text-neutral-700">
                    • {cleanPreviewInline(line.replace(/^\\item\s*/, ""))}
                  </p>
                );
              }
              if (line.includes("\\hfill")) {
                const [l, r] = line.split("\\hfill");
                return (
                  <div key={i} className="flex items-baseline justify-between gap-3 pt-1">
                    <span className="font-semibold text-neutral-900">{cleanPreviewInline(l)}</span>
                    <span className="shrink-0 text-[11px] text-neutral-500">{cleanPreviewInline(r || "")}</span>
                  </div>
                );
              }
              if (line.includes("\\LARGE") || line.includes("\\huge") || line.includes("\\Large")) {
                return (
                  <p key={i} className="text-center text-lg font-extrabold text-neutral-950">
                    {cleanPreviewInline(line)}
                  </p>
                );
              }
              if (line.startsWith("\\begin{") || line.startsWith("\\end{")) return null;
              return <p key={i}>{cleanPreviewInline(line)}</p>;
            })}
          </div>
        </div>
      </div>
    </ToolShell>
  );
}
