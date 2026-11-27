import type { ReactNode } from "react";
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
  FileInput,
  FileOutput,
  FilePenLine,
  FileSpreadsheet,
  FileStack,
  FileText,
  FileType,
  FileUp,
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
  Presentation,
  RotateCw,
  ScanEye,
  ScanText,
  Scissors,
  Shield,
  Sigma,
  Sparkles,
  Stamp,
  Table,
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
  "edit-pdf": FilePenLine,
  "office-to-pdf": FileSpreadsheet,
  "pdf-to-word": FileText,
  "pdf-to-powerpoint": Presentation,
  "pdf-to-excel": Table,
  "word-to-pdf": FileInput,
  "powerpoint-to-pdf": FileOutput,
  "excel-to-pdf": FileUp,
  "ocr-pdf": ScanEye,
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

const FONT = 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif';

function ArrowDiag({
  x1,
  y1,
  x2,
  y2,
  color = "#FFFFFF",
  head = 4.2,
  sw = 2.2,
}: {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color?: string;
  head?: number;
  sw?: number;
}) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;
  const px = -uy;
  const py = ux;
  const hx1 = x2 - ux * head + px * head * 0.78;
  const hy1 = y2 - uy * head + py * head * 0.78;
  const hx2 = x2 - ux * head - px * head * 0.78;
  const hy2 = y2 - uy * head - py * head * 0.78;
  return (
    <g stroke={color} strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round" fill="none">
      <line x1={x1} y1={y1} x2={x2} y2={y2} />
      <polyline points={`${hx1},${hy1} ${x2},${y2} ${hx2},${hy2}`} />
    </g>
  );
}

function DualConvertIcon({
  direction,
  color,
  softColor,
  glyph,
}: {
  direction: "fromPdf" | "toPdf";
  color: string;
  softColor: string;
  glyph: (cx: number, cy: number) => ReactNode;
}) {
  const topTile = (
    <rect x="4" y="4" width="24" height="24" rx="5" fill={direction === "fromPdf" ? softColor : color} />
  );
  const botTile = (
    <rect
      x="20"
      y="20"
      width="24"
      height="24"
      rx="5"
      fill={color}
      stroke="var(--bg-surface)"
      strokeWidth="2.4"
    />
  );

  if (direction === "fromPdf") {
    return (
      <>
        {topTile}
        <ArrowDiag x1={10} y1={10} x2={16.5} y2={16.5} color={color} head={4.2} sw={2.2} />
        {botTile}
        {glyph(32, 32)}
      </>
    );
  }
  return (
    <>
      {topTile}
      {glyph(15.5, 16)}
      {botTile}
      <ArrowDiag x1={27.5} y1={27.5} x2={36.5} y2={36.5} color="#FFFFFF" head={4.6} sw={2.4} />
    </>
  );
}

function LetterGlyph({
  text,
  cx,
  cy,
  size = 15.5,
  color = "#FFFFFF",
}: {
  text: string;
  cx: number;
  cy: number;
  size?: number;
  color?: string;
}) {
  return (
    <text
      x={cx}
      y={cy + 0.5}
      fill={color}
      fontFamily={FONT}
      fontSize={size}
      fontWeight="800"
      textAnchor="middle"
      dominantBaseline="central"
      letterSpacing={text.length > 2 ? "-0.5" : "0"}
    >
      {text}
    </text>
  );
}

function PictureGlyph({ cx, cy, scale = 1 }: { cx: number; cy: number; scale?: number }) {
  return (
    <g transform={`translate(${cx - 9 * scale}, ${cy - 8 * scale}) scale(${scale})`}>
      <rect x="1" y="1" width="16" height="14" rx="2" fill="none" stroke="#FFFFFF" strokeWidth="1.9" />
      <circle cx="5.5" cy="5.5" r="1.6" fill="#FFFFFF" />
      <path d="M2.5 13 L7 8.5 L10 11.2 L12.5 9 L15.5 13 Z" fill="#FFFFFF" />
    </g>
  );
}

function TileBase({ color, children }: { color: string; children: ReactNode }) {
  return (
    <>
      <rect x="5" y="5" width="38" height="38" rx="8" fill={color} />
      {children}
    </>
  );
}

function renderCustomSvg(toolId: string): ReactNode {
  switch (toolId) {
    case "merge":
      return (
        <>
          <rect x="4" y="4" width="24" height="24" rx="5" fill="#E86A42" />
          <ArrowDiag x1={9.5} y1={9.5} x2={17.5} y2={17.5} head={4.4} sw={2.3} />
          <rect
            x="20"
            y="20"
            width="24"
            height="24"
            rx="5"
            fill="#E86A42"
            stroke="var(--bg-surface)"
            strokeWidth="2.4"
          />
          <ArrowDiag x1={38.5} y1={38.5} x2={30.5} y2={30.5} head={4.4} sw={2.3} />
        </>
      );

    case "split":
      return (
        <>
          <rect x="4" y="4" width="24" height="24" rx="5" fill="#E86A42" />
          <ArrowDiag x1={18} y1={18} x2={9.5} y2={9.5} head={4.4} sw={2.3} />
          <rect
            x="20"
            y="20"
            width="24"
            height="24"
            rx="5"
            fill="#E86A42"
            stroke="var(--bg-surface)"
            strokeWidth="2.4"
          />
          <ArrowDiag x1={30} y1={30} x2={38.5} y2={38.5} head={4.4} sw={2.3} />
        </>
      );

    case "compress":
      return (
        <>
          {[
            { x: 5, y: 5, x1: 9.5, y1: 9.5, x2: 16.5, y2: 16.5 },
            { x: 26, y: 5, x1: 38.5, y1: 9.5, x2: 31.5, y2: 16.5 },
            { x: 5, y: 26, x1: 9.5, y1: 38.5, x2: 16.5, y2: 31.5 },
            { x: 26, y: 26, x1: 38.5, y1: 38.5, x2: 31.5, y2: 31.5 },
          ].map((q) => (
            <g key={`${q.x}-${q.y}`}>
              <rect x={q.x} y={q.y} width="17" height="17" rx="3.8" fill="#68A843" />
              <ArrowDiag x1={q.x1} y1={q.y1} x2={q.x2} y2={q.y2} head={3.8} sw={2.1} />
            </g>
          ))}
        </>
      );

    case "compress-image":
      return (
        <TileBase color="#2FA37C">
          <PictureGlyph cx={24} cy={22} scale={1.15} />
          <ArrowDiag x1={8} y1={37} x2={15} y2={30} head={3.6} sw={2.1} />
          <ArrowDiag x1={40} y1={37} x2={33} y2={30} head={3.6} sw={2.1} />
        </TileBase>
      );

    case "pdf-to-word":
      return (
        <DualConvertIcon
          direction="fromPdf"
          color="#4F74D4"
          softColor="#D6E0FA"
          glyph={(cx, cy) => <LetterGlyph text="W" cx={cx} cy={cy} size={15} />}
        />
      );

    case "word-to-pdf":
      return (
        <DualConvertIcon
          direction="toPdf"
          color="#4F74D4"
          softColor="#D6E0FA"
          glyph={(cx, cy) => <LetterGlyph text="W" cx={cx} cy={cy} size={15} />}
        />
      );

    case "pdf-to-powerpoint":
      return (
        <DualConvertIcon
          direction="fromPdf"
          color="#EA7145"
          softColor="#FADCD0"
          glyph={(cx, cy) => <LetterGlyph text="P" cx={cx} cy={cy} size={15.5} />}
        />
      );

    case "powerpoint-to-pdf":
      return (
        <DualConvertIcon
          direction="toPdf"
          color="#EA7145"
          softColor="#FADCD0"
          glyph={(cx, cy) => <LetterGlyph text="P" cx={cx} cy={cy} size={15.5} />}
        />
      );

    case "pdf-to-excel":
      return (
        <DualConvertIcon
          direction="fromPdf"
          color="#58A164"
          softColor="#D2ECD7"
          glyph={(cx, cy) => <LetterGlyph text="X" cx={cx} cy={cy} size={15.5} />}
        />
      );

    case "excel-to-pdf":
      return (
        <DualConvertIcon
          direction="toPdf"
          color="#58A164"
          softColor="#D2ECD7"
          glyph={(cx, cy) => <LetterGlyph text="X" cx={cx} cy={cy} size={15.5} />}
        />
      );

    case "pdf-to-image":
      return (
        <DualConvertIcon
          direction="fromPdf"
          color="#D9941E"
          softColor="#FBE7BD"
          glyph={(cx, cy) => <PictureGlyph cx={cx} cy={cy} scale={0.92} />}
        />
      );

    case "image-to-pdf":
      return (
        <DualConvertIcon
          direction="toPdf"
          color="#D9941E"
          softColor="#FBE7BD"
          glyph={(cx, cy) => <PictureGlyph cx={cx} cy={cy} scale={0.92} />}
        />
      );

    case "html-to-pdf":
      return (
        <DualConvertIcon
          direction="toPdf"
          color="#0284C7"
          softColor="#CEEBFB"
          glyph={(cx, cy) => <LetterGlyph text="</>" cx={cx} cy={cy} size={10.5} />}
        />
      );

    case "pdf-to-markdown":
      return (
        <DualConvertIcon
          direction="fromPdf"
          color="#475569"
          softColor="#DCE4EE"
          glyph={(cx, cy) => <LetterGlyph text="MD" cx={cx} cy={cy} size={11} />}
        />
      );

    case "office-to-pdf":
      return (
        <>
          <rect x="4" y="6" width="13" height="13" rx="3" fill="#4F74D4" />
          <LetterGlyph text="W" cx={10.5} cy={12.5} size={8.5} />
          <rect x="15" y="4" width="13" height="13" rx="3" fill="#EA7145" />
          <LetterGlyph text="P" cx={21.5} cy={10.5} size={8.5} />
          <rect x="4" y="18" width="13" height="13" rx="3" fill="#58A164" />
          <LetterGlyph text="X" cx={10.5} cy={24.5} size={8.5} />
          <rect
            x="19"
            y="19"
            width="25"
            height="25"
            rx="5.5"
            fill="#B7281E"
            stroke="var(--bg-surface)"
            strokeWidth="2.4"
          />
          <LetterGlyph text="PDF" cx={31.5} cy={31.5} size={9.5} />
        </>
      );

    case "edit-pdf":
      return (
        <TileBase color="#96577B">
          <rect x="13.5" y="13.5" width="21" height="21" fill="none" stroke="#FFFFFF" strokeWidth="1.7" />
          <rect x="11.5" y="11.5" width="4" height="4" fill="#FFFFFF" />
          <rect x="32.5" y="11.5" width="4" height="4" fill="#FFFFFF" />
          <rect x="11.5" y="32.5" width="4" height="4" fill="#FFFFFF" />
          <rect x="32.5" y="32.5" width="4" height="4" fill="#FFFFFF" />
          <path d="M16.5 19.5 H23.5 M20 19.5 V29.5" stroke="#FFFFFF" strokeWidth="2.2" strokeLinecap="round" />
          <path
            d="M25.5 28.5 L31.5 18.5 L34.2 20.1 L28.2 30.1 L24.8 31.2 Z"
            fill="#FFFFFF"
          />
        </TileBase>
      );

    case "watermark":
      return (
        <TileBase color="#D97724">
          <path
            d="M24 12 C21.2 12 19.5 14 20 16.8 L21.2 22.5 H26.8 L28 16.8 C28.5 14 26.8 12 24 12 Z"
            fill="#FFFFFF"
          />
          <rect x="13" y="24" width="22" height="5.5" rx="2" fill="#FFFFFF" />
          <rect x="11" y="32" width="26" height="3.2" rx="1.6" fill="#FFFFFF" opacity="0.9" />
        </TileBase>
      );

    case "sign":
      return (
        <TileBase color="#3B69B8">
          <path
            d="M29.5 13 L35 18.5 L21 32.5 L14.5 34 L16 27.5 Z"
            fill="#FFFFFF"
          />
          <path
            d="M12.5 36.5 Q18 33 23 36.5 T35 36"
            fill="none"
            stroke="#FFFFFF"
            strokeWidth="2.3"
            strokeLinecap="round"
          />
        </TileBase>
      );

    case "protect":
      return (
        <TileBase color="#4C4F59">
          <path
            d="M17.5 22 V17.5 A6.5 6.5 0 0 1 30.5 17.5 V22"
            fill="none"
            stroke="#FFFFFF"
            strokeWidth="2.8"
            strokeLinecap="round"
          />
          <rect x="14" y="21.5" width="20" height="14.5" rx="3" fill="#FFFFFF" />
          <circle cx="24" cy="28" r="2.2" fill="#4C4F59" />
          <rect x="23" y="29" width="2" height="3.5" rx="1" fill="#4C4F59" />
        </TileBase>
      );

    case "unlock":
      return (
        <TileBase color="#5B616E">
          <path
            d="M17.5 18.5 A6.5 6.5 0 0 1 30.5 16.5 V22"
            fill="none"
            stroke="#FFFFFF"
            strokeWidth="2.8"
            strokeLinecap="round"
          />
          <rect x="14" y="21.5" width="20" height="14.5" rx="3" fill="#FFFFFF" />
          <circle cx="24" cy="28" r="2.2" fill="#5B616E" />
          <rect x="23" y="29" width="2" height="3.5" rx="1" fill="#5B616E" />
        </TileBase>
      );

    case "organize":
      return (
        <>
          <rect x="5" y="5" width="17" height="17" rx="4" fill="#E86A42" />
          <rect x="26" y="5" width="17" height="17" rx="4" fill="#F9CEC1" />
          <rect x="5" y="26" width="17" height="17" rx="4" fill="#F9CEC1" />
          <rect x="26" y="26" width="17" height="17" rx="4" fill="#E86A42" />
          <LetterGlyph text="1" cx={13.5} cy={13.5} size={11} />
          <LetterGlyph text="2" cx={34.5} cy={34.5} size={11} />
          <ArrowDiag x1={18} y1={30} x2={30} y2={18} color="#E86A42" head={3.8} sw={2.2} />
        </>
      );

    case "rotate":
      return (
        <TileBase color="#E15A3E">
          <rect x="15.5" y="16" width="15" height="17.5" rx="2.5" fill="none" stroke="#FFFFFF" strokeWidth="2.3" />
          <path
            d="M21 12.5 A11.5 11.5 0 1 1 13.5 29"
            fill="none"
            stroke="#FFFFFF"
            strokeWidth="2.4"
            strokeLinecap="round"
          />
          <polyline points="20,9.5 23,12.5 19.5,15.5" fill="none" stroke="#FFFFFF" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
        </TileBase>
      );

    case "crop":
      return (
        <TileBase color="#B06825">
          <path d="M16 11 V31 H36" fill="none" stroke="#FFFFFF" strokeWidth="2.8" strokeLinecap="round" strokeLinejoin="round" />
          <path d="M12 17 H31 V36" fill="none" stroke="#FFFFFF" strokeWidth="2.8" strokeLinecap="round" strokeLinejoin="round" />
        </TileBase>
      );

    case "page-numbers":
      return (
        <TileBase color="#D97236">
          <rect x="13" y="10" width="22" height="28" rx="3" fill="none" stroke="#FFFFFF" strokeWidth="2.2" />
          <line x1="17" y1="16" x2="31" y2="16" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" opacity="0.7" />
          <line x1="17" y1="21" x2="28" y2="21" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" opacity="0.7" />
          <rect x="20" y="26" width="12" height="9" rx="2" fill="#FFFFFF" />
          <LetterGlyph text="1" cx={26} cy={30.5} size={8} color="#D97236" />
        </TileBase>
      );

    case "forms":
      return (
        <TileBase color="#7C5AC2">
          <rect x="13" y="14.5" width="7" height="7" rx="1.5" fill="#FFFFFF" />
          <polyline points="14.5,18 16.2,19.7 19,16.5" fill="none" stroke="#7C5AC2" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          <rect x="23" y="16" width="12" height="4" rx="2" fill="#FFFFFF" />
          <rect x="13" y="25.5" width="22" height="8" rx="2.5" fill="#FFFFFF" opacity="0.92" />
          <rect x="15.5" y="28.2" width="10" height="2.6" rx="1.3" fill="#7C5AC2" />
        </TileBase>
      );

    case "compare":
      return (
        <>
          <rect x="4" y="6" width="21" height="36" rx="4.5" fill="#3B82F6" />
          <rect x="23" y="6" width="21" height="36" rx="4.5" fill="#1D4ED8" stroke="var(--bg-surface)" strokeWidth="2" />
          <line x1="9" y1="16" x2="19" y2="16" stroke="#FFFFFF" strokeWidth="2.1" strokeLinecap="round" />
          <line x1="9" y1="22" x2="17" y2="22" stroke="#FFFFFF" strokeWidth="2.1" strokeLinecap="round" />
          <line x1="29" y1="16" x2="39" y2="16" stroke="#FFFFFF" strokeWidth="2.1" strokeLinecap="round" />
          <line x1="29" y1="22" x2="39" y2="22" stroke="#93C5FD" strokeWidth="2.1" strokeLinecap="round" />
          <ArrowDiag x1={19} y1={31} x2={30} y2={31} head={3.6} sw={2} />
        </>
      );

    case "ocr-pdf":
      return (
        <TileBase color="#4B9E5F">
          <path d="M12 18 V12 H18 M30 12 H36 V18 M12 30 V36 H18 M30 36 H36 V30" fill="none" stroke="#FFFFFF" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
          <LetterGlyph text="OCR" cx={24} cy={24} size={10.5} />
        </TileBase>
      );

    case "scan-to-pdf":
      return (
        <TileBase color="#0F9D8A">
          <rect x="15" y="11" width="18" height="26" rx="2.5" fill="none" stroke="#FFFFFF" strokeWidth="2.1" />
          <line x1="11" y1="24" x2="37" y2="24" stroke="#FFFFFF" strokeWidth="2.6" strokeLinecap="round" />
          <line x1="19" y1="17" x2="29" y2="17" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" opacity="0.8" />
          <line x1="19" y1="31" x2="29" y2="31" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" opacity="0.8" />
        </TileBase>
      );

    case "pdfa":
      return (
        <TileBase color="#1F7A8C">
          <LetterGlyph text="PDF" cx={24} cy={17.5} size={11.5} />
          <rect x="13" y="25.5" width="22" height="9" rx="3" fill="#FFFFFF" />
          <LetterGlyph text="/A" cx={24} cy={30} size={8.5} color="#1F7A8C" />
        </TileBase>
      );

    case "repair":
      return (
        <TileBase color="#459E6E">
          <path
            d="M30.5 14 A5.5 5.5 0 0 0 23 20.5 L14.5 29 A2.2 2.2 0 1 0 17.6 32.1 L26.1 23.6 A5.5 5.5 0 0 0 33.5 16.5 L29.5 19.8 L27.2 17.5 Z"
            fill="#FFFFFF"
          />
        </TileBase>
      );

    case "redact":
      return (
        <TileBase color="#333842">
          <line x1="13" y1="15.5" x2="35" y2="15.5" stroke="#FFFFFF" strokeWidth="2.3" strokeLinecap="round" opacity="0.7" />
          <rect x="12.5" y="21" width="23" height="6.5" rx="1.5" fill="#B7281E" />
          <line x1="13" y1="33" x2="29" y2="33" stroke="#FFFFFF" strokeWidth="2.3" strokeLinecap="round" opacity="0.7" />
        </TileBase>
      );

    case "equation-to-image":
      return (
        <TileBase color="#B7281E">
          <LetterGlyph text="Σ" cx={24} cy={23.5} size={23} />
        </TileBase>
      );

    case "latex-resume":
      return (
        <TileBase color="#B91C1C">
          <LetterGlyph text="TeX" cx={24} cy={24} size={14} />
        </TileBase>
      );

    case "resume-builder":
      return (
        <TileBase color="#DC2626">
          <circle cx="24" cy="17.5" r="4.8" fill="#FFFFFF" />
          <path d="M16 30.5 C16 26.2 32 26.2 32 30.5 Z" fill="#FFFFFF" />
          <line x1="15" y1="35" x2="33" y2="35" stroke="#FFFFFF" strokeWidth="2.2" strokeLinecap="round" />
        </TileBase>
      );

    case "summarize":
      return (
        <TileBase color="#8B3DFF">
          <path
            d="M17.5 12 L19.2 16.8 L24 18.5 L19.2 20.2 L17.5 25 L15.8 20.2 L11 18.5 L15.8 16.8 Z"
            fill="#FFFFFF"
          />
          <line x1="25.5" y1="18.5" x2="35.5" y2="18.5" stroke="#FFFFFF" strokeWidth="2.4" strokeLinecap="round" />
          <line x1="13" y1="29" x2="35.5" y2="29" stroke="#FFFFFF" strokeWidth="2.4" strokeLinecap="round" />
          <line x1="13" y1="34.5" x2="28" y2="34.5" stroke="#FFFFFF" strokeWidth="2.4" strokeLinecap="round" opacity="0.85" />
        </TileBase>
      );

    case "translate":
      return (
        <>
          <rect x="4" y="4" width="24" height="24" rx="5" fill="#E9D8FD" />
          <LetterGlyph text="A" cx={15.5} cy={16} size={14.5} color="#7C3AED" />
          <rect
            x="20"
            y="20"
            width="24"
            height="24"
            rx="5"
            fill="#7C3AED"
            stroke="var(--bg-surface)"
            strokeWidth="2.4"
          />
          <LetterGlyph text="文" cx={32} cy={32} size={14.5} />
        </>
      );

    default:
      return (
        <TileBase color="#B7281E">
          <LetterGlyph text="PDF" cx={24} cy={24} size={12} />
        </TileBase>
      );
  }
}

const SIZE_CLASSES = {
  xs: "h-5 w-5",
  sm: "h-8 w-8",
  md: "h-11 w-11",
  lg: "h-14 w-14",
} as const;

export function ToolIconBadge({
  toolId,
  category: _category,
  size = "md",
}: {
  toolId: string;
  category?: ToolMeta["category"];
  size?: "xs" | "sm" | "md" | "lg";
}) {
  return (
    <span className={`inline-grid shrink-0 place-items-center ${SIZE_CLASSES[size]}`}>
      <svg
        viewBox="0 0 48 48"
        className="h-full w-full"
        aria-hidden="true"
      >
        {renderCustomSvg(toolId)}
      </svg>
    </span>
  );
}
