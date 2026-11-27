import React, { useCallback, useRef, useState } from "react";
import { Link } from "react-router";
import { ArrowRight, FileText, Image as ImageIcon, ShieldCheck, Sparkles, UploadCloud, X } from "lucide-react";
import { useFileVault } from "~/context/file-vault";
import { formatBytes } from "~/lib/api";
import { ToolIconBadge } from "~/lib/tool-icons";

const PDF_QUICK_ACTIONS = [
  { toolId: "compress", label: "Compress PDF", to: "/compress-pdf" },
  { toolId: "split", label: "Split PDF", to: "/split-pdf" },
  { toolId: "pdf-to-image", label: "PDF to Image", to: "/pdf-to-image" },
  { toolId: "merge", label: "Merge PDFs", to: "/merge-pdfs" },
  { toolId: "watermark", label: "Watermark", to: "/watermark-pdf" },
  { toolId: "protect", label: "Protect", to: "/lock-pdf" },
  { toolId: "sign", label: "Sign PDF", to: "/sign-pdf" },
  { toolId: "edit-pdf", label: "Edit PDF", to: "/edit-pdf" },
  { toolId: "pdf-to-word", label: "PDF to Word", to: "/tools/pdf-to-word" },
];

const IMAGE_QUICK_ACTIONS = [
  { toolId: "compress-image", label: "Compress Image", to: "/compress-image" },
  { toolId: "image-to-pdf", label: "Images to PDF", to: "/image-to-pdf" },
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
    <section className="mx-auto max-w-7xl px-4 pt-10 pb-8 sm:px-6">
      <div className="cm-lift mx-auto max-w-2xl text-center">
        <span className="inline-flex items-center gap-2 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1 text-xs font-medium text-[var(--ink-secondary)]">
          <ShieldCheck className="h-3.5 w-3.5 text-[var(--state-ok-fg)]" />
          Private local processing · Shared File Vault
        </span>
        <h1 className="mt-4 text-3xl font-semibold tracking-tight text-[var(--ink-primary)] sm:text-5xl">
          A little love for your <span className="text-[var(--accent)]">PDFs</span>.
        </h1>
        <p className="mt-3 text-sm leading-relaxed text-[var(--ink-secondary)] sm:text-base">
          In Mandarin, <strong className="font-semibold text-[var(--ink-primary)]">520</strong> sounds
          like <em>wǒ ài nǐ</em> (&ldquo;I love you&rdquo;). Drop files once and jump across 36 PDF,
          Office, image, math, and resume tools without re-uploading.
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
        className={`cm-lift mt-8 cursor-pointer rounded-2xl border border-dashed p-8 text-center transition-all sm:p-11 ${
          dragging
            ? "border-[var(--accent)] bg-[var(--accent-soft)]"
            : "border-[var(--border-strong)] bg-[var(--bg-surface)] hover:border-[var(--ink-muted)] hover:bg-[var(--bg-band)]"
        }`}
        style={{ "--stagger": "50ms" } as React.CSSProperties}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept=".pdf,application/pdf,image/*,.json,.tex,.txt,.html,.htm,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.odt,.odp,.ods,.csv,.rtf"
          className="hidden"
          onChange={(e) => {
            const picked = Array.from(e.target.files ?? []);
            if (picked.length) addFiles(picked);
            e.target.value = "";
          }}
        />
        <div className="mx-auto grid h-12 w-12 place-items-center rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]">
          <UploadCloud className="h-5 w-5" />
        </div>
        <h2 className="mt-4 text-base font-semibold tracking-tight text-[var(--ink-primary)]">
          {dragging ? "Release to add to File Vault" : "Drag & drop files into your 520pdf Vault"}
        </h2>
        <p className="mt-1 text-xs text-[var(--ink-secondary)]">
          PDFs, images, JSON, or LaTeX · or{" "}
          <span className="font-medium text-[var(--ink-primary)] underline decoration-[var(--border-strong)] underline-offset-4">
            browse from disk
          </span>
        </p>
        <div className="mt-5 flex flex-wrap items-center justify-center gap-2 text-xs text-[var(--ink-muted)]">
          <span className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1">
            <FileText className="h-3.5 w-3.5" /> PDF
          </span>
          <span className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1">
            <ImageIcon className="h-3.5 w-3.5" /> JPG · PNG · WebP
          </span>
          <span className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1">
            <Sparkles className="h-3.5 w-3.5" /> Carried across every route
          </span>
        </div>
      </div>

      {files.length > 0 && (
        <div className="surface-card cm-lift mt-4 rounded-2xl p-5">
          <div className="mb-3 flex items-center justify-between gap-4">
            <p className="text-xs font-medium text-[var(--ink-muted)]">
              {files.length} file{files.length === 1 ? "" : "s"} in File Vault
            </p>
            <button
              type="button"
              onClick={clear}
              className="cm-btn rounded-md px-2 py-1 text-xs font-medium text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
            >
              Clear vault
            </button>
          </div>

          <div className="grid gap-2 sm:grid-cols-2">
            {files.map((item) => (
              <div
                key={item.id}
                className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-2"
              >
                <div className="grid h-9 w-9 shrink-0 place-items-center overflow-hidden rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-subtle)]">
                  {item.kind === "image" ? (
                    <img src={item.url} alt="" className="h-full w-full object-cover" />
                  ) : (
                    <FileText className="h-4 w-4 text-[var(--ink-secondary)]" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-[var(--ink-primary)]">
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
                  className="cm-btn rounded-lg p-1.5 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>

          {quickActions.length > 0 && (
            <div className="mt-4 border-t border-[var(--border-subtle)] pt-4">
              <p className="mb-2 text-xs font-medium text-[var(--ink-muted)]">
                Jump directly to a tool
              </p>
              <div className="flex flex-wrap gap-1.5">
                {quickActions.map((act) => (
                  <Link
                    key={act.to}
                    to={act.to}
                    className="cm-btn inline-flex items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)] hover:bg-[var(--bg-subtle)]"
                  >
                    <ToolIconBadge toolId={act.toolId} size="xs" />
                    {act.label}
                    <ArrowRight className="h-3 w-3 text-[var(--ink-muted)]" />
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
