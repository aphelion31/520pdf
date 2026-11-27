import React, { useRef, useState } from "react";
import { ArrowDown, ArrowUp, FileText, Trash2, UploadCloud } from "lucide-react";
import { formatBytes } from "~/lib/api";

interface FileDropzoneProps {
  accept: string;
  multiple?: boolean;
  files: File[];
  onFilesChange: (files: File[]) => void;
  label?: string;
}

export function FileDropzone({
  accept,
  multiple = false,
  files,
  onFilesChange,
  label,
}: FileDropzoneProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);

  const acceptList = accept
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean);

  const accepts = (file: File) => {
    if (!acceptList.length || acceptList.includes("*")) return true;
    const lower = file.name.toLowerCase();
    return acceptList.some((rule) =>
      rule.startsWith(".")
        ? lower.endsWith(rule)
        : rule.endsWith("/*")
          ? file.type.toLowerCase().startsWith(rule.slice(0, -1))
          : file.type.toLowerCase() === rule,
    );
  };

  const ingest = (incoming: File[]) => {
    const filtered = incoming.filter((f) => f.size > 0 && accepts(f));
    if (!filtered.length) return;
    if (multiple) {
      onFilesChange([...files, ...filtered].slice(0, 30));
    } else {
      onFilesChange([filtered[0]]);
    }
  };

  const move = (idx: number, delta: -1 | 1) => {
    const next = [...files];
    const target = idx + delta;
    if (target < 0 || target >= next.length) return;
    [next[idx], next[target]] = [next[target], next[idx]];
    onFilesChange(next);
  };

  return (
    <div className="space-y-3">
      <div
        role="button"
        tabIndex={0}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && inputRef.current?.click()}
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          ingest(Array.from(e.dataTransfer.files ?? []));
        }}
        className={`cursor-pointer rounded-2xl border border-dashed p-6 text-center transition-colors duration-150 ${
          dragging
            ? "border-[var(--accent)] bg-[var(--accent-soft)]"
            : "border-[var(--border-strong)] bg-[var(--bg-band)] hover:border-[var(--ink-muted)]"
        }`}
      >
        <input
          ref={inputRef}
          type="file"
          accept={accept}
          multiple={multiple}
          className="hidden"
          onChange={(e) => {
            ingest(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />
        <UploadCloud className="mx-auto h-7 w-7 text-[var(--accent)]" />
        <p className="mt-2 text-sm font-semibold text-[var(--ink-primary)]">
          {label || (multiple ? "Drop files here or click to add" : "Drop a file here or click to choose")}
        </p>
        <p className="mt-0.5 text-xs text-[var(--ink-muted)]">Accepted: {accept}</p>
      </div>

      {files.length > 0 && (
        <ul className="divide-y divide-[var(--border-subtle)] rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)]">
          {files.map((file, idx) => (
            <li key={`${file.name}-${idx}`} className="flex items-center gap-3 px-3.5 py-2.5">
              <FileText className="h-4 w-4 shrink-0 text-[var(--accent)]" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-[var(--ink-primary)]">
                  {idx + 1}. {file.name}
                </p>
                <p className="text-xs text-[var(--ink-muted)] tabular-nums">{formatBytes(file.size)}</p>
              </div>
              <div className="flex items-center gap-1">
                {multiple && (
                  <>
                    <button
                      type="button"
                      disabled={idx === 0}
                      onClick={() => move(idx, -1)}
                      aria-label={`Move ${file.name} earlier`}
                      className="rounded-lg p-1.5 text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)] disabled:opacity-30"
                    >
                      <ArrowUp className="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      disabled={idx === files.length - 1}
                      onClick={() => move(idx, 1)}
                      aria-label={`Move ${file.name} later`}
                      className="rounded-lg p-1.5 text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)] disabled:opacity-30"
                    >
                      <ArrowDown className="h-3.5 w-3.5" />
                    </button>
                  </>
                )}
                <button
                  type="button"
                  onClick={() => onFilesChange(files.filter((_, i) => i !== idx))}
                  aria-label={`Remove ${file.name}`}
                  className="rounded-lg p-1.5 text-[var(--ink-muted)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent)]"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
