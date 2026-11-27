import React, { useEffect, useMemo, useState } from "react";
import { Link } from "react-router";
import {
  BookOpen,
  Check,
  Copy,
  Eye,
  EyeOff,
  Globe,
  KeyRound,
  Loader2,
  Lock,
  Plus,
  ShieldCheck,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolProgressBar, useActiveToolProgress } from "~/components/ToolProgressBar";
import { fetchCatalog, runTool, type ApiTool, type ToolResponse } from "~/lib/api";

interface AuthStatus {
  auth_enabled: boolean;
  authenticated: boolean;
  username: string | null;
  algorithm: string;
  registration_enabled?: boolean;
}

interface TokenMeta {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  created_at: string;
  expires_at: number | null;
  last_used_at: string | null;
  usage_count: number;
  revoked: boolean;
  revealable?: boolean;
}

export default function ApiKeysPage() {
  const [auth, setAuth] = useState<AuthStatus | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [tokens, setTokens] = useState<TokenMeta[]>([]);
  const [tools, setTools] = useState<ApiTool[]>([]);
  const activeJob = useActiveToolProgress();

  const [name, setName] = useState("");
  const [expiryDays, setExpiryDays] = useState("90");
  const [scopeMode, setScopeMode] = useState<"all" | "custom">("all");
  const [selectedScopes, setSelectedScopes] = useState<string[]>(["compress", "merge"]);
  const [creating, setCreating] = useState(false);
  const [mintedSecret, setMintedSecret] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState(false);
  const [copiedCurl, setCopiedCurl] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revealedTokens, setRevealedTokens] = useState<Record<string, string>>({});
  const [copiedTokenId, setCopiedTokenId] = useState<string | null>(null);
  const [revealTarget, setRevealTarget] = useState<TokenMeta | null>(null);
  const [revealPassword, setRevealPassword] = useState("");
  const [revealBusy, setRevealBusy] = useState(false);
  const [revealError, setRevealError] = useState<string | null>(null);

  // CORS Origins management state
  const [corsOrigins, setCorsOrigins] = useState<string[]>(["*"]);
  const [corsInput, setCorsInput] = useState("");
  const [corsBusy, setCorsBusy] = useState(false);
  const [corsError, setCorsError] = useState<string | null>(null);

  // Interactive API Request Builder state
  const [builderTool, setBuilderTool] = useState("compress");
  const [builderValues, setBuilderValues] = useState<Record<string, string>>({
    target_kb: "250",
    dpi: "120",
    quality: "70",
    quality_preset: "high",
    remove_metadata: "true",
  });
  const [testFiles, setTestFiles] = useState<File[]>([]);
  const [testBusy, setTestBusy] = useState(false);
  const [testError, setTestError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<ToolResponse | null>(null);

  const loadState = async () => {
    setAuthLoading(true);
    try {
      const meRes = await fetch("/api/auth/me", { headers: { Accept: "application/json" } });
      const me = (await meRes.json()) as AuthStatus;
      setAuth(me);
      if (me.authenticated) {
        const [tRes, cRes] = await Promise.all([
          fetch("/api/tokens", { headers: { Accept: "application/json" } }),
          fetch("/api/cors", { headers: { Accept: "application/json" } }),
        ]);
        if (tRes.ok) {
          const data = (await tRes.json()) as { tokens?: TokenMeta[] };
          setTokens(data.tokens ?? []);
        }
        if (cRes.ok) {
          const corsData = (await cRes.json()) as { origins?: string[] };
          setCorsOrigins(corsData.origins ?? ["*"]);
        }
      } else {
        setTokens([]);
      }
    } catch {
      setAuth(null);
    } finally {
      setAuthLoading(false);
    }
  };

  useEffect(() => {
    void loadState();
    fetchCatalog()
      .then(setTools)
      .catch(() => setTools([]));
  }, []);

  const activeTool = useMemo(
    () => tools.find((t) => t.id === builderTool) ?? null,
    [tools, builderTool],
  );

  useEffect(() => {
    if (!activeTool) return;
    const defaults: Record<string, string> = {};
    for (const f of activeTool.fields) {
      defaults[f.name] = String(f.value ?? "");
    }
    setBuilderValues(defaults);
    setTestResult(null);
    setTestError(null);
  }, [activeTool?.id]);

  const handleCreateToken = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!name.trim()) {
      setError("Enter a label for this API key.");
      return;
    }
    const scopes = scopeMode === "all" ? ["*"] : selectedScopes;
    if (scopeMode === "custom" && scopes.length === 0) {
      setError("Select at least one tool scope.");
      return;
    }
    setCreating(true);
    try {
      const body: Record<string, unknown> = {
        name: name.trim(),
        scopes,
      };
      if (expiryDays !== "never") {
        body.expires_in_days = Number(expiryDays);
      }
      const res = await fetch("/api/tokens", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = (await res.json()) as { token?: string; error?: string };
      if (!res.ok || !data.token) {
        throw new Error(data.error || "Failed to create API token");
      }
      setMintedSecret(data.token);
      setName("");
      await loadState();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create API token");
    } finally {
      setCreating(false);
    }
  };

  useEffect(() => {
    if (!revealTarget) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !revealBusy) {
        setRevealTarget(null);
        setRevealPassword("");
        setRevealError(null);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [revealTarget, revealBusy]);

  const handleRevoke = async (id: string) => {
    setError(null);
    const res = await fetch(`/api/tokens/${encodeURIComponent(id)}`, { method: "DELETE" });
    if (res.ok) {
      const removedSecret = revealedTokens[id];
      setRevealedTokens((prev) => {
        if (!(id in prev)) return prev;
        const next = { ...prev };
        delete next[id];
        return next;
      });
      if (removedSecret && mintedSecret === removedSecret) {
        setMintedSecret(null);
      }
      await loadState();
    } else {
      const d = (await res.json()) as { error?: string };
      setError(d.error || "Could not revoke token");
    }
  };

  const saveCorsOrigins = async (nextOrigins: string[]) => {
    setCorsBusy(true);
    setCorsError(null);
    try {
      const res = await fetch("/api/cors", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ origins: nextOrigins }),
      });
      const data = (await res.json().catch(() => ({}))) as {
        origins?: string[];
        error?: string;
      };
      if (!res.ok || !data.origins) {
        throw new Error(data.error || "Could not update CORS origins");
      }
      setCorsOrigins(data.origins);
      return true;
    } catch (err) {
      setCorsError(err instanceof Error ? err.message : "Could not update CORS origins");
      return false;
    } finally {
      setCorsBusy(false);
    }
  };

  const handleAddCorsOrigin = async (e: React.FormEvent) => {
    e.preventDefault();
    const candidate = corsInput.trim();
    if (!candidate) {
      setCorsError("Enter an origin URL like https://app.example.com or *");
      return;
    }
    const ok = await saveCorsOrigins([...corsOrigins, candidate]);
    if (ok) setCorsInput("");
  };

  const handleRemoveCorsOrigin = async (origin: string) => {
    await saveCorsOrigins(corsOrigins.filter((item) => item !== origin));
  };

  const handleToggleWildcardCors = async () => {
    const hasWildcard = corsOrigins.includes("*");
    const next = hasWildcard
      ? corsOrigins.filter((o) => o !== "*")
      : ["*", ...corsOrigins.filter((o) => o !== "*")];
    await saveCorsOrigins(next);
  };

  const openRevealModal = (tok: TokenMeta) => {
    setRevealTarget(tok);
    setRevealPassword("");
    setRevealError(null);
  };

  const closeRevealModal = () => {
    if (revealBusy) return;
    setRevealTarget(null);
    setRevealPassword("");
    setRevealError(null);
  };

  const hideRevealedToken = (id: string) => {
    setRevealedTokens((prev) => {
      if (!(id in prev)) return prev;
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  const handleRevealSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!revealTarget) return;
    if (!revealPassword) {
      setRevealError("Enter your login password.");
      return;
    }
    setRevealBusy(true);
    setRevealError(null);
    try {
      const res = await fetch(`/api/tokens/${encodeURIComponent(revealTarget.id)}/reveal`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password: revealPassword }),
      });
      const data = (await res.json().catch(() => ({}))) as { token?: string; error?: string };
      if (!res.ok || !data.token) {
        throw new Error(data.error || "Incorrect password");
      }
      const tokenId = revealTarget.id;
      setRevealedTokens((prev) => ({ ...prev, [tokenId]: data.token! }));
      setRevealPassword("");
      setRevealTarget(null);
    } catch (err) {
      setRevealError(err instanceof Error ? err.message : "Could not verify password");
    } finally {
      setRevealBusy(false);
    }
  };

  const tokenDisplay = mintedSecret || "520pdf_k_<your_api_key>";
  const originUrl = typeof window !== "undefined" ? window.location.origin : "http://127.0.0.1:8080";

  const curlSnippet = useMemo(() => {
    const lines = [
      `curl -fSL -D - -o output_${builderTool} \\`,
      `  -H "Authorization: Bearer ${tokenDisplay}" \\`,
      `  -F "files=@sample${(activeTool?.accept || ".pdf").split(",")[0]}" \\`,
    ];
    for (const [k, v] of Object.entries(builderValues)) {
      if (v.trim() !== "") {
        lines.push(`  --form-string "${k}=${v}" \\`);
      }
    }
    lines.push(`  ${originUrl}/api/v1/tools/${builderTool}`);
    return lines.join("\n");
  }, [activeTool, builderTool, builderValues, originUrl, tokenDisplay]);

  const handleLiveTest = async () => {
    if (!activeTool) return;
    setTestBusy(true);
    setTestError(null);
    setTestResult(null);
    try {
      const cleanOpts: Record<string, string> = {};
      for (const [k, v] of Object.entries(builderValues)) {
        if (v.trim() !== "") cleanOpts[k] = v;
      }
      const res = await runTool(
        builderTool,
        testFiles,
        cleanOpts,
        mintedSecret ? { bearerToken: mintedSecret } : undefined,
      );
      setTestResult(res);
    } catch (e) {
      setTestError(e instanceof Error ? e.message : "Test call failed");
    } finally {
      setTestBusy(false);
    }
  };

  if (authLoading) {
    return (
      <div className="mx-auto max-w-7xl px-4 py-16 text-center text-sm text-[var(--ink-muted)]">
        Checking Argon2id workspace session…
      </div>
    );
  }

  if (!auth?.authenticated) {
    return (
      <div className="mx-auto max-w-xl px-4 py-14 sm:px-6">
        <div className="surface-card cm-lift space-y-5 rounded-2xl p-8 text-center">
          <span className="mx-auto grid h-12 w-12 place-items-center rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]">
            <Lock className="h-5 w-5" />
          </span>
          <h1 className="text-2xl font-semibold tracking-tight text-[var(--ink-primary)]">
            Sign in to access API Keys
          </h1>
          <p className="text-sm leading-relaxed text-[var(--ink-secondary)]">
            Creating, viewing, and revoking scoped <code className="font-mono">520pdf_k_...</code> API tokens requires an authenticated Argon2id account session.
          </p>
          <div className="flex flex-wrap justify-center gap-2.5 pt-2">
            <Link
              to="/login?next=/api-keys"
              className="cm-btn inline-flex items-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-2.5 text-sm font-semibold text-white hover:bg-[var(--accent-hover)]"
            >
              <KeyRound className="h-4 w-4" /> Sign in or Create Account
            </Link>
            <Link
              to="/docs/api-tokens"
              className="cm-btn inline-flex items-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-5 py-2.5 text-sm font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
            >
              <BookOpen className="h-4 w-4 text-[var(--ink-muted)]" /> Read Fumadocs API Guide
            </Link>
          </div>
        </div>
      </div>
    );
  }

  const allowAllCors = corsOrigins.includes("*");
  const testPct =
    activeJob?.toolId === builderTool ? Math.round(activeJob.percent) : null;

  return (
    <div className="cm-lift mx-auto max-w-7xl space-y-6 px-4 py-8 sm:px-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <span className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1 text-xs font-medium text-[var(--ink-secondary)]">
            <ShieldCheck className="h-3.5 w-3.5 text-[var(--state-ok-fg)]" /> Signed in as {auth.username}
          </span>
          <h1 className="mt-2 text-2xl font-semibold tracking-tight text-[var(--ink-primary)] sm:text-3xl">
            API Keys &amp; Endpoint Customizer
          </h1>
          <p className="mt-1 text-sm text-[var(--ink-secondary)]">
            Mint scoped API tokens to call any of the 36 PDF, Office, OCR, and media tools with configurable CORS.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Link
            to="/docs"
            className="cm-btn inline-flex items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3.5 py-2 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
          >
            <BookOpen className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> API Docs (Fumadocs)
          </Link>
        </div>
      </div>

      {mintedSecret && (
        <div
          data-testid="minted-token-banner"
          className="cm-lift rounded-xl border border-[var(--state-ok-border)] bg-[var(--state-ok-bg)] p-4"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-xs font-semibold text-[var(--state-ok-fg)]">
                Save your new API secret — or view it anytime with your login password
              </p>
              <code className="mt-1 block break-all font-mono text-sm font-semibold text-[var(--ink-primary)]">
                {mintedSecret}
              </code>
            </div>
            <button
              type="button"
              onClick={() => {
                navigator.clipboard.writeText(mintedSecret);
                setCopiedKey(true);
                setTimeout(() => setCopiedKey(false), 1800);
              }}
              className="cm-btn inline-flex items-center gap-1.5 rounded-lg border border-[var(--state-ok-border)] bg-[var(--bg-surface)] px-3 py-1.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
            >
              {copiedKey ? <Check className="h-3.5 w-3.5 text-[var(--state-ok-fg)]" /> : <Copy className="h-3.5 w-3.5 text-[var(--ink-muted)]" />}
              {copiedKey ? "Copied" : "Copy token"}
            </button>
          </div>
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-[0.95fr_1.05fr]">
        <div className="space-y-6">
          <form onSubmit={handleCreateToken} className="surface-card space-y-4 rounded-2xl p-6">
            <h2 className="text-base font-semibold tracking-tight text-[var(--ink-primary)]">
              Create API Token
            </h2>
            <div>
              <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                Token label
              </label>
              <input
                type="text"
                required
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. CI Pipeline / Invoice Batch"
                className="cm-input mt-1.5 h-9 px-3 text-sm"
              />
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div>
                <label className="mb-1.5 block text-xs font-medium text-[var(--ink-secondary)]">
                  Expiration
                </label>
                <BasecoatSelect
                  ariaLabel="Token expiration"
                  value={expiryDays}
                  onChange={setExpiryDays}
                  options={[
                    { value: "7", label: "7 days" },
                    { value: "30", label: "30 days" },
                    { value: "90", label: "90 days" },
                    { value: "365", label: "1 year" },
                    { value: "never", label: "Never expires" },
                  ]}
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-medium text-[var(--ink-secondary)]">
                  Tool scope
                </label>
                <BasecoatSelect
                  ariaLabel="Token tool scope mode"
                  value={scopeMode}
                  onChange={(v) => setScopeMode(v as "all" | "custom")}
                  options={[
                    { value: "all", label: "All 36 tools (*)" },
                    { value: "custom", label: "Specific tools only" },
                  ]}
                />
              </div>
            </div>

            {scopeMode === "custom" && (
              <div>
                <p className="mb-2 text-xs font-medium text-[var(--ink-secondary)]">
                  Permitted tools ({selectedScopes.length} selected)
                </p>
                <div className="grid max-h-40 grid-cols-2 gap-1.5 overflow-y-auto rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3 text-xs">
                  {tools.map((t) => {
                    const checked = selectedScopes.includes(t.id);
                    return (
                      <label key={t.id} className="flex cursor-pointer items-center gap-2">
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={(e) =>
                            setSelectedScopes((prev) =>
                              e.target.checked
                                ? [...prev, t.id]
                                : prev.filter((item) => item !== t.id),
                            )
                          }
                          className="accent-[var(--accent)]"
                        />
                        <span className="truncate font-mono">{t.id}</span>
                      </label>
                    );
                  })}
                </div>
              </div>
            )}

            {error && (
              <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs text-[var(--state-bad-fg)]">
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={creating}
              className="cm-btn inline-flex h-10 w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-4 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
            >
              {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
              {creating ? "Minting key…" : "Generate API Key"}
            </button>
          </form>

          <div className="surface-card space-y-3 rounded-2xl p-6">
            <h2 className="text-base font-semibold tracking-tight text-[var(--ink-primary)]">
              Your Tokens ({tokens.length})
            </h2>
            {tokens.length === 0 ? (
              <p className="rounded-xl border border-dashed border-[var(--border-subtle)] p-4 text-center text-xs text-[var(--ink-muted)]">
                No API tokens created yet.
              </p>
            ) : (
              <div className="space-y-2.5">
                {tokens.map((tok) => {
                  const revealedValue = revealedTokens[tok.id];
                  return (
                    <div
                      key={tok.id}
                      className={`rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3.5 ${
                        tok.revoked ? "opacity-55" : ""
                      }`}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div>
                          <p className="text-sm font-semibold text-[var(--ink-primary)]">
                            {tok.name}{" "}
                            {tok.revoked && (
                              <span className="ml-1 rounded bg-red-500/15 px-1.5 py-0.5 text-[10px] font-semibold text-[var(--state-bad-fg)] uppercase">
                                Revoked
                              </span>
                            )}
                          </p>
                          <p className="mt-0.5 font-mono text-xs text-[var(--ink-muted)] tabular-nums">
                            {tok.prefix} · {tok.usage_count} hits
                          </p>
                        </div>
                        {!tok.revoked && (
                          <div className="flex shrink-0 items-center gap-1.5">
                            {tok.revealable ? (
                              <button
                                type="button"
                                onClick={() =>
                                  revealedValue ? hideRevealedToken(tok.id) : openRevealModal(tok)
                                }
                                aria-label={revealedValue ? `Hide ${tok.name}` : `View ${tok.name}`}
                                className="cm-btn inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2.5 py-1 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
                              >
                                {revealedValue ? (
                                  <>
                                    <EyeOff className="h-3 w-3" /> Hide
                                  </>
                                ) : (
                                  <>
                                    <Eye className="h-3 w-3" /> View
                                  </>
                                )}
                              </button>
                            ) : (
                              <span
                                title="Legacy key created before encrypted view support"
                                className="rounded-md border border-[var(--border-subtle)] px-2 py-0.5 text-[10px] font-medium text-[var(--ink-muted)]"
                              >
                                Legacy key
                              </span>
                            )}
                            <button
                              type="button"
                              onClick={() => handleRevoke(tok.id)}
                              aria-label={`Revoke ${tok.name}`}
                              className="cm-btn inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] px-2.5 py-1 text-xs font-medium text-[var(--ink-secondary)] hover:border-red-500/50 hover:text-[var(--state-bad-fg)]"
                            >
                              <Trash2 className="h-3 w-3" /> Revoke
                            </button>
                          </div>
                        )}
                      </div>
                      {revealedValue && (
                        <div
                          data-testid={`revealed-token-${tok.id}`}
                          className="cm-lift mt-2.5 rounded-lg border border-[var(--state-ok-border)] bg-[var(--state-ok-bg)] p-2.5"
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <code className="min-w-0 flex-1 break-all font-mono text-[11px] font-semibold text-[var(--ink-primary)]">
                              {revealedValue}
                            </code>
                            <div className="flex shrink-0 items-center gap-1.5">
                              <button
                                type="button"
                                onClick={() => {
                                  navigator.clipboard.writeText(revealedValue);
                                  setCopiedTokenId(tok.id);
                                  setTimeout(
                                    () => setCopiedTokenId((curr) => (curr === tok.id ? null : curr)),
                                    1800,
                                  );
                                }}
                                className="cm-btn inline-flex items-center gap-1 rounded-md border border-[var(--state-ok-border)] bg-[var(--bg-surface)] px-2 py-1 text-[11px] font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
                              >
                                {copiedTokenId === tok.id ? (
                                  <Check className="h-3 w-3 text-[var(--state-ok-fg)]" />
                                ) : (
                                  <Copy className="h-3 w-3 text-[var(--ink-muted)]" />
                                )}
                                {copiedTokenId === tok.id ? "Copied" : "Copy"}
                              </button>
                              <button
                                type="button"
                                onClick={() => setMintedSecret(revealedValue)}
                                className="cm-btn rounded-md border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2 py-1 text-[11px] font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
                              >
                                Use in Builder
                              </button>
                            </div>
                          </div>
                        </div>
                      )}
                      <div className="mt-2 flex flex-wrap gap-1">
                        {tok.scopes.map((s) => (
                          <span
                            key={s}
                            className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2 py-0.5 font-mono text-[10px] text-[var(--ink-secondary)]"
                          >
                            {s === "*" ? "all-tools (*)" : s}
                          </span>
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <div className="surface-card space-y-3 rounded-2xl p-6">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <Globe className="h-4 w-4 text-[var(--ink-muted)]" />
                <h2 className="text-base font-semibold tracking-tight text-[var(--ink-primary)]">
                  API CORS Origins
                </h2>
              </div>
              <span
                className={`rounded-full border px-2.5 py-0.5 text-[11px] font-medium ${
                  allowAllCors
                    ? "border-[var(--state-ok-border)] bg-[var(--state-ok-bg)] text-[var(--state-ok-fg)]"
                    : "border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-secondary)]"
                }`}
              >
                {allowAllCors ? "All origins (*)" : "Restricted"}
              </span>
            </div>
            <p className="text-xs leading-relaxed text-[var(--ink-secondary)]">
              Browser origins allowed to call <code className="font-mono">/api/*</code>. Defaults to{" "}
              <code className="font-mono">*</code>; add custom origins to grant credentialed access or restrict callers.
            </p>

            <form onSubmit={handleAddCorsOrigin} className="flex gap-2">
              <input
                type="text"
                name="cors_origin"
                value={corsInput}
                onChange={(e) => setCorsInput(e.target.value)}
                placeholder="https://app.example.com"
                aria-label="Add allowed CORS origin"
                className="cm-input h-9 flex-1 px-3 font-mono text-xs"
              />
              <button
                type="submit"
                disabled={corsBusy}
                className="cm-btn inline-flex h-9 shrink-0 items-center gap-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-subtle)] px-3 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)] disabled:opacity-50"
              >
                <Plus className="h-3.5 w-3.5" /> Add Origin
              </button>
            </form>

            {corsError && (
              <p className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-[var(--state-bad-fg)]">
                {corsError}
              </p>
            )}

            <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
              <button
                type="button"
                disabled={corsBusy}
                onClick={handleToggleWildcardCors}
                className={`cm-btn inline-flex items-center gap-1 rounded-md border px-2.5 py-1 font-mono text-xs font-medium disabled:opacity-50 ${
                  allowAllCors
                    ? "border-[var(--ink-primary)] bg-[var(--ink-primary)] text-[var(--bg-surface)]"
                    : "border-[var(--border-subtle)] bg-[var(--bg-page)] text-[var(--ink-secondary)] hover:border-[var(--border-strong)]"
                }`}
              >
                * {allowAllCors ? "All enabled" : "Enable *"}
              </button>

              {corsOrigins
                .filter((o) => o !== "*")
                .map((origin) => (
                  <span
                    key={origin}
                    className="inline-flex items-center gap-1.5 rounded-md border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1 font-mono text-xs text-[var(--ink-primary)]"
                  >
                    <span>{origin}</span>
                    <button
                      type="button"
                      disabled={corsBusy}
                      onClick={() => handleRemoveCorsOrigin(origin)}
                      aria-label={`Remove origin ${origin}`}
                      className="rounded p-0.5 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--state-bad-fg)]"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </span>
                ))}
            </div>
          </div>
        </div>

        <div className="surface-card space-y-4 rounded-2xl p-6">
          <div className="flex items-center justify-between gap-2">
            <h2 className="flex items-center gap-2 text-base font-semibold tracking-tight text-[var(--ink-primary)]">
              <Terminal className="h-4 w-4 text-[var(--ink-muted)]" /> Customize Tool Request
            </h2>
            <span className="font-mono text-[11px] text-[var(--ink-muted)]">
              POST /api/v1/tools/{builderTool}
            </span>
          </div>

          <div>
            <label className="mb-1.5 block text-xs font-medium text-[var(--ink-secondary)]">
              Select endpoint (36 tools)
            </label>
            <BasecoatSelect
              ariaLabel="API request builder tool"
              value={builderTool}
              onChange={setBuilderTool}
              options={tools.map((t) => ({ value: t.id, label: `${t.name} (${t.id})` }))}
            />
          </div>

          {(activeTool?.fields?.length ?? 0) > 0 && (
            <div className="grid gap-3 sm:grid-cols-2">
              {activeTool!.fields.map((f) => {
                if (f.type === "select" && f.options?.length) {
                  const opts = f.options.map((o) =>
                    typeof o === "string" ? { value: o, label: o } : o,
                  );
                  return (
                    <div key={f.name}>
                      <label className="mb-1 block text-xs font-medium text-[var(--ink-secondary)]">
                        {f.label || f.name}
                      </label>
                      <BasecoatSelect
                        ariaLabel={f.label || f.name}
                        value={builderValues[f.name] ?? ""}
                        onChange={(v) => setBuilderValues((prev) => ({ ...prev, [f.name]: v }))}
                        options={opts}
                      />
                    </div>
                  );
                }
                if (f.type === "checkbox") {
                  return (
                    <label
                      key={f.name}
                      className="flex cursor-pointer items-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-2 text-xs sm:col-span-2"
                    >
                      <input
                        type="checkbox"
                        checked={builderValues[f.name] === "true"}
                        onChange={(e) =>
                          setBuilderValues((prev) => ({
                            ...prev,
                            [f.name]: String(e.target.checked),
                          }))
                        }
                        className="accent-[var(--accent)]"
                      />
                      <span>{f.label || f.name}</span>
                    </label>
                  );
                }
                return (
                  <div key={f.name}>
                    <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                      {f.label || f.name}
                    </label>
                    <input
                      type={f.type === "number" ? "number" : "text"}
                      min={f.min}
                      max={f.max}
                      value={builderValues[f.name] ?? ""}
                      onChange={(e) =>
                        setBuilderValues((prev) => ({ ...prev, [f.name]: e.target.value }))
                      }
                      className="cm-input mt-1 h-9 px-3 font-mono text-xs"
                    />
                  </div>
                );
              })}
            </div>
          )}

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <span className="text-xs font-medium text-[var(--ink-secondary)]">Generated cURL</span>
              <button
                type="button"
                onClick={() => {
                  navigator.clipboard.writeText(curlSnippet);
                  setCopiedCurl(true);
                  setTimeout(() => setCopiedCurl(false), 1800);
                }}
                className="cm-btn inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] px-2.5 py-1 text-[11px] font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
              >
                {copiedCurl ? <Check className="h-3 w-3 text-[var(--state-ok-fg)]" /> : <Copy className="h-3 w-3" />}
                {copiedCurl ? "Copied" : "Copy cURL"}
              </button>
            </div>
            <pre className="overflow-x-auto rounded-xl border border-[var(--border-subtle)] bg-neutral-950 p-4 font-mono text-xs leading-relaxed text-neutral-100">
              {curlSnippet}
            </pre>
          </div>

          <div className="space-y-3 border-t border-[var(--border-subtle)] pt-4">
            <label className="block text-xs font-medium text-[var(--ink-secondary)]">
              Test live call ({activeTool?.accept || ".pdf"})
            </label>
            <input
              type="file"
              accept={activeTool?.accept || ".pdf"}
              multiple={Boolean(activeTool?.multiple)}
              onChange={(e) => setTestFiles(Array.from(e.target.files ?? []))}
              className="block w-full text-xs text-[var(--ink-secondary)]"
            />
            {testError && (
              <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2 text-xs text-[var(--state-bad-fg)]">
                {testError}
              </p>
            )}
            {testBusy && <ToolProgressBar toolId={builderTool} />}
            <button
              type="button"
              disabled={testBusy}
              onClick={handleLiveTest}
              className="cm-btn inline-flex h-10 w-full items-center justify-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 text-xs font-semibold text-[var(--ink-primary)] hover:border-[var(--border-strong)] disabled:opacity-50"
            >
              {testBusy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
              {testBusy
                ? testPct !== null
                  ? `Executing API call… ${testPct}%`
                  : "Executing API call…"
                : `Test POST /api/v1/tools/${builderTool}`}
            </button>
            {testResult && (
              <ResultBanner
                blob={testResult.blob}
                filename={testResult.filename}
                originalBytes={testFiles.reduce((s, f) => s + f.size, 0)}
                note={
                  testResult.compressStatus
                    ? `${testResult.compressStatus} (${testResult.compressDpi || 0} dpi, q${testResult.compressQuality || 0})`
                    : undefined
                }
              />
            )}
          </div>
        </div>
      </div>

      {revealTarget && (
        <div
          onClick={(e) => {
            if (e.target === e.currentTarget) closeRevealModal();
          }}
          className="cm-dialog-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-xs"
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="reveal-token-title"
            className="surface-card cm-dialog-panel w-full max-w-md space-y-4 rounded-2xl p-6 shadow-2xl"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3">
                <span className="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]">
                  <Lock className="h-4 w-4" />
                </span>
                <div>
                  <h3 id="reveal-token-title" className="text-base font-semibold text-[var(--ink-primary)]">
                    Confirm password to view token
                  </h3>
                  <p className="mt-1 text-xs leading-relaxed text-[var(--ink-secondary)]">
                    Enter the login password for{" "}
                    <span className="font-semibold text-[var(--ink-primary)]">{auth.username}</span>{" "}
                    to reveal <span className="font-semibold">&ldquo;{revealTarget.name}&rdquo;</span>.
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={closeRevealModal}
                disabled={revealBusy}
                aria-label="Close modal"
                className="cm-btn rounded-lg p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)] disabled:opacity-50"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <form onSubmit={handleRevealSubmit} className="space-y-4">
              <div>
                <label
                  htmlFor="reveal_password_input"
                  className="block text-xs font-medium text-[var(--ink-secondary)]"
                >
                  Login password
                </label>
                <input
                  id="reveal_password_input"
                  type="password"
                  name="reveal_password"
                  autoFocus
                  autoComplete="current-password"
                  required
                  value={revealPassword}
                  onChange={(e) => setRevealPassword(e.target.value)}
                  placeholder="Enter your password"
                  className="cm-input mt-1.5 h-10 px-3.5 text-sm"
                />
              </div>

              {revealError && (
                <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs font-medium text-[var(--state-bad-fg)]">
                  {revealError}
                </p>
              )}

              <div className="flex items-center justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={closeRevealModal}
                  disabled={revealBusy}
                  className="cm-btn rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-2 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)] disabled:opacity-50"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={revealBusy}
                  className="cm-btn inline-flex items-center gap-1.5 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
                >
                  {revealBusy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Eye className="h-3.5 w-3.5" />}
                  {revealBusy ? "Verifying…" : "Reveal Token"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
