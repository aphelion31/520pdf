import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { Loader2, Play, RotateCw } from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { FileDropzone } from "~/components/FileDropzone";
import { HelpTip } from "~/components/HelpTip";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { fetchCatalog, runTool, type ApiTool, type ToolResponse } from "~/lib/api";
import { TOOLS_CATALOG } from "~/lib/catalog";
import { renderPdfPages, type PdfPageRender } from "~/lib/pdfjs";

export default function ToolWorkbench() {
  const { toolId = "" } = useParams();
  const meta = TOOLS_CATALOG.find((t) => t.id === toolId);
  const { match } = useFileVault();

  const [apiTool, setApiTool] = useState<ApiTool | null>(null);
  const [files, setFiles] = useState<File[]>(() => {
    const matched = match(meta?.accept || ".pdf");
    if (!matched.length) return [];
    return meta?.multiple ? matched.map((m) => m.file) : [matched[0].file];
  });
  const [values, setValues] = useState<Record<string, string | boolean>>({});
  const [pages, setPages] = useState<Array<PdfPageRender & { selected: boolean; rot: number }>>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  useEffect(() => {
    setResult(null);
    setError(null);
    fetchCatalog()
      .then((list) => {
        const found = list.find((t) => t.id === toolId) || null;
        setApiTool(found);
        const init: Record<string, string | boolean> = {};
        for (const f of found?.fields ?? []) {
          init[f.name] = f.type === "checkbox" ? f.value === true || f.value === "true" : String(f.value ?? "");
        }
        setValues(init);
      })
      .catch(() => setApiTool(null));
  }, [toolId]);

  useEffect(() => {
    const first = files[0];
    if (!first || !(first.type === "application/pdf" || /\.pdf$/i.test(first.name))) {
      setPages([]);
      return;
    }
    let cancelled = false;
    renderPdfPages(first, 24, 170)
      .then(({ pages: rendered }) => {
        if (!cancelled) {
          setPages(rendered.map((p) => ({ ...p, selected: true, rot: 0 })));
        }
      })
      .catch(() => {
        if (!cancelled) setPages([]);
      });
    return () => {
      cancelled = true;
    };
  }, [files]);

  if (!meta && !apiTool) {
    return (
      <ToolShell title="Tool not found" description="Unknown tool route." category="Error">
        <p className="text-sm text-[var(--ink-secondary)]">Check the catalog to pick an available tool.</p>
      </ToolShell>
    );
  }

  const title = apiTool?.name || meta?.name || toolId;
  const description = apiTool?.description || meta?.description || "";
  const category = apiTool?.category || meta?.category || "Tool";
  const accept = apiTool?.accept || meta?.accept || ".pdf";
  const multiple = apiTool?.multiple ?? meta?.multiple ?? false;

  const hasInlineHtml =
    toolId === "html-to-pdf" && String(values.html ?? "").trim().length > 0;

  const handleRun = async () => {
    if (!files.length && !hasInlineHtml) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const payload: Record<string, string | boolean> = { ...values };
      if (toolId === "organize" && pages.length && files.length === 1) {
        payload.pages = pages.filter((p) => p.selected).map((p) => p.pageNumber).join(",");
      }
      if (toolId === "rotate" && files.length === 1 && pages.length && pages.some((p) => p.rot !== 0)) {
        payload.rotations = JSON.stringify(
          Object.fromEntries(pages.map((p) => [p.pageNumber, p.rot])),
        );
      }
      const res = await runTool(toolId, files, payload);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Processing failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell title={title} description={description} category={category}>
      <div className="surface-card space-y-6 rounded-2xl p-6">
        <FileDropzone
          accept={accept}
          multiple={multiple}
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
          }}
        />

        {toolId === "rotate" && files.length > 1 && (
          <p className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-3 text-xs font-medium text-[var(--ink-secondary)]">
            Batch mode: all {files.length} PDFs will be rotated by {String(values.angle || "90")}° and exported together as a ZIP archive.
          </p>
        )}

        {pages.length > 0 && files.length === 1 && (toolId === "organize" || toolId === "rotate") && (
          <div>
            <p className="mb-2 text-xs font-semibold text-[var(--ink-muted)] uppercase">
              Interactive Page Grid
            </p>
            <div className="grid grid-cols-3 gap-3 sm:grid-cols-6">
              {pages.map((p, idx) => (
                <div
                  key={`${p.pageNumber}-${idx}`}
                  className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-2 text-center"
                >
                  <img
                    src={p.dataUrl}
                    alt={`Page ${p.pageNumber}`}
                    style={{ transform: `rotate(${p.rot}deg)` }}
                    className="mx-auto h-28 w-full rounded bg-white object-contain transition-transform"
                  />
                  <p className="mt-1 text-xs font-semibold">Page {p.pageNumber}</p>
                  {toolId === "rotate" && (
                    <button
                      type="button"
                      onClick={() =>
                        setPages((prev) =>
                          prev.map((item, i) =>
                            i === idx ? { ...item, rot: (item.rot + 90) % 360 } : item,
                          ),
                        )
                      }
                      className="mt-1 inline-flex items-center gap-1 rounded border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2 py-0.5 text-[11px]"
                    >
                      <RotateCw className="h-3 w-3" /> {p.rot}°
                    </button>
                  )}
                  {toolId === "organize" && (
                    <div className="mt-1 flex items-center justify-center gap-1 text-[11px]">
                      <input
                        type="checkbox"
                        checked={p.selected}
                        onChange={(e) =>
                          setPages((prev) =>
                            prev.map((item, i) =>
                              i === idx ? { ...item, selected: e.target.checked } : item,
                            ),
                          )
                        }
                      />
                      <button
                        type="button"
                        disabled={idx === 0}
                        onClick={() =>
                          setPages((prev) => {
                            const copy = [...prev];
                            [copy[idx - 1], copy[idx]] = [copy[idx], copy[idx - 1]];
                            return copy;
                          })
                        }
                        className="px-1 disabled:opacity-30"
                      >
                        ←
                      </button>
                      <button
                        type="button"
                        disabled={idx === pages.length - 1}
                        onClick={() =>
                          setPages((prev) => {
                            const copy = [...prev];
                            [copy[idx + 1], copy[idx]] = [copy[idx], copy[idx + 1]];
                            return copy;
                          })
                        }
                        className="px-1 disabled:opacity-30"
                      >
                        →
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {(apiTool?.fields?.length ?? 0) > 0 && (
          <div className="grid gap-4 sm:grid-cols-2">
            {apiTool!.fields.map((f) => {
              const type = f.type || "text";
              if (type === "checkbox") {
                return (
                  <div
                    key={f.name}
                    className="sm:col-span-2 flex items-center justify-between gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-3 text-sm"
                  >
                    <label className="flex flex-1 cursor-pointer items-center gap-3">
                      <input
                        type="checkbox"
                        checked={Boolean(values[f.name])}
                        onChange={(e) => setValues((v) => ({ ...v, [f.name]: e.target.checked }))}
                        className="h-4 w-4 accent-[var(--accent)]"
                      />
                      <span>{f.label || f.name}</span>
                    </label>
                    <HelpTip text={f.help} />
                  </div>
                );
              }
              if (type === "textarea") {
                return (
                  <div key={f.name} className="sm:col-span-2">
                    <label className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--ink-secondary)]">
                      <span>{f.label || f.name}</span>
                      <HelpTip text={f.help} />
                    </label>
                    <textarea
                      rows={3}
                      value={String(values[f.name] ?? "")}
                      onChange={(e) => setValues((v) => ({ ...v, [f.name]: e.target.value }))}
                      className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3 font-mono text-xs"
                    />
                  </div>
                );
              }
              if (type === "select" || (f.options && f.options.length > 0)) {
                const selectOpts = (f.options ?? []).map((o) =>
                  typeof o === "string" ? { value: o, label: o } : o,
                );
                return (
                  <div key={f.name}>
                    <label className="mb-1.5 inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--ink-secondary)]">
                      <span>{f.label || f.name}</span>
                      <HelpTip text={f.help} />
                    </label>
                    <BasecoatSelect
                      ariaLabel={f.label || f.name}
                      value={String(values[f.name] ?? selectOpts[0]?.value ?? "")}
                      onChange={(val) => setValues((v) => ({ ...v, [f.name]: val }))}
                      options={selectOpts}
                    />
                  </div>
                );
              }
              return (
                <div key={f.name}>
                  <label className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--ink-secondary)]">
                    <span>{f.label || f.name}</span>
                    <HelpTip text={f.help} />
                  </label>
                  <input
                    type={type === "password" ? "password" : type === "number" ? "number" : "text"}
                    value={String(values[f.name] ?? "")}
                    onChange={(e) => setValues((v) => ({ ...v, [f.name]: e.target.value }))}
                    className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                  />
                </div>
              );
            })}
          </div>
        )}

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || (!files.length && !hasInlineHtml)}
          onClick={handleRun}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
          {busy ? "Processing…" : `Run ${title}`}
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
