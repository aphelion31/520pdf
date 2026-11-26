import { useEffect, useState } from "react";
import { Loader2, Scissors } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";
import { renderPdfPages, type PdfPageRender } from "~/lib/pdfjs";

export default function SplitPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [previews, setPreviews] = useState<PdfPageRender[]>([]);
  const [totalPages, setTotalPages] = useState(0);
  const [selected, setSelected] = useState<number[]>([]);
  const [mode, setMode] = useState<"extract" | "all" | "ranges">("extract");
  const [pagesExpr, setPagesExpr] = useState("1");
  const [rangesExpr, setRangesExpr] = useState("1-2;3");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  useEffect(() => {
    const file = files[0];
    if (!file) {
      setPreviews([]);
      setTotalPages(0);
      return;
    }
    let cancelled = false;
    renderPdfPages(file, 24, 180)
      .then(({ totalPages: total, pages }) => {
        if (cancelled) return;
        setTotalPages(total);
        setPreviews(pages);
        const all = pages.map((p) => p.pageNumber);
        setSelected(all);
        setPagesExpr(all.join(","));
      })
      .catch(() => {
        if (!cancelled) {
          setPreviews([]);
          setTotalPages(0);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [files]);

  const togglePage = (num: number) => {
    const next = selected.includes(num)
      ? selected.filter((p) => p !== num)
      : [...selected, num].sort((a, b) => a - b);
    setSelected(next);
    setPagesExpr(next.join(","));
  };

  const handleSplit = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const options: Record<string, string> = { mode };
      if (mode === "extract") options.pages = pagesExpr;
      if (mode === "ranges") options.ranges = rangesExpr;
      const res = await runTool("split", files, options);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Split failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Split PDF"
      description="Extract specific page ranges or burst every page into individual PDFs inside a ZIP."
      category="Organize"
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

        {files[0] && (
          <>
            <div className="grid gap-3 sm:grid-cols-3">
              {[
                { id: "extract" as const, title: "Extract selected", sub: "Single PDF with chosen pages" },
                { id: "all" as const, title: "Split all pages", sub: "ZIP archive, one PDF per page" },
                { id: "ranges" as const, title: "Custom groups", sub: "ZIP with semicolon ranges" },
              ].map((m) => (
                <button
                  key={m.id}
                  type="button"
                  onClick={() => setMode(m.id)}
                  className={`rounded-xl border p-3.5 text-left transition-colors ${
                    mode === m.id
                      ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                      : "border-[var(--border-subtle)] bg-[var(--bg-page)] hover:border-[var(--accent)]/40"
                  }`}
                >
                  <p className="text-sm font-bold text-[var(--ink-primary)]">{m.title}</p>
                  <p className="mt-0.5 text-xs text-[var(--ink-muted)]">{m.sub}</p>
                </button>
              ))}
            </div>

            {mode === "extract" && (
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Pages ({totalPages || "?"} total)
                </label>
                <input
                  type="text"
                  value={pagesExpr}
                  onChange={(e) => setPagesExpr(e.target.value)}
                  placeholder="1,3-5"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                />
              </div>
            )}

            {mode === "ranges" && (
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Output groups (semicolon-separated)
                </label>
                <input
                  type="text"
                  value={rangesExpr}
                  onChange={(e) => setRangesExpr(e.target.value)}
                  placeholder="1-2;3-4"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                />
              </div>
            )}

            {previews.length > 0 && mode === "extract" && (
              <div>
                <p className="mb-2 text-xs font-semibold text-[var(--ink-muted)] uppercase">
                  Click pages to toggle selection
                </p>
                <div className="grid grid-cols-3 gap-3 sm:grid-cols-6">
                  {previews.map((p) => {
                    const active = selected.includes(p.pageNumber);
                    return (
                      <button
                        key={p.pageNumber}
                        type="button"
                        onClick={() => togglePage(p.pageNumber)}
                        className={`overflow-hidden rounded-xl border p-1.5 text-center transition-all ${
                          active
                            ? "border-[var(--accent)] ring-2 ring-[var(--accent)]/25"
                            : "border-[var(--border-subtle)] opacity-55"
                        }`}
                      >
                        <img
                          src={p.dataUrl}
                          alt={`Page ${p.pageNumber}`}
                          className="mx-auto h-28 w-full rounded object-contain bg-white"
                        />
                        <span className="mt-1 block text-xs font-semibold text-[var(--ink-primary)]">
                          Page {p.pageNumber}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
          </>
        )}

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || !files[0]}
          onClick={handleSplit}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Scissors className="h-4 w-4" />}
          {busy ? "Splitting PDF…" : "Split PDF"}
        </button>

        {result && (
          <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
        )}
      </div>
    </ToolShell>
  );
}
