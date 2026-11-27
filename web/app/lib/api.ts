export interface ToolField {
  name: string;
  label?: string;
  type?: string;
  value?: string | boolean | number;
  min?: number;
  max?: number;
  step?: number;
  help?: string;
  description?: string;
  required?: boolean;
  options?: Array<string | { label: string; value: string }>;
}

export interface ApiTool {
  id: string;
  name: string;
  description: string;
  category: string;
  route: string;
  accept: string;
  multiple: boolean;
  fields: ToolField[];
}

export interface ToolResponse {
  blob: Blob;
  filename: string;
  contentType: string;
  compressStatus: string | null;
  compressDpi: string | null;
  compressQuality: string | null;
}

export interface ToolJobProgress {
  jobId: string;
  toolId: string;
  percent: number;
  stage: string;
  elapsedMs: number;
  done: boolean;
  cancel?: () => void;
}

let activeProgress: ToolJobProgress | null = null;
const progressListeners = new Set<() => void>();
let progressClearTimer: number | null = null;

function emitProgress(next: ToolJobProgress | null) {
  activeProgress = next;
  for (const listener of progressListeners) {
    listener();
  }
}

export function subscribeToolProgress(listener: () => void): () => void {
  progressListeners.add(listener);
  return () => {
    progressListeners.delete(listener);
  };
}

export function getActiveToolProgress(): ToolJobProgress | null {
  return activeProgress;
}

function makeJobId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID().replace(/-/g, "").slice(0, 20);
  }
  return `job_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function parseFilename(disposition: string | null, fallback: string): string {
  if (!disposition) return fallback;
  const utf8 = /filename\*=UTF-8''([^;]+)/i.exec(disposition);
  if (utf8) {
    try {
      return decodeURIComponent(utf8[1]).split(/[\\/]/).pop() || fallback;
    } catch {
      // fall through to plain header
    }
  }
  const plain = /filename="([^"]+)"|filename=([^;]+)/i.exec(disposition);
  if (plain) {
    return (plain[1] || plain[2]).trim().split(/[\\/]/).pop() || fallback;
  }
  return fallback;
}

export async function fetchCatalog(): Promise<ApiTool[]> {
  const res = await fetch("/api/tools", { headers: { Accept: "application/json" } });
  if (!res.ok) throw new Error(`Catalog unavailable (${res.status})`);
  const data = (await res.json()) as { tools?: ApiTool[] };
  return data.tools ?? [];
}

export interface RunToolOptions {
  bearerToken?: string;
  onProgress?: (progress: ToolJobProgress) => void;
}

export async function runTool(
  toolId: string,
  files: File[],
  options: Record<string, string | boolean | number | undefined> = {},
  extra: RunToolOptions = {},
): Promise<ToolResponse> {
  const form = new FormData();
  for (const f of files) {
    form.append("files", f, f.name);
  }
  for (const [key, val] of Object.entries(options)) {
    if (val === undefined) continue;
    form.append(key, typeof val === "boolean" ? String(val) : String(val));
  }

  if (progressClearTimer !== null) {
    window.clearTimeout(progressClearTimer);
    progressClearTimer = null;
  }

  const jobId = makeJobId();
  const startedAt = Date.now();
  const endpoint = extra.bearerToken
    ? `/api/v1/tools/${encodeURIComponent(toolId)}`
    : `/api/tools/${encodeURIComponent(toolId)}`;

  return new Promise<ToolResponse>((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", endpoint, true);
    xhr.responseType = "blob";
    xhr.setRequestHeader("X-Job-Id", jobId);
    if (extra.bearerToken) {
      xhr.setRequestHeader("Authorization", `Bearer ${extra.bearerToken}`);
    }

    let displayedPct = 4;
    let serverFloorPct = 24;
    let stage = files.length > 0 ? "Uploading files…" : "Starting…";
    let inServerPhase = false;
    let settled = false;
    let pollTimer: number | null = null;
    let tickTimer: number | null = null;
    let pollInFlight = false;

    const push = (done = false) => {
      const next: ToolJobProgress = {
        jobId,
        toolId,
        percent: displayedPct,
        stage,
        elapsedMs: Date.now() - startedAt,
        done,
        cancel: done ? undefined : () => xhr.abort(),
      };
      emitProgress(next);
      extra.onProgress?.(next);
    };

    const cleanupTimers = () => {
      if (pollTimer !== null) {
        window.clearInterval(pollTimer);
        pollTimer = null;
      }
      if (tickTimer !== null) {
        window.clearInterval(tickTimer);
        tickTimer = null;
      }
    };

    const pollServer = async () => {
      if (settled || pollInFlight || !inServerPhase) return;
      pollInFlight = true;
      try {
        const res = await fetch(`/api/jobs/${encodeURIComponent(jobId)}`, {
          headers: { Accept: "application/json" },
        });
        if (!res.ok || settled) return;
        const data = (await res.json()) as {
          percent?: number;
          stage?: string;
          done?: boolean;
        };
        if (typeof data.percent === "number" && data.percent > serverFloorPct) {
          serverFloorPct = Math.min(95, data.percent);
          if (serverFloorPct > displayedPct) {
            displayedPct = serverFloorPct;
          }
        }
        if (data.stage && data.stage !== "Starting…") {
          stage = data.stage;
        }
        push(false);
      } catch {
        // Ignore transient poll errors while request runs
      } finally {
        pollInFlight = false;
      }
    };

    const startServerPhase = () => {
      if (inServerPhase || settled) return;
      inServerPhase = true;
      if (displayedPct < 24) displayedPct = 24;
      if (stage.startsWith("Uploading")) {
        stage = "Processing…";
      }
      push(false);
      void pollServer();
      pollTimer = window.setInterval(() => {
        void pollServer();
      }, 220);
    };

    tickTimer = window.setInterval(() => {
      if (settled) return;
      if (inServerPhase) {
        const ceiling = Math.min(92, Math.max(serverFloorPct + 14, 88));
        if (displayedPct < serverFloorPct) {
          displayedPct = serverFloorPct;
        } else if (displayedPct < ceiling) {
          displayedPct = Math.min(ceiling, displayedPct + (ceiling - displayedPct) * 0.045);
        }
      }
      push(false);
    }, 120);

    xhr.upload.onprogress = (ev) => {
      if (settled || !ev.lengthComputable || ev.total <= 0) return;
      const ratio = Math.min(1, Math.max(0, ev.loaded / ev.total));
      const mapped = 4 + ratio * 20;
      if (mapped > displayedPct) displayedPct = mapped;
      stage = `Uploading files… (${Math.round(ratio * 100)}%)`;
      push(false);
    };

    xhr.upload.onload = () => {
      startServerPhase();
    };

    xhr.onreadystatechange = () => {
      if (xhr.readyState >= XMLHttpRequest.HEADERS_RECEIVED && !settled) {
        if (!inServerPhase) startServerPhase();
      }
    };

    xhr.onprogress = (ev) => {
      if (settled || !ev.lengthComputable || ev.total <= 0) return;
      const ratio = Math.min(1, Math.max(0, ev.loaded / ev.total));
      const mapped = 94 + ratio * 5;
      if (mapped > displayedPct) displayedPct = mapped;
      stage = "Downloading output…";
      push(false);
    };

    xhr.onabort = () => {
      settled = true;
      cleanupTimers();
      emitProgress(null);
      reject(new Error("Cancelled"));
    };

    xhr.onerror = () => {
      settled = true;
      cleanupTimers();
      emitProgress(null);
      reject(new Error("Network request failed"));
    };

    xhr.onload = async () => {
      settled = true;
      cleanupTimers();
      const blob = xhr.response instanceof Blob ? xhr.response : new Blob([]);
      if (xhr.status < 200 || xhr.status >= 300) {
        emitProgress(null);
        let msg = `Processing failed (${xhr.status})`;
        try {
          const text = await blob.text();
          const err = JSON.parse(text) as { error?: string };
          if (err.error) msg = err.error;
        } catch {
          // keep HTTP status fallback
        }
        reject(new Error(msg));
        return;
      }
      if (!blob.size) {
        emitProgress(null);
        reject(new Error("Server returned an empty output"));
        return;
      }
      displayedPct = 100;
      stage = "Complete";
      push(true);
      progressClearTimer = window.setTimeout(() => {
        if (activeProgress?.jobId === jobId) {
          emitProgress(null);
        }
        progressClearTimer = null;
      }, 450);
      resolve({
        blob,
        filename: parseFilename(
          xhr.getResponseHeader("Content-Disposition"),
          `520pdf-${toolId}.pdf`,
        ),
        contentType:
          xhr.getResponseHeader("Content-Type") || blob.type || "application/octet-stream",
        compressStatus: xhr.getResponseHeader("x-compress-status"),
        compressDpi: xhr.getResponseHeader("x-compress-dpi"),
        compressQuality: xhr.getResponseHeader("x-compress-quality"),
      });
    };

    push(false);
    if (files.length === 0) {
      startServerPhase();
    }
    xhr.send(form);
  });
}

export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
