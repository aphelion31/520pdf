import React from "react";
import { Link } from "react-router";
import { ArrowLeft, ShieldCheck } from "lucide-react";

interface ToolShellProps {
  title: string;
  description: string;
  category: string;
  children: React.ReactNode;
}

export function ToolShell({ title, description, category, children }: ToolShellProps) {
  return (
    <div className="mx-auto max-w-5xl px-4 py-8 sm:px-6">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <Link
          to="/"
          className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--ink-secondary)] hover:text-[var(--accent)]"
        >
          <ArrowLeft className="h-3.5 w-3.5" /> All 28 tools
        </Link>
        <span className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1 text-[11px] font-semibold text-[var(--ink-secondary)]">
          <ShieldCheck className="h-3.5 w-3.5 text-[var(--accent)]" /> Local Axum Worker · No external retention
        </span>
      </div>

      <div className="mb-6">
        <span className="inline-block rounded-full bg-[var(--accent-soft)] px-2.5 py-0.5 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
          {category}
        </span>
        <h1 className="mt-2 text-3xl font-extrabold tracking-tight text-[var(--ink-primary)]">
          {title}
        </h1>
        <p className="mt-1.5 text-sm text-[var(--ink-secondary)]">{description}</p>
      </div>

      {children}
    </div>
  );
}
