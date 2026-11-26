import { useMemo, useState } from "react";
import { Link } from "react-router";
import { ArrowRight, Cpu, Layers, Search, Zap } from "lucide-react";
import { DropHero } from "~/components/DropHero";
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

  return (
    <div>
      <DropHero />

      <section id="tools" className="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
          <div>
            <div className="flex items-center gap-2">
              <span className="h-0.5 w-5 bg-[var(--accent)]" />
              <span className="text-xs font-bold tracking-widest text-[var(--accent)] uppercase">
                Toolkit
              </span>
            </div>
            <h2 className="mt-1 text-2xl font-extrabold tracking-tight text-[var(--ink-primary)] sm:text-3xl">
              {TOOLS_CATALOG.length} tools, one single Rust binary
            </h2>
          </div>

          <div className="relative w-full sm:w-72">
            <Search className="pointer-events-none absolute top-1/2 left-3.5 h-4 w-4 -translate-y-1/2 text-[var(--ink-muted)]" />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search 36 PDF tools…"
              aria-label="Search PDF tools"
              className="w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] py-2.5 pr-4 pl-10 text-sm text-[var(--ink-primary)] placeholder:text-[var(--ink-muted)] focus:border-[var(--accent)] focus:outline-none"
            />
          </div>
        </div>

        <div className="mt-5 flex flex-wrap gap-2" role="group" aria-label="Filter by category">
          {CATEGORIES.map((cat) => {
            const active = category === cat;
            const CatIcon = CATEGORY_META[cat].icon;
            return (
              <button
                key={cat}
                type="button"
                aria-pressed={active}
                onClick={() => setCategory(cat)}
                className={`inline-flex items-center gap-1.5 rounded-lg px-3.5 py-1.5 text-xs font-semibold transition-colors ${
                  active
                    ? "bg-[var(--ink-primary)] text-[var(--bg-surface)]"
                    : "border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)]"
                }`}
              >
                <CatIcon className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                <span>{cat}</span>
              </button>
            );
          })}
        </div>

        <div className="mt-6 grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-4">
          {filtered.map((tool) => (
            <Link
              key={tool.id}
              to={tool.route}
              className={`surface-card group relative flex flex-col justify-between rounded-2xl p-5 transition-all hover:-translate-y-0.5 hover:border-[var(--accent)]/50 hover:shadow-md ${
                tool.core ? "ring-1 ring-[var(--accent)]/15" : ""
              }`}
            >
              <div>
                <div className="mb-3 flex items-start justify-between gap-2">
                  <ToolIconBadge toolId={tool.id} category={tool.category} size="md" />
                  <div className="flex flex-wrap items-center justify-end gap-1.5">
                    <span
                      className={`rounded-full px-2.5 py-0.5 text-[10px] font-bold tracking-wider uppercase ${CATEGORY_META[tool.category].pillClass}`}
                    >
                      {tool.category}
                    </span>
                    {tool.core && (
                      <span className="rounded-full border border-[var(--border-subtle)] px-2 py-0.5 text-[10px] font-semibold text-[var(--ink-muted)]">
                        Core
                      </span>
                    )}
                  </div>
                </div>
                <h3 className="text-base font-bold text-[var(--ink-primary)] group-hover:text-[var(--accent)]">
                  {tool.name}
                </h3>
                <p className="mt-1.5 text-xs leading-relaxed text-[var(--ink-secondary)]">
                  {tool.description}
                </p>
              </div>
              <div className="mt-4 inline-flex items-center gap-1 text-xs font-semibold text-[var(--ink-primary)]">
                <span>Open tool</span>
                <ArrowRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
              </div>
            </Link>
          ))}
        </div>

        {filtered.length === 0 && (
          <p className="mt-8 rounded-2xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] p-8 text-center text-sm text-[var(--ink-secondary)]">
            No tools match &ldquo;{query}&rdquo;. Clear search or choose All categories.
          </p>
        )}

        <div className="mt-10 grid grid-cols-1 gap-4 md:grid-cols-3">
          {[
            { value: "1 binary", label: "Axum 0.8 + React Router 8 SPA", icon: Cpu },
            { value: "36 tools", label: "13 core local + 23 PDF & Office tools", icon: Layers },
            { value: "0 MB", label: "No Node or Python runtime needed", icon: Zap },
          ].map((stat) => {
            const StatIcon = stat.icon;
            return (
              <div
                key={stat.label}
                className="surface-card flex h-full items-center gap-4 rounded-2xl p-5"
              >
                <span className="inline-grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent)]">
                  <StatIcon className="h-5 w-5" aria-hidden="true" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="whitespace-nowrap text-2xl leading-none font-extrabold tracking-tight text-[var(--accent)] tabular-nums">
                    {stat.value}
                  </div>
                  <div className="mt-1.5 text-xs leading-snug text-[var(--ink-secondary)]">
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
