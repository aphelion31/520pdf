import { useEffect, useState } from "react";
import {
  Link,
  Links,
  Meta,
  Outlet,
  Scripts,
  ScrollRestoration,
  isRouteErrorResponse,
  useLocation,
} from "react-router";
import type { Route } from "./+types/root";
import {
  FileCode2,
  FileUser,
  FolderOpen,
  LayoutGrid,
  LogIn,
  Minimize2,
  Moon,
  Sigma,
  Sparkles,
  Sun,
  UploadCloud,
} from "lucide-react";
import { FileVaultProvider, useFileVault } from "./context/file-vault";
import "./app.css";

const THEME_INIT_SCRIPT = `(function(){try{var t=localStorage.getItem("520pdf.theme");var d=t==="dark"||(t==="auto"&&window.matchMedia("(prefers-color-scheme: dark)").matches);if(d){document.documentElement.classList.add("dark");document.documentElement.style.colorScheme="dark";}else{document.documentElement.style.colorScheme="light";}}catch(e){}})();`;

export function Layout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta
          name="description"
          content="520pdf — Single-binary Rust + React Router PDF workbench. Merge, split, compress, convert, sign, and build resumes locally."
        />
        <title>520pdf — A Little Love for Your PDFs</title>
        <link rel="icon" type="image/svg+xml" href="/brand-assets/favicon.svg" />
        <link rel="apple-touch-icon" href="/brand-assets/icon.svg" />
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
        <Meta />
        <Links />
      </head>
      <body className="min-h-dvh antialiased">
        {children}
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}

function SiteChrome({ children }: { children: React.ReactNode }) {
  const { files } = useFileVault();
  const { pathname } = useLocation();
  const [dark, setDark] = useState(false);

  useEffect(() => {
    setDark(document.documentElement.classList.contains("dark"));
  }, []);

  const toggleTheme = () => {
    const next = !dark;
    setDark(next);
    document.documentElement.classList.toggle("dark", next);
    document.documentElement.style.colorScheme = next ? "dark" : "light";
    try {
      localStorage.setItem("520pdf.theme", next ? "dark" : "light");
    } catch {
      // ignore storage failure
    }
  };

  const navLink = (to: string, label: string, Icon: React.ComponentType<{ className?: string }>) => {
    const active = pathname === to;
    return (
      <Link
        to={to}
        className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
          active
            ? "bg-[var(--accent-soft)] text-[var(--accent)]"
            : "text-[var(--ink-secondary)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
        }`}
      >
        <Icon className="h-[15px] w-[15px] shrink-0" aria-hidden="true" />
        <span>{label}</span>
      </Link>
    );
  };

  return (
    <div className="flex min-h-dvh flex-col">
      <header className="sticky top-0 z-40 border-b border-[var(--border-subtle)] bg-[var(--bg-surface)]/90 backdrop-blur-md">
        <div className="mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3 sm:px-6">
          <div className="flex items-center gap-6">
            <Link to="/" className="flex items-center gap-2" aria-label="520pdf home">
              <img
                src="/brand-assets/logo.svg"
                alt="520pdf — 我爱你 PDF"
                className="h-9 w-auto dark:hidden"
              />
              <img
                src="/brand-assets/logo_dark.svg"
                alt="520pdf — 我爱你 PDF"
                className="hidden h-9 w-auto dark:block"
              />
            </Link>
            <nav className="hidden items-center gap-1 md:flex" aria-label="Primary">
              {navLink("/", "All Tools", LayoutGrid)}
              {navLink("/drop", "Drop Hub", UploadCloud)}
              {navLink("/compress-pdf", "Compress", Minimize2)}
              {navLink("/resume-builder", "Resume", FileUser)}
              {navLink("/latex-resume", "LaTeX", FileCode2)}
              {navLink("/equation-to-image", "Equations", Sigma)}
            </nav>
          </div>

          <div className="flex items-center gap-2">
            <Link
              to="/drop"
              className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-xs font-semibold text-[var(--ink-primary)] transition-colors hover:border-[var(--accent)] hover:text-[var(--accent)]"
              title="Shared File Vault across tools"
            >
              {files.length > 0 ? <FolderOpen className="h-3.5 w-3.5 text-[var(--accent)]" /> : <UploadCloud className="h-3.5 w-3.5" />}
              <span className="tabular-nums">{files.length}</span>
              <span className="hidden sm:inline">in vault</span>
            </Link>

            <Link
              to="/login"
              className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-xs font-semibold text-[var(--ink-secondary)] transition-colors hover:border-[var(--accent)] hover:text-[var(--ink-primary)]"
              title="Workspace sign in (Argon2id)"
            >
              <LogIn className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">Sign in</span>
            </Link>

            <button
              type="button"
              onClick={toggleTheme}
              aria-label="Toggle color theme"
              className="grid h-9 w-9 place-items-center rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--ink-secondary)] transition-colors hover:text-[var(--ink-primary)]"
            >
              {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
            </button>
          </div>
        </div>
      </header>

      <main className="flex-1">{children}</main>

      <footer className="mt-16 border-t border-[var(--border-subtle)] bg-[var(--bg-surface)]">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-4 py-6 text-xs text-[var(--ink-muted)] sm:px-6">
          <div className="flex items-center gap-2">
            <img src="/brand-assets/icon.svg" alt="" className="h-5 w-5" />
            <span className="font-semibold text-[var(--ink-primary)]">520pdf</span>
            <span>·</span>
            <span>我爱你 PDF — Single-binary Rust Axum + React Router 8</span>
          </div>
          <div className="flex items-center gap-4">
            <span className="inline-flex items-center gap-1">
              <Sparkles className="h-3.5 w-3.5 text-[var(--accent)]" /> 28 self-hosted PDF & document tools
            </span>
            <Link to="/drop" className="hover:text-[var(--ink-primary)]">
              Drop Hub
            </Link>
          </div>
        </div>
      </footer>
    </div>
  );
}

export default function App() {
  return (
    <FileVaultProvider>
      <SiteChrome>
        <Outlet />
      </SiteChrome>
    </FileVaultProvider>
  );
}

export function HydrateFallback() {
  return (
    <div className="grid min-h-dvh place-items-center text-sm text-[var(--ink-muted)]">
      Loading 520pdf…
    </div>
  );
}

export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
  const title = isRouteErrorResponse(error) ? `${error.status}` : "Something went wrong";
  const detail = isRouteErrorResponse(error)
    ? error.statusText
    : error instanceof Error
      ? error.message
      : "Unexpected error";
  return (
    <main className="mx-auto max-w-xl p-10">
      <h1 className="text-2xl font-bold">{title}</h1>
      <p className="mt-2 text-sm text-[var(--ink-secondary)]">{detail}</p>
      <Link
        to="/"
        className="mt-6 inline-block rounded-lg bg-[var(--accent)] px-4 py-2 text-sm font-semibold text-white"
      >
        Back to 520pdf
      </Link>
    </main>
  );
}
