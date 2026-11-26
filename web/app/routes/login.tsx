import React, { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { KeyRound, Loader2, LogOut, ShieldCheck } from "lucide-react";

interface AuthStatus {
  auth_enabled: boolean;
  authenticated: boolean;
  username: string | null;
  algorithm: string;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = () => {
    fetch("/api/auth/me", { headers: { Accept: "application/json" } })
      .then((r) => r.json())
      .then((data: AuthStatus) => setStatus(data))
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
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
      });
      const body = (await res.json()) as { error?: string };
      if (!res.ok) throw new Error(body.error || "Sign in failed");
      loadStatus();
      navigate("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign in failed");
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
      <div className="surface-card space-y-6 rounded-3xl p-7 shadow-sm">
        <div className="text-center">
          <img src="/brand-assets/logo.svg" alt="520pdf" className="mx-auto h-9 w-auto dark:hidden" />
          <img
            src="/brand-assets/logo_dark.svg"
            alt="520pdf"
            className="mx-auto hidden h-9 w-auto dark:block"
          />
          <span className="mt-4 inline-flex items-center gap-1.5 rounded-full bg-[var(--accent-soft)] px-3 py-1 text-[11px] font-bold tracking-wider text-[var(--accent)] uppercase">
            <ShieldCheck className="h-3.5 w-3.5" /> Argon2id (PHC v=19) Auth
          </span>
          <h1 className="mt-3 text-2xl font-extrabold text-[var(--ink-primary)]">
            Workspace Access
          </h1>
          <p className="mt-1 text-xs text-[var(--ink-secondary)]">
            Passwords are verified strictly with Argon2id + HMAC-SHA256 HttpOnly sessions.
          </p>
        </div>

        {status?.authenticated ? (
          <div className="space-y-4 rounded-2xl border border-emerald-500/30 bg-emerald-500/[0.06] p-5 text-center">
            <p className="text-sm font-semibold text-[var(--ink-primary)]">
              Signed in as <span className="text-[var(--accent)]">{status.username}</span>
            </p>
            <div className="flex gap-2">
              <Link
                to="/"
                className="flex-1 rounded-xl bg-[var(--accent)] py-2.5 text-center text-xs font-semibold text-white hover:bg-[var(--accent-hover)]"
              >
                Open Workbench
              </Link>
              <button
                type="button"
                onClick={handleLogout}
                className="inline-flex items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-4 py-2.5 text-xs font-semibold"
              >
                <LogOut className="h-3.5 w-3.5" /> Sign out
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            {status && !status.auth_enabled && (
              <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3.5 text-xs leading-relaxed text-[var(--ink-secondary)]">
                Open-access mode active (matching zero-account iHatePDF default). To enable login,
                set <code className="font-mono font-semibold">AUTH_USER</code> and{" "}
                <code className="font-mono font-semibold">AUTH_PASSWORD_HASH</code> via{" "}
                <code className="font-mono font-semibold">pdf520 hash-password</code>.
              </div>
            )}

            <div>
              <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                Username
              </label>
              <input
                type="text"
                name="username"
                required
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="admin"
                className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                Password
              </label>
              <input
                type="password"
                name="password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••••••"
                className="mt-1.5 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2.5 text-sm"
              />
            </div>

            {error && (
              <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs text-red-600 dark:text-red-400">
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={busy}
              className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
            >
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <KeyRound className="h-4 w-4" />}
              {busy ? "Verifying Argon2id…" : "Sign in"}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
