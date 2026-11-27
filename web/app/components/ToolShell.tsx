import React from "react";
import { Link, useLocation } from "react-router";
import { ArrowLeft, ShieldCheck } from "lucide-react";
import { TOOLS_CATALOG } from "~/lib/catalog";
import { ToolIconBadge } from "~/lib/tool-icons";
import { ToolProgressBar } from "~/components/ToolProgressBar";

interface ToolShellProps {
  title: string;
  description: string;
  category: string;
  toolId?: string;
  headerActions?: React.ReactNode;
  children: React.ReactNode;
}

export function ToolShell({
  title,
  description,
  category,
  toolId,
  headerActions,
  children,
}: ToolShellProps) {
  const { pathname } = useLocation();
  const matchedTool = TOOLS_CATALOG.find(
    (item) => (toolId && item.id === toolId) || item.route === pathname,
  );

  return (
    <div className="mx-auto max-w-7xl px-4 py-6 sm:px-6">
      <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
        <Link
          to="/"
          className="cm-btn inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-medium text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
        >
          <ArrowLeft className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> All 36 tools
        </Link>
        <span className="inline-flex items-center gap-1.5 text-xs text-[var(--ink-muted)]">
          <ShieldCheck className="h-3.5 w-3.5 text-[var(--state-ok-fg)]" />
          Processed locally · Files stay private
        </span>
      </div>

      <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex min-w-0 items-start gap-3.5">
          {matchedTool && (
            <ToolIconBadge toolId={matchedTool.id} category={matchedTool.category} size="lg" />
          )}
          <div className="min-w-0">
            <p className="text-xs font-medium text-[var(--ink-muted)]">{category} · Local workbench</p>
            <h1 className="mt-0.5 text-2xl font-semibold tracking-tight text-[var(--ink-primary)]">
              {title}
            </h1>
            <p className="mt-1 max-w-2xl text-sm text-[var(--ink-secondary)]">{description}</p>
          </div>
        </div>

        {headerActions && (
          <div className="flex flex-wrap items-center gap-2 shrink-0">{headerActions}</div>
        )}
      </div>

      <div className="cm-lift">
        <ToolProgressBar className="mb-4" />
        {children}
      </div>
    </div>
  );
}
