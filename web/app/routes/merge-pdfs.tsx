import { useEffect, useState } from "react";
import { Layers, Loader2 } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";

export default function MergePdfs() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => match(".pdf").map((v) => v.file));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  useEffect(() => {
    if (files.length === 0) {
      const vaultPdfs = match(".pdf").map((v) => v.file);
      if (vaultPdfs.length) setFiles(vaultPdfs);
    }
  }, [match]);

  const handleMerge = async () => {
    if (files.length < 2) {
      setError("Add at least two PDF files to merge.");
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await runTool("merge", files);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Merge failed");
    } finally {
      setBusy(false);
    }
  };

  const totalInput = files.reduce((sum, f) => sum + f.size, 0);

  return (
    <ToolShell
      title="Merge PDFs"
      description="Combine multiple PDF documents in order using native Rust lopdf processing."
      category="Organize"
    >
      <div className="surface-card space-y-5 rounded-2xl p-6">
        <FileDropzone
          accept=".pdf"
          multiple
          files={files}
          onFilesChange={(next) => {
            setFiles(next);
            setResult(null);
          }}
        />

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || files.length < 2}
          onClick={handleMerge}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Layers className="h-4 w-4" />}
          {busy ? "Merging PDFs…" : `Merge ${files.length} PDFs`}
        </button>

        {result && (
          <ResultBanner blob={result.blob} filename={result.filename} originalBytes={totalInput} />
        )}
      </div>
    </ToolShell>
  );
}
