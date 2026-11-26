import React, { createContext, useCallback, useContext, useMemo, useState } from "react";

export type VaultKind = "pdf" | "image" | "text" | "other";

export interface VaultFile {
  id: string;
  file: File;
  url: string;
  kind: VaultKind;
}

interface FileVaultValue {
  files: VaultFile[];
  addFiles: (files: File[]) => VaultFile[];
  replaceFile: (id: string, file: File) => void;
  removeFile: (id: string) => void;
  clear: () => void;
  match: (accept?: string) => VaultFile[];
  totalBytes: number;
}

const MAX_FILES = 30;
const MAX_TOTAL_BYTES = 49 * 1024 * 1024;
const FileVaultContext = createContext<FileVaultValue | null>(null);

function kindOf(file: File): VaultKind {
  if (file.type === "application/pdf" || /\.pdf$/i.test(file.name)) return "pdf";
  if (file.type.startsWith("image/") || /\.(png|jpe?g|webp|gif|bmp)$/i.test(file.name)) return "image";
  if (/\.(txt|tex|json|html?)$/i.test(file.name) || file.type.startsWith("text/")) return "text";
  return "other";
}

export function fileMatchesAccept(file: File, accept?: string): boolean {
  if (!accept || accept.trim() === "" || accept === "*") return true;
  const ext = `.${file.name.split(".").pop()?.toLowerCase() ?? ""}`;
  return accept
    .split(",")
    .map((t) => t.trim().toLowerCase())
    .some((rule) => {
      if (!rule) return false;
      if (rule === "*" || rule === "*/*") return true;
      if (rule.endsWith("/*")) return file.type.toLowerCase().startsWith(rule.slice(0, -1));
      if (rule.includes("/")) return file.type.toLowerCase() === rule;
      return ext === rule;
    });
}

let seq = 0;

export function FileVaultProvider({ children }: { children: React.ReactNode }) {
  const [files, setFiles] = useState<VaultFile[]>([]);

  const addFiles = useCallback((incoming: File[]) => {
    const valid = incoming.filter((f) => f.size > 0);
    const created: VaultFile[] = valid.map((file) => ({
      id: `vf_${Date.now()}_${seq++}`,
      file,
      url: URL.createObjectURL(file),
      kind: kindOf(file),
    }));
    setFiles((prev) => {
      const seen = new Set(prev.map((f) => `${f.file.name}:${f.file.size}:${f.file.lastModified}`));
      const deduped = created.filter((item) => {
        const key = `${item.file.name}:${item.file.size}:${item.file.lastModified}`;
        if (seen.has(key)) {
          URL.revokeObjectURL(item.url);
          return false;
        }
        seen.add(key);
        return true;
      });
      const next = [...prev, ...deduped];
      while (
        next.length > MAX_FILES ||
        next.reduce((sum, item) => sum + item.file.size, 0) > MAX_TOTAL_BYTES
      ) {
        const evicted = next.shift();
        if (evicted) URL.revokeObjectURL(evicted.url);
      }
      return next;
    });
    return created;
  }, []);

  const replaceFile = useCallback((id: string, file: File) => {
    setFiles((prev) =>
      prev.map((item) => {
        if (item.id !== id) return item;
        URL.revokeObjectURL(item.url);
        return {
          id: item.id,
          file,
          url: URL.createObjectURL(file),
          kind: kindOf(file),
        };
      }),
    );
  }, []);

  const removeFile = useCallback((id: string) => {
    setFiles((prev) => {
      const target = prev.find((item) => item.id === id);
      if (target) URL.revokeObjectURL(target.url);
      return prev.filter((item) => item.id !== id);
    });
  }, []);

  const clear = useCallback(() => {
    setFiles((prev) => {
      for (const item of prev) URL.revokeObjectURL(item.url);
      return [];
    });
  }, []);

  const match = useCallback(
    (accept?: string) => files.filter((item) => fileMatchesAccept(item.file, accept)),
    [files],
  );

  const totalBytes = useMemo(
    () => files.reduce((sum, item) => sum + item.file.size, 0),
    [files],
  );

  const value = useMemo(
    () => ({ files, addFiles, replaceFile, removeFile, clear, match, totalBytes }),
    [files, addFiles, replaceFile, removeFile, clear, match, totalBytes],
  );

  return <FileVaultContext.Provider value={value}>{children}</FileVaultContext.Provider>;
}

export function useFileVault(): FileVaultValue {
  const ctx = useContext(FileVaultContext);
  if (!ctx) throw new Error("useFileVault must be used within FileVaultProvider");
  return ctx;
}
