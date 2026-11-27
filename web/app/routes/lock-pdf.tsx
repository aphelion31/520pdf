import { useState } from "react";
import { Loader2, Lock, ShieldAlert } from "lucide-react";
import { FileDropzone } from "~/components/FileDropzone";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { useFileVault } from "~/context/file-vault";
import { runTool, type ToolResponse } from "~/lib/api";

export default function LockPdf() {
  const { match } = useFileVault();
  const [files, setFiles] = useState<File[]>(() => {
    const first = match(".pdf")[0];
    return first ? [first.file] : [];
  });
  const [mode, setMode] = useState<"visual" | "password">("visual");
  const [watermarkText, setWatermarkText] = useState("CONFIDENTIAL");
  const [addOverlay, setAddOverlay] = useState(true);
  const [removeMetadata, setRemoveMetadata] = useState(true);
  const [password, setPassword] = useState("");
  const [ownerPassword, setOwnerPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const handleProtect = async () => {
    if (!files[0]) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const options: Record<string, string> =
        mode === "visual"
          ? {
              mode: "visual",
              watermark_text: watermarkText,
              add_overlay: String(addOverlay),
              remove_metadata: String(removeMetadata),
            }
          : {
              mode: "password",
              password,
              owner_password: ownerPassword,
            };
      const res = await runTool("protect", files, options);
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Protection failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Protect PDF"
      description="Add visual confidentiality stamps and remove hidden metadata, or lock your PDF with a password."
      category="Security"
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

        <div className="grid gap-3 sm:grid-cols-2">
          <button
            type="button"
            onClick={() => setMode("visual")}
            className={`rounded-xl border p-4 text-left transition-colors ${
              mode === "visual"
                ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                : "border-[var(--border-subtle)] bg-[var(--bg-page)]"
            }`}
          >
            <p className="text-sm font-bold text-[var(--ink-primary)]">
              Visual Protection + Metadata Wipe
            </p>
            <p className="mt-0.5 text-xs text-[var(--ink-muted)]">
              Instant stamp, border frame & clears hidden document info
            </p>
          </button>

          <button
            type="button"
            onClick={() => setMode("password")}
            className={`rounded-xl border p-4 text-left transition-colors ${
              mode === "password"
                ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                : "border-[var(--border-subtle)] bg-[var(--bg-page)]"
            }`}
          >
            <p className="text-sm font-bold text-[var(--ink-primary)]">Password Encryption</p>
            <p className="mt-0.5 text-xs text-[var(--ink-muted)]">
              Lock file access & editing with AES-256 passwords
            </p>
          </button>
        </div>

        {mode === "visual" ? (
          <div className="space-y-4">
            <div>
              <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                Protection overlay text
              </label>
              <input
                type="text"
                value={watermarkText}
                onChange={(e) => setWatermarkText(e.target.value)}
                disabled={!addOverlay}
                className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm disabled:opacity-50"
              />
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <label className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-3 text-sm">
                <input
                  type="checkbox"
                  checked={addOverlay}
                  onChange={(e) => setAddOverlay(e.target.checked)}
                  className="h-4 w-4 accent-[var(--accent)]"
                />
                <span>Add diagonal stamp + border</span>
              </label>

              <label className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-3 text-sm">
                <input
                  type="checkbox"
                  checked={removeMetadata}
                  onChange={(e) => setRemoveMetadata(e.target.checked)}
                  className="h-4 w-4 accent-[var(--accent)]"
                />
                <span>Strip author / title metadata</span>
              </label>
            </div>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  User password (required)
                </label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Document open password"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Owner password (must differ)
                </label>
                <input
                  type="password"
                  value={ownerPassword}
                  onChange={(e) => setOwnerPassword(e.target.value)}
                  placeholder="Permissions password"
                  className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
                />
              </div>
            </div>
            <p className="inline-flex items-center gap-2 text-xs text-[var(--ink-muted)]">
              <ShieldAlert className="h-3.5 w-3.5" /> Passwords stay private and are never written to logs.
            </p>
          </div>
        )}

        {error && (
          <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <button
          type="button"
          disabled={busy || !files[0]}
          onClick={handleProtect}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Lock className="h-4 w-4" />}
          {busy ? "Protecting PDF…" : "Protect PDF"}
        </button>

        {result && (
          <ResultBanner blob={result.blob} filename={result.filename} originalBytes={files[0]?.size} />
        )}
      </div>
    </ToolShell>
  );
}
