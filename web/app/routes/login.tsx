import React, { useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";
import { KeyRound, Loader2, LogOut, ShieldCheck, UserPlus } from "lucide-react";
import { SlidingTabs } from "~/components/SlidingTabs";

interface AuthStatus {
  auth_enabled: boolean;
  authenticated: boolean;
  username: string | null;
  algorithm: string;
  registration_enabled?: boolean;
}

function safeNextPath(raw: string | null): string {
  if (!raw || !raw.startsWith("/") || raw.startsWith("//")) return "/";
  return raw;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const nextPath = safeNextPath(searchParams.get("next"));

  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = () => {
    fetch("/api/auth/me", { headers: { Accept: "application/json" } })
      .then((r) => r.json())
      .then((data: AuthStatus) => {
        setStatus(data);
        if (data.registration_enabled === false) {
          setMode("login");
        }
      })
      .catch(() => setStatus(null));
  };

  useEffect(() => {
    loadStatus();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const endpoint = mode === "register" ? "/api/auth/register" : "/api/auth/login";
      const res = await fetch(endpoint, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
      });
      const body = (await res.json()) as { error?: string };
      if (!res.ok) throw new Error(body.error || "Authentication failed");
      loadStatus();
      navigate(nextPath);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Authentication failed");
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = async () => {
    await fetch("/api/auth/logout", { method: "POST" });
    loadStatus();
  };

  return (
    <div className="mx-auto flex max-w-md flex-col justify-center px-4 py-14 sm:px-6">
      <div className="surface-card cm-lift space-y-6 rounded-2xl p-7">
        <div className="text-center">
          <img src="/brand-assets/logo.svg" alt="520pdf" className="mx-auto h-9 w-auto dark:hidden" />
          <img
            src="/brand-assets/logo_dark.svg"
            alt="520pdf"
            className="mx-auto hidden h-9 w-auto dark:block"
          />
          <span className="mt-4 inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-subtle)] px-3 py-1 text-xs font-medium text-[var(--ink-secondary)]">
            <ShieldCheck className="h-3.5 w-3.5 text-[var(--state-ok-fg)]" /> Argon2id (PHC v=19) Auth
          </span>
          <h1 className="mt-3 text-2xl font-semibold tracking-tight text-[var(--ink-primary)]">
            Workspace &amp; API Key Access
          </h1>
          <p className="mt-1 text-xs leading-relaxed text-[var(--ink-secondary)]">
            Passwords are hashed strictly with Argon2id + HMAC-SHA256 HttpOnly sessions.
          </p>
        </div>

        {status?.authenticated ? (
          <div className="space-y-4 rounded-xl border border-[var(--state-ok-border)] bg-[var(--state-ok-bg)] p-5 text-center">
            <p className="text-sm font-medium text-[var(--ink-primary)]">
              Signed in as <span className="font-semibold text-[var(--state-ok-fg)]">{status.username}</span>
            </p>
            <div className="flex flex-wrap gap-2">
              <Link
                to="/api-keys"
                className="cm-btn flex-1 rounded-xl bg-[var(--accent)] py-2.5 text-center text-xs font-semibold text-white hover:bg-[var(--accent-hover)]"
              >
                Manage API Keys
              </Link>
              <Link
                to="/"
                className="cm-btn flex-1 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] py-2.5 text-center text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
              >
                Workbench
              </Link>
              <button
                type="button"
                onClick={handleLogout}
                className="cm-btn inline-flex w-full items-center justify-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-4 py-2 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
              >
                <LogOut className="h-3.5 w-3.5" /> Sign out
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            {status?.registration_enabled !== false && (
              <SlidingTabs
                ariaLabel="Authentication mode"
                fullWidth
                value={mode}
                onChange={(v) => {
                  setMode(v as "login" | "register");
                  setError(null);
                }}
                items={[
                  { value: "login", label: "Sign in" },
                  { value: "register", label: "Create account" },
                ]}
              />
            )}

            <div>
              <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                Username
              </label>
              <input
                type="text"
                name="username"
                required
                minLength={3}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="developer"
                className="cm-input mt-1.5 h-10 px-3.5 text-sm"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                Password {mode === "register" && <span className="font-normal text-[var(--ink-muted)]">(min 8 chars)</span>}
              </label>
              <input
                type="password"
                name="password"
                required
                minLength={mode === "register" ? 8 : 1}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••••••"
                className="cm-input mt-1.5 h-10 px-3.5 text-sm"
              />
            </div>

            {error && (
              <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs text-[var(--state-bad-fg)]">
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={busy}
              className="cm-btn inline-flex h-10 w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-4 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
            >
              {busy ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : mode === "register" ? (
                <UserPlus className="h-4 w-4" />
              ) : (
                <KeyRound className="h-4 w-4" />
              )}
              {busy
                ? "Hashing Argon2id…"
                : mode === "register"
                  ? "Create Account & Continue"
                  : "Sign in"}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
