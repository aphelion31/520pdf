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

export async function runTool(
  toolId: string,
  files: File[],
  options: Record<string, string | boolean | number | undefined> = {},
): Promise<ToolResponse> {
  const form = new FormData();
  for (const f of files) {
    form.append("files", f, f.name);
  }
  for (const [key, val] of Object.entries(options)) {
    if (val === undefined) continue;
    form.append(key, typeof val === "boolean" ? String(val) : String(val));
  }
  const res = await fetch(`/api/tools/${encodeURIComponent(toolId)}`, {
    method: "POST",
    body: form,
  });
  if (!res.ok) {
    let msg = `Processing failed (${res.status})`;
    try {
      const err = (await res.json()) as { error?: string };
      if (err.error) msg = err.error;
    } catch {
      // keep HTTP message
    }
    throw new Error(msg);
  }
  const blob = await res.blob();
  if (!blob.size) throw new Error("Server returned an empty output");
  return {
    blob,
    filename: parseFilename(res.headers.get("Content-Disposition"), `520pdf-${toolId}.pdf`),
    contentType: res.headers.get("Content-Type") || blob.type || "application/octet-stream",
    compressStatus: res.headers.get("x-compress-status"),
    compressDpi: res.headers.get("x-compress-dpi"),
    compressQuality: res.headers.get("x-compress-quality"),
  };
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
