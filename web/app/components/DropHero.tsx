import React, { useCallback, useRef, useState } from "react";
import { Link } from "react-router";
import { ArrowRight, FileText, Image as ImageIcon, ShieldCheck, Sparkles, UploadCloud, X } from "lucide-react";
import { useFileVault } from "~/context/file-vault";
import { formatBytes } from "~/lib/api";

const PDF_QUICK_ACTIONS = [
  { label: "Compress PDF", to: "/compress-pdf" },
  { label: "Split PDF", to: "/split-pdf" },
  { label: "PDF to Image", to: "/pdf-to-image" },
  { label: "Merge PDFs", to: "/merge-pdfs" },
  { label: "Watermark", to: "/watermark-pdf" },
  { label: "Protect", to: "/lock-pdf" },
  { label: "Sign PDF", to: "/sign-pdf" },
];

const IMAGE_QUICK_ACTIONS = [
  { label: "Compress Image", to: "/compress-image" },
  { label: "Images to PDF", to: "/image-to-pdf" },
];

export function DropHero() {
  const { files, addFiles, removeFile, clear } = useFileVault();
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const depth = useRef(0);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      depth.current = 0;
      setDragging(false);
      const dropped = Array.from(e.dataTransfer.files ?? []);
      if (dropped.length) addFiles(dropped);
    },
    [addFiles],
  );

  const hasPdf = files.some((f) => f.kind === "pdf");
  const hasImage = files.some((f) => f.kind === "image");
  const quickActions = [...(hasPdf ? PDF_QUICK_ACTIONS : []), ...(hasImage ? IMAGE_QUICK_ACTIONS : [])];

  return (
    <section className="mx-auto max-w-5xl px-4 pt-10 pb-8 sm:px-6">
      <div className="mx-auto max-w-2xl text-center">
        <span className="inline-flex items-center gap-2 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3.5 py-1 text-[11px] font-semibold tracking-wider text-[var(--accent)] uppercase">
          <ShieldCheck className="h-3.5 w-3.5" /> Self-hosted Axum · Shared File Vault
        </span>
        <h1 className="mt-4 text-4xl font-extrabold tracking-tight text-[var(--ink-primary)] sm:text-5xl">
          A little love for your <span className="text-[var(--accent)]">PDFs</span>.
        </h1>
        <p className="mt-3 text-base text-[var(--ink-secondary)]">
          In Mandarin, <strong className="font-semibold text-[var(--ink-primary)]">520</strong> sounds
          like <em>wǒ ài nǐ</em> (&ldquo;I love you&rdquo;). Drop files once and jump across 28 PDF,
          image, math, and resume tools without re-uploading.
        </p>
      </div>

      <div
        role="button"
        tabIndex={0}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && inputRef.current?.click()}
        onDragEnter={(e) => {
          e.preventDefault();
          depth.current += 1;
          setDragging(true);
        }}
        onDragLeave={(e) => {
          e.preventDefault();
          depth.current -= 1;
          if (depth.current <= 0) setDragging(false);
        }}
        onDragOver={(e) => e.preventDefault()}
        onDrop={onDrop}
        className={`mt-8 cursor-pointer rounded-3xl border-2 border-dashed p-8 text-center transition-all sm:p-11 ${
          dragging
            ? "border-[var(--accent)] bg-[var(--accent-soft)]"
            : "border-[var(--border-subtle)] bg-[var(--bg-surface)] hover:border-[var(--accent)]/60"
        }`}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept=".pdf,application/pdf,image/*,.json,.tex,.txt,.html,.htm,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.odt"
          className="hidden"
          onChange={(e) => {
            const picked = Array.from(e.target.files ?? []);
            if (picked.length) addFiles(picked);
            e.target.value = "";
          }}
        />
        <div className="mx-auto grid h-14 w-14 place-items-center rounded-2xl bg-[var(--accent-soft)] text-[var(--accent)]">
          <UploadCloud className="h-7 w-7" />
        </div>
        <h2 className="mt-4 text-lg font-bold text-[var(--ink-primary)]">
          {dragging ? "Release to add to File Vault" : "Drag & drop files into your 520pdf Vault"}
        </h2>
        <p className="mt-1 text-sm text-[var(--ink-secondary)]">
          PDFs, images, JSON, or LaTeX · or{" "}
          <span className="font-semibold text-[var(--accent)]">browse from disk</span>
        </p>
        <div className="mt-5 flex flex-wrap items-center justify-center gap-2 text-xs text-[var(--ink-muted)]">
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[var(--bg-subtle)] px-3 py-1">
            <FileText className="h-3.5 w-3.5" /> PDF
          </span>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[var(--bg-subtle)] px-3 py-1">
            <ImageIcon className="h-3.5 w-3.5" /> JPG · PNG · WebP
          </span>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[var(--bg-subtle)] px-3 py-1">
            <Sparkles className="h-3.5 w-3.5" /> Carried across every route
          </span>
        </div>
      </div>

      {files.length > 0 && (
        <div className="surface-card mt-5 rounded-2xl p-5 shadow-sm">
          <div className="mb-3 flex items-center justify-between gap-4">
            <p className="text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
              {files.length} file{files.length === 1 ? "" : "s"} in File Vault
            </p>
            <button
              type="button"
              onClick={clear}
              className="text-xs font-semibold text-[var(--ink-secondary)] hover:text-[var(--accent)]"
            >
              Clear vault
            </button>
          </div>

          <div className="grid gap-2.5 sm:grid-cols-2">
            {files.map((item) => (
              <div
                key={item.id}
                className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-2"
              >
                <div className="grid h-10 w-10 shrink-0 place-items-center overflow-hidden rounded-lg bg-[var(--accent-soft)]">
                  {item.kind === "image" ? (
                    <img src={item.url} alt="" className="h-full w-full object-cover" />
                  ) : (
                    <FileText className="h-5 w-5 text-[var(--accent)]" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold text-[var(--ink-primary)]">
                    {item.file.name}
                  </p>
                  <p className="text-xs text-[var(--ink-muted)] tabular-nums">
                    {formatBytes(item.file.size)}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => removeFile(item.id)}
                  aria-label={`Remove ${item.file.name}`}
                  className="rounded-lg p-1.5 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--accent)]"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>

          {quickActions.length > 0 && (
            <div className="mt-4 border-t border-[var(--border-subtle)] pt-4">
              <p className="mb-2 text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
                Jump directly to a tool
              </p>
              <div className="flex flex-wrap gap-2">
                {quickActions.map((act) => (
                  <Link
                    key={act.to}
                    to={act.to}
                    className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-1.5 text-xs font-semibold text-[var(--ink-primary)] transition-colors hover:border-[var(--accent)] hover:text-[var(--accent)]"
                  >
                    {act.label}
                    <ArrowRight className="h-3 w-3" />
                  </Link>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
