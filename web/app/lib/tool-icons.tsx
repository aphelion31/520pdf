import type { LucideIcon } from "lucide-react";
import {
  Archive,
  ArrowLeftRight,
  Bot,
  ClipboardList,
  Crop,
  EyeOff,
  FileCode2,
  FileImage,
  FileSpreadsheet,
  FileStack,
  FileType,
  FileUser,
  FolderKanban,
  Gauge,
  GitCompareArrows,
  Globe,
  Grid2x2,
  Hash,
  ImageDown,
  Images,
  Languages,
  LayoutGrid,
  Lock,
  Minimize2,
  PenTool,
  Pencil,
  RotateCw,
  ScanText,
  Scissors,
  Shield,
  Sigma,
  Sparkles,
  Stamp,
  Unlock,
  Wand2,
  Wrench,
} from "lucide-react";
import type { ToolMeta } from "~/lib/catalog";

type CategoryFilter = "All" | ToolMeta["category"];

const TOOL_ICONS: Record<string, LucideIcon> = {
  merge: FileStack,
  split: Scissors,
  compress: Minimize2,
  "compress-image": ImageDown,
  "pdf-to-image": Images,
  "image-to-pdf": FileImage,
  watermark: Stamp,
  protect: Lock,
  sign: PenTool,
  "equation-to-image": Sigma,
  "latex-resume": FileCode2,
  "resume-builder": FileUser,
  organize: LayoutGrid,
  rotate: RotateCw,
  crop: Crop,
  "page-numbers": Hash,
  forms: ClipboardList,
  "office-to-pdf": FileSpreadsheet,
  "html-to-pdf": Globe,
  compare: GitCompareArrows,
  "pdf-to-markdown": FileType,
  unlock: Unlock,
  pdfa: Archive,
  "scan-to-pdf": ScanText,
  repair: Wrench,
  redact: EyeOff,
  summarize: Sparkles,
  translate: Languages,
};

export const CATEGORY_META: Record<
  CategoryFilter,
  { icon: LucideIcon; tileClass: string; pillClass: string }
> = {
  All: {
    icon: Grid2x2,
    tileClass: "bg-[var(--accent-soft)] text-[var(--accent)]",
    pillClass: "bg-[var(--accent-soft)] text-[var(--accent)]",
  },
  Organize: {
    icon: FolderKanban,
    tileClass: "bg-rose-500/10 text-rose-600 dark:text-rose-400",
    pillClass: "bg-rose-500/10 text-rose-600 dark:text-rose-400",
  },
  Optimize: {
    icon: Gauge,
    tileClass: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    pillClass: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
  },
  Convert: {
    icon: ArrowLeftRight,
    tileClass: "bg-sky-500/10 text-sky-600 dark:text-sky-400",
    pillClass: "bg-sky-500/10 text-sky-600 dark:text-sky-400",
  },
  Edit: {
    icon: Pencil,
    tileClass: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
    pillClass: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
  },
  Security: {
    icon: Shield,
    tileClass: "bg-violet-500/10 text-violet-600 dark:text-violet-400",
    pillClass: "bg-violet-500/10 text-violet-600 dark:text-violet-400",
  },
  Create: {
    icon: Wand2,
    tileClass: "bg-[var(--accent-soft)] text-[var(--accent)]",
    pillClass: "bg-[var(--accent-soft)] text-[var(--accent)]",
  },
  AI: {
    icon: Bot,
    tileClass: "bg-fuchsia-500/10 text-fuchsia-600 dark:text-fuchsia-400",
    pillClass: "bg-fuchsia-500/10 text-fuchsia-600 dark:text-fuchsia-400",
  },
};

export function getToolIcon(toolId: string): LucideIcon {
  return TOOL_ICONS[toolId] ?? FileType;
}

const SIZE_CLASSES = {
  sm: { box: "h-8 w-8 rounded-lg", icon: "h-4 w-4" },
  md: { box: "h-10 w-10 rounded-xl", icon: "h-5 w-5" },
  lg: { box: "h-12 w-12 rounded-2xl", icon: "h-6 w-6" },
} as const;

export function ToolIconBadge({
  toolId,
  category = "Create",
  size = "md",
}: {
  toolId: string;
  category?: ToolMeta["category"];
  size?: "sm" | "md" | "lg";
}) {
  const Icon = getToolIcon(toolId);
  const { tileClass } = CATEGORY_META[category];
  const { box, icon } = SIZE_CLASSES[size];
  return (
    <span className={`inline-grid shrink-0 place-items-center ${box} ${tileClass}`}>
      <Icon className={icon} aria-hidden="true" />
    </span>
  );
}
