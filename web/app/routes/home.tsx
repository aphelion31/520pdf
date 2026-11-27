import type React from "react";
import { useMemo, useState } from "react";
import { Link } from "react-router";
import { ArrowRight, Cpu, Layers, Search, Zap } from "lucide-react";
import { DropHero } from "~/components/DropHero";
import { SlidingTabs } from "~/components/SlidingTabs";
import { CATEGORIES, TOOLS_CATALOG } from "~/lib/catalog";
import { CATEGORY_META, ToolIconBadge } from "~/lib/tool-icons";

export default function Home() {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<(typeof CATEGORIES)[number]>("All");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return TOOLS_CATALOG.filter((tool) => {
      const catMatch = category === "All" || tool.category === category;
      const textMatch =
        !q ||
        `${tool.name} ${tool.description} ${tool.category} ${tool.id}`.toLowerCase().includes(q);
      return catMatch && textMatch;
    });
  }, [query, category]);

  const categoryItems = useMemo(
    () =>
      CATEGORIES.map((cat) => {
        const CatIcon = CATEGORY_META[cat].icon;
        return {
          value: cat,
          label: cat,
          icon: <CatIcon className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />,
        };
      }),
    [],
  );

  return (
    <div>
      <DropHero />

      <section id="tools" className="mx-auto max-w-7xl px-4 py-8 sm:px-6">
        <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
          <div>
            <p className="text-xs font-medium text-[var(--ink-muted)]">Toolkit</p>
            <h2 className="mt-1 text-2xl font-semibold tracking-tight text-[var(--ink-primary)] sm:text-3xl">
              {TOOLS_CATALOG.length} tools, one single Rust binary
            </h2>
          </div>

          <div className="relative w-full sm:w-72">
            <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-[var(--ink-muted)]" />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search 36 PDF tools…"
              aria-label="Search PDF tools"
              className="cm-input h-9 pr-3 pl-9 text-sm"
            />
          </div>
        </div>

        <div className="mt-5">
          <SlidingTabs
            ariaLabel="Filter by category"
            value={category}
            onChange={(v) => setCategory(v as (typeof CATEGORIES)[number])}
            items={categoryItems}
          />
        </div>

        <div className="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {filtered.map((tool, idx) => (
            <Link
              key={tool.id}
              to={tool.route}
              style={{ "--stagger": `${Math.min(idx, 11) * 20}ms` } as React.CSSProperties}
              className="surface-card cm-glare cm-lift group relative flex flex-col justify-between rounded-xl p-4 transition-colors duration-150 hover:border-[var(--border-strong)]"
            >
              <div>
                <div className="mb-3 flex items-start justify-between gap-2">
                  <ToolIconBadge toolId={tool.id} category={tool.category} size="md" />
                  <div className="flex flex-wrap items-center justify-end gap-1.5">
                    <span className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-subtle)] px-2 py-0.5 text-[11px] font-medium text-[var(--ink-secondary)]">
                      {tool.category}
                    </span>
                    {tool.core && (
                      <span className="rounded-md border border-[var(--border-subtle)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--ink-muted)]">
                        Core
                      </span>
                    )}
                  </div>
                </div>
                <h3 className="text-sm font-semibold tracking-tight text-[var(--ink-primary)]">
                  {tool.name}
                </h3>
                <p className="mt-1 text-xs leading-relaxed text-[var(--ink-secondary)]">
                  {tool.description}
                </p>
              </div>
              <div className="mt-4 inline-flex items-center gap-1 text-xs font-medium text-[var(--ink-secondary)] transition-colors group-hover:text-[var(--ink-primary)]">
                <span>Open tool</span>
                <ArrowRight className="h-3.5 w-3.5 transition-transform duration-150 group-hover:translate-x-0.5" />
              </div>
            </Link>
          ))}
        </div>

        {filtered.length === 0 && (
          <p className="mt-8 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] p-8 text-center text-sm text-[var(--ink-secondary)]">
            No tools match &ldquo;{query}&rdquo;. Clear search or choose All categories.
          </p>
        )}

        <div className="mt-10 grid grid-cols-1 gap-4 md:grid-cols-3">
          {[
            { value: "1 binary", label: "Axum 0.8 + React Router 8 SPA", icon: Cpu },
            { value: "36 tools", label: "13 core local + 23 PDF & Office tools", icon: Layers },
            { value: "0 MB", label: "No Node or Python runtime needed", icon: Zap },
          ].map((stat, idx) => {
            const StatIcon = stat.icon;
            return (
              <div
                key={stat.label}
                style={{ "--stagger": `${idx * 45}ms` } as React.CSSProperties}
                className="surface-card cm-glare cm-lift flex h-full items-center gap-4 rounded-xl p-5"
              >
                <span className="inline-grid h-10 w-10 shrink-0 place-items-center rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]">
                  <StatIcon className="h-4 w-4" aria-hidden="true" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="whitespace-nowrap text-3xl leading-none font-light tracking-tight text-[var(--ink-primary)] tabular-nums">
                    {stat.value}
                  </div>
                  <div className="mt-1.5 text-xs font-medium leading-snug text-[var(--ink-muted)]">
                    {stat.label}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}
