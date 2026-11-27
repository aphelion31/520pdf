import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircle,
  ArrowDown,
  ArrowUp,
  BookmarkCheck,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Download,
  GripVertical,
  Lightbulb,
  Loader2,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  Upload,
} from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { HelpTip } from "~/components/HelpTip";
import { ResumePreview } from "~/components/ResumePreview";
import { SlidingTabs } from "~/components/SlidingTabs";
import { ToolShell } from "~/components/ToolShell";
import { runTool, type ToolResponse } from "~/lib/api";
import {
  clearDraft,
  computeAtsScore,
  createDefaultResume,
  DEGREE_OPTIONS,
  loadDraft,
  MONTHS,
  normalizeImportedResume,
  OTHER_CATEGORIES,
  saveDraft,
  SECTION_DEFAULTS_EN,
  SECTION_DEFAULTS_ID,
  STEP_ITEMS,
  STEP_TIPS,
  toResumePayload,
  uid,
  YEARS,
  type EducationItem,
  type ExperienceItem,
  type OrganizationItem,
  type OtherItem,
  type ResumeDocument,
  type ResumeStyle,
  type SectionTitles,
  type StepId,
} from "~/lib/resume-types";

function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return items;
  }
  const next = [...items];
  const [picked] = next.splice(from, 1);
  next.splice(to, 0, picked);
  return next;
}

async function resizePortraitFile(file: File): Promise<string> {
  if (file.size > 10 * 1024 * 1024) {
    throw new Error("Photo file must be 10 MB or smaller.");
  }
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(new Error("Could not read selected image."));
    reader.readAsDataURL(file);
  });
  const img = await new Promise<HTMLImageElement>((resolve, reject) => {
    const el = new Image();
    el.onload = () => resolve(el);
    el.onerror = () => reject(new Error("Unsupported image format."));
    el.src = dataUrl;
  });

  const targetW = 360;
  const targetH = 450;
  const targetRatio = targetW / targetH;
  const srcRatio = img.naturalWidth / Math.max(1, img.naturalHeight);
  let sx = 0;
  let sy = 0;
  let sw = img.naturalWidth;
  let sh = img.naturalHeight;
  if (srcRatio > targetRatio) {
    sw = Math.round(img.naturalHeight * targetRatio);
    sx = Math.max(0, Math.round((img.naturalWidth - sw) / 2));
  } else {
    sh = Math.round(img.naturalWidth / targetRatio);
    sy = Math.max(0, Math.round((img.naturalHeight - sh) / 4));
  }
  const canvas = document.createElement("canvas");
  canvas.width = targetW;
  canvas.height = targetH;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas context unavailable.");
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, targetW, targetH);
  ctx.drawImage(img, sx, sy, sw, sh, 0, 0, targetW, targetH);
  return canvas.toDataURL("image/jpeg", 0.88);
}

interface SectionHeaderProps {
  title: string;
  subtitle: string;
  onTitleChange?: (val: string) => void;
  showTips: boolean;
  onToggleTips: () => void;
  tips: string[];
}

function SectionHeader({
  title,
  subtitle,
  onTitleChange,
  showTips,
  onToggleTips,
  tips,
}: SectionHeaderProps) {
  const [editing, setEditing] = useState(false);
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          {onTitleChange && editing ? (
            <input
              type="text"
              autoFocus
              value={title}
              onChange={(e) => onTitleChange(e.target.value)}
              onBlur={() => setEditing(false)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === "Escape") setEditing(false);
              }}
              className="cm-input max-w-sm px-2.5 py-1 text-base font-semibold"
            />
          ) : (
            <div className="flex items-center gap-2">
              <h2 className="text-base font-semibold tracking-tight text-[var(--ink-primary)]">
                {title}
              </h2>
              {onTitleChange && (
                <button
                  type="button"
                  aria-label="Edit section title"
                  onClick={() => setEditing(true)}
                  className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
                >
                  <Pencil className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          )}
          <p className="mt-0.5 text-xs text-[var(--ink-secondary)]">{subtitle}</p>
        </div>

        <button
          type="button"
          onClick={onToggleTips}
          className={`cm-btn inline-flex items-center gap-1.5 rounded-xl border px-2.5 py-1.5 text-xs font-medium ${
            showTips
              ? "border-[var(--border-strong)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]"
              : "border-[var(--border-subtle)] bg-[var(--bg-page)] text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
          }`}
        >
          <Lightbulb className="h-3.5 w-3.5 text-amber-500" /> Tips
        </button>
      </div>

      <div className="cm-accordion" data-open={showTips ? "true" : "false"}>
        <div>
          <ul className="space-y-1 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-band)] px-3.5 py-2.5 text-xs text-[var(--ink-secondary)]">
            {tips.map((t, i) => (
              <li key={i} className="flex items-start gap-2">
                <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500/80" />
                <span>{t}</span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

interface AccordionCardProps {
  title: string;
  subtitle?: string;
  expanded: boolean;
  onToggleExpand: () => void;
  onDelete: () => void;
  index: number;
  total: number;
  onMove: (from: number, to: number) => void;
  dragIndex: number | null;
  onDragStart: (idx: number) => void;
  onDragEnd: () => void;
  children: React.ReactNode;
}

function AccordionCard({
  title,
  subtitle,
  expanded,
  onToggleExpand,
  onDelete,
  index,
  total,
  onMove,
  dragIndex,
  onDragStart,
  onDragEnd,
  children,
}: AccordionCardProps) {
  return (
    <div
      draggable
      onDragStart={() => onDragStart(index)}
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        if (dragIndex !== null && dragIndex !== index) {
          onMove(dragIndex, index);
        }
        onDragEnd();
      }}
      onDragEnd={onDragEnd}
      className={`rounded-xl border transition-colors duration-150 ${
        dragIndex === index
          ? "border-[var(--accent)] bg-[var(--bg-subtle)]"
          : "border-[var(--border-subtle)] bg-[var(--bg-band)]"
      }`}
    >
      <div className="flex items-center gap-2 px-3 py-2.5">
        <span
          title="Drag to reorder"
          className="cursor-grab text-[var(--ink-muted)] hover:text-[var(--ink-primary)]"
        >
          <GripVertical className="h-4 w-4" />
        </span>

        <button
          type="button"
          onClick={onToggleExpand}
          className="min-w-0 flex-1 text-left"
        >
          <p className="truncate text-xs font-semibold text-[var(--ink-primary)]">{title}</p>
          {subtitle && (
            <p className="truncate text-[11px] text-[var(--ink-muted)]">{subtitle}</p>
          )}
        </button>

        <div className="flex items-center gap-0.5">
          <button
            type="button"
            disabled={index === 0}
            aria-label="Move item up"
            onClick={() => onMove(index, index - 1)}
            className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-surface)] hover:text-[var(--ink-primary)] disabled:opacity-30"
          >
            <ArrowUp className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            disabled={index === total - 1}
            aria-label="Move item down"
            onClick={() => onMove(index, index + 1)}
            className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-surface)] hover:text-[var(--ink-primary)] disabled:opacity-30"
          >
            <ArrowDown className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            aria-label="Delete item"
            onClick={onDelete}
            className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-red-500/10 hover:text-red-600"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            aria-label={expanded ? "Collapse item" : "Expand item"}
            onClick={onToggleExpand}
            className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-surface)] hover:text-[var(--ink-primary)]"
          >
            {expanded ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
          </button>
        </div>
      </div>

      <div className="cm-accordion" data-open={expanded ? "true" : "false"}>
        <div>
          <div className="space-y-3 rounded-b-xl border-t border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3.5 py-3.5">
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}

interface MonthYearPickerProps {
  label: string;
  month: string;
  year: string;
  disabled?: boolean;
  onMonthChange: (val: string) => void;
  onYearChange: (val: string) => void;
}

function MonthYearPicker({
  label,
  month,
  year,
  disabled = false,
  onMonthChange,
  onYearChange,
}: MonthYearPickerProps) {
  return (
    <div>
      <label className="mb-1 block text-xs font-medium text-[var(--ink-secondary)]">
        {label}
      </label>
      <div className="grid grid-cols-2 gap-1.5">
        <BasecoatSelect
          size="sm"
          disabled={disabled}
          ariaLabel={`${label} month`}
          placeholder="Month"
          value={month}
          onChange={onMonthChange}
          options={[
            { value: "", label: "—" },
            ...MONTHS.map((m) => ({ value: m, label: m })),
          ]}
        />
        <BasecoatSelect
          size="sm"
          disabled={disabled}
          ariaLabel={`${label} year`}
          placeholder="Year"
          value={year}
          onChange={onYearChange}
          options={[
            { value: "", label: "—" },
            ...YEARS.map((y) => ({ value: y, label: y })),
          ]}
        />
      </div>
    </div>
  );
}

export default function ResumeBuilder() {
  const [doc, setDoc] = useState<ResumeDocument>(() => createDefaultResume());
  const [step, setStep] = useState<StepId>("personal");
  const [editingDocTitle, setEditingDocTitle] = useState(false);
  const [showTips, setShowTips] = useState(false);
  const [openIds, setOpenIds] = useState<Record<string, boolean>>({
    "exp-1": true,
    "edu-1": true,
    "org-1": true,
    "oth-1": true,
  });
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [savedNotice, setSavedNotice] = useState(false);
  const [photoError, setPhotoError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const importInputRef = useRef<HTMLInputElement>(null);
  const photoInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const saved = loadDraft();
    if (saved) {
      setDoc(saved);
    }
  }, []);

  const ats = useMemo(() => computeAtsScore(doc), [doc]);

  const toggleOpen = (id: string) =>
    setOpenIds((prev) => ({ ...prev, [id]: !prev[id] }));

  const updateTitle = (key: keyof SectionTitles, val: string) =>
    setDoc((prev) => ({
      ...prev,
      sectionTitles: { ...prev.sectionTitles, [key]: val },
    }));

  const handleStyleChange = (patch: Partial<ResumeStyle>) => {
    setDoc((prev) => {
      const nextStyle = { ...prev.style, ...patch };
      let nextTitles = prev.sectionTitles;
      if (patch.language && patch.language !== prev.style.language) {
        const oldDefaults =
          prev.style.language === "id" ? SECTION_DEFAULTS_ID : SECTION_DEFAULTS_EN;
        const newDefaults =
          patch.language === "id" ? SECTION_DEFAULTS_ID : SECTION_DEFAULTS_EN;
        nextTitles = {
          experience:
            prev.sectionTitles.experience === oldDefaults.experience
              ? newDefaults.experience
              : prev.sectionTitles.experience,
          education:
            prev.sectionTitles.education === oldDefaults.education
              ? newDefaults.education
              : prev.sectionTitles.education,
          organizations:
            prev.sectionTitles.organizations === oldDefaults.organizations
              ? newDefaults.organizations
              : prev.sectionTitles.organizations,
          other:
            prev.sectionTitles.other === oldDefaults.other
              ? newDefaults.other
              : prev.sectionTitles.other,
        };
      }
      return { ...prev, style: nextStyle, sectionTitles: nextTitles };
    });
  };

  const handleSaveDraft = () => {
    if (saveDraft(doc)) {
      setSavedNotice(true);
      setTimeout(() => setSavedNotice(false), 1800);
    }
  };

  const handleReset = () => {
    clearDraft();
    const fresh = createDefaultResume();
    setDoc(fresh);
    setStep("personal");
    setResult(null);
    setError(null);
    setOpenIds({ "exp-1": true, "edu-1": true, "org-1": true, "oth-1": true });
  };

  const handleExportJson = () => {
    const payload = JSON.stringify(toResumePayload(doc), null, 2);
    const blob = new Blob([payload], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${(doc.personalInfo.fullName || "resume").toLowerCase().replace(/\s+/g, "-")}.json`;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  const handleImportJson = async (file: File | undefined) => {
    if (!file) return;
    try {
      const text = await file.text();
      const parsed = normalizeImportedResume(JSON.parse(text));
      setDoc(parsed);
      saveDraft(parsed);
      setError(null);
    } catch {
      setError("Invalid resume JSON file.");
    }
  };

  const handlePhotoUpload = async (file: File | undefined) => {
    if (!file) return;
    setPhotoError(null);
    try {
      const resized = await resizePortraitFile(file);
      setDoc((prev) => ({
        ...prev,
        personalInfo: { ...prev.personalInfo, photoDataUrl: resized },
      }));
    } catch (e) {
      setPhotoError(e instanceof Error ? e.message : "Could not process photo.");
    }
  };

  const handleGeneratePdf = async () => {
    saveDraft(doc);
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const payload = toResumePayload(doc);
      const res = await runTool("resume-builder", [], {
        resume_json: JSON.stringify(payload),
        template: doc.style.headerStyle,
        photo: doc.personalInfo.photoDataUrl || undefined,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Resume PDF generation failed");
    } finally {
      setBusy(false);
    }
  };

  const stepIndex = STEP_ITEMS.findIndex((s) => s.id === step);

  const goStep = (target: StepId) => {
    saveDraft(doc);
    setStep(target);
    setShowTips(false);
    setDragIndex(null);
  };

  const updateExp = (id: string, patch: Partial<ExperienceItem>) =>
    setDoc((prev) => ({
      ...prev,
      experience: prev.experience.map((x) => (x.id === id ? { ...x, ...patch } : x)),
    }));

  const updateEdu = (id: string, patch: Partial<EducationItem>) =>
    setDoc((prev) => ({
      ...prev,
      education: prev.education.map((x) => (x.id === id ? { ...x, ...patch } : x)),
    }));

  const updateOrg = (id: string, patch: Partial<OrganizationItem>) =>
    setDoc((prev) => ({
      ...prev,
      organizations: prev.organizations.map((x) => (x.id === id ? { ...x, ...patch } : x)),
    }));

  const updateOther = (id: string, patch: Partial<OtherItem>) =>
    setDoc((prev) => ({
      ...prev,
      other: prev.other.map((x) => (x.id === id ? { ...x, ...patch } : x)),
    }));

  const stepTabs = STEP_ITEMS.map((item) => {
    const completed =
      (item.id === "personal" && Boolean(doc.personalInfo.fullName.trim())) ||
      (item.id === "experience" && doc.experience.length > 0) ||
      (item.id === "education" && doc.education.length > 0) ||
      (item.id === "organizations" && doc.organizations.length > 0) ||
      (item.id === "other" && doc.other.length > 0) ||
      (item.id === "review" && ats.score >= 70);
    return {
      value: item.id,
      label: `${item.number}. ${item.shortLabel}`,
      dot: (completed ? "done" : "none") as "done" | "none",
    };
  });

  const inputClass = "cm-input mt-1 px-3 py-2 text-sm";

  const headerActions = (
    <>
      <button
        type="button"
        onClick={handleSaveDraft}
        className="cm-btn inline-flex h-8 items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
      >
        {savedNotice ? (
          <>
            <Check className="h-3.5 w-3.5 text-emerald-600" /> Saved
          </>
        ) : (
          <>
            <BookmarkCheck className="h-3.5 w-3.5" /> Save Draft
          </>
        )}
      </button>

      <input
        ref={importInputRef}
        type="file"
        accept=".json,application/json"
        className="hidden"
        onChange={(e) => {
          handleImportJson(e.target.files?.[0]);
          e.target.value = "";
        }}
      />
      <button
        type="button"
        onClick={() => importInputRef.current?.click()}
        className="cm-btn inline-flex h-8 items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
      >
        <Upload className="h-3.5 w-3.5" /> Import JSON
      </button>

      <button
        type="button"
        onClick={handleExportJson}
        className="cm-btn inline-flex h-8 items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
      >
        <Download className="h-3.5 w-3.5" /> Export JSON
      </button>

      <button
        type="button"
        onClick={handleReset}
        className="cm-btn inline-flex h-8 items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 text-xs font-medium text-[var(--ink-muted)] hover:border-red-500/40 hover:text-red-600"
      >
        <RotateCcw className="h-3.5 w-3.5" /> Reset
      </button>
    </>
  );

  return (
    <ToolShell
      title="Resume Builder"
      description="Create an ATS-friendly resume step by step with live formatting controls, score checks, and crisp A4 PDF export."
      category="Create"
      headerActions={headerActions}
    >
      {/* Unified Document Title + Sliding Step Bar */}
      <div className="surface-card mb-5 flex flex-wrap items-center justify-between gap-3 rounded-2xl px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          {editingDocTitle ? (
            <input
              type="text"
              autoFocus
              value={doc.documentTitle}
              onChange={(e) => setDoc((p) => ({ ...p, documentTitle: e.target.value }))}
              onBlur={() => setEditingDocTitle(false)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === "Escape") setEditingDocTitle(false);
              }}
              className="cm-input px-2.5 py-1 text-sm font-semibold"
            />
          ) : (
            <>
              <span className="truncate text-sm font-semibold text-[var(--ink-primary)]">
                {doc.documentTitle || "Untitled Resume"}
              </span>
              <button
                type="button"
                aria-label="Rename resume document"
                onClick={() => setEditingDocTitle(true)}
                className="cm-btn rounded-md p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-subtle)] hover:text-[var(--ink-primary)]"
              >
                <Pencil className="h-3.5 w-3.5" />
              </button>
            </>
          )}
        </div>

        <SlidingTabs
          ariaLabel="Resume sections"
          items={stepTabs}
          value={step}
          onChange={(v) => goStep(v as StepId)}
          radius={12}
        />
      </div>

      {/* Main 2-Column Studio + Live A4 Sheet */}
      <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.08fr)]">
        {/* Left: Step Form */}
        <div className="surface-card rounded-2xl p-5 sm:p-6">
          <div className="space-y-5">
            {step === "personal" && (
              <>
                <SectionHeader
                  title="Enter your personal information"
                  subtitle="Help recruiters reach you and understand your profile at a glance."
                  showTips={showTips}
                  onToggleTips={() => setShowTips((v) => !v)}
                  tips={STEP_TIPS.personal}
                />

                <div className="space-y-3.5">
                  <div>
                    <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                      Full Name
                    </label>
                    <input
                      type="text"
                      value={doc.personalInfo.fullName}
                      onChange={(e) =>
                        setDoc((p) => ({
                          ...p,
                          personalInfo: { ...p.personalInfo, fullName: e.target.value },
                        }))
                      }
                      className={inputClass}
                    />
                  </div>

                  <div className="grid gap-3 sm:grid-cols-2">
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Phone Number (Mobile)
                      </label>
                      <input
                        type="text"
                        placeholder="+62 812..."
                        value={doc.personalInfo.phone}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, phone: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Email Address
                      </label>
                      <input
                        type="email"
                        value={doc.personalInfo.email}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, email: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                  </div>

                  <div className="grid gap-3 sm:grid-cols-2">
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        LinkedIn Profile URL
                      </label>
                      <input
                        type="text"
                        placeholder="linkedin.com/in/username"
                        value={doc.personalInfo.linkedin}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, linkedin: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Portfolio / Website (Optional)
                      </label>
                      <input
                        type="text"
                        placeholder="https://github.com/username"
                        value={doc.personalInfo.website}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, website: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                  </div>

                  <div className="grid gap-3 sm:grid-cols-2">
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Address / City (Optional)
                      </label>
                      <input
                        type="text"
                        placeholder="Jakarta / Surabaya, Indonesia"
                        value={doc.personalInfo.address}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, address: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Headline Role (Optional)
                      </label>
                      <input
                        type="text"
                        placeholder="Full Stack Engineer"
                        value={doc.personalInfo.title}
                        onChange={(e) =>
                          setDoc((p) => ({
                            ...p,
                            personalInfo: { ...p.personalInfo, title: e.target.value },
                          }))
                        }
                        className={inputClass}
                      />
                    </div>
                  </div>

                  <div>
                    <div className="flex items-center justify-between">
                      <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                        Short Professional Summary
                      </label>
                      <span className="font-mono text-[11px] text-[var(--ink-muted)] tabular-nums">
                        {doc.personalInfo.summary.trim().length} chars · 100–350 recommended
                      </span>
                    </div>
                    <textarea
                      rows={4}
                      value={doc.personalInfo.summary}
                      onChange={(e) =>
                        setDoc((p) => ({
                          ...p,
                          personalInfo: { ...p.personalInfo, summary: e.target.value },
                        }))
                      }
                      className={`${inputClass} leading-relaxed`}
                    />
                  </div>

                  {/* Portrait Photo Upload */}
                  <div>
                    <label className="block text-xs font-medium text-[var(--ink-secondary)]">
                      Photo (Optional)
                    </label>
                    <input
                      ref={photoInputRef}
                      type="file"
                      accept="image/png,image/jpeg,image/webp"
                      className="hidden"
                      onChange={(e) => {
                        handlePhotoUpload(e.target.files?.[0]);
                        e.target.value = "";
                      }}
                    />
                    {doc.personalInfo.photoDataUrl ? (
                      <div className="mt-1.5 inline-flex items-start gap-3 rounded-xl border border-dashed border-[var(--border-strong)] bg-[var(--bg-band)] p-3">
                        <img
                          src={doc.personalInfo.photoDataUrl}
                          alt="Uploaded CV portrait"
                          className="h-28 w-[90px] rounded-md border border-[var(--border-subtle)] object-cover object-top"
                        />
                        <div className="flex flex-col gap-2">
                          <button
                            type="button"
                            onClick={() => photoInputRef.current?.click()}
                            className="cm-btn rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2.5 py-1 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--border-strong)]"
                          >
                            Replace photo
                          </button>
                          <button
                            type="button"
                            onClick={() =>
                              setDoc((p) => ({
                                ...p,
                                personalInfo: { ...p.personalInfo, photoDataUrl: "" },
                              }))
                            }
                            className="cm-btn inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-medium text-red-600 hover:bg-red-500/10"
                          >
                            <Trash2 className="h-3.5 w-3.5" /> Remove photo
                          </button>
                        </div>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => photoInputRef.current?.click()}
                        className="cm-btn mt-1.5 flex w-full items-center justify-center gap-2 rounded-xl border border-dashed border-[var(--border-strong)] bg-[var(--bg-band)] px-4 py-4 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--ink-muted)] hover:text-[var(--ink-primary)]"
                      >
                        <Upload className="h-4 w-4 text-[var(--ink-muted)]" />
                        Upload portrait photo (JPG / PNG / WebP · auto-cropped 4:5)
                      </button>
                    )}
                    {photoError && <p className="mt-1 text-xs text-red-500">{photoError}</p>}
                  </div>
                </div>
              </>
            )}

            {step === "experience" && (
              <>
                <SectionHeader
                  title={doc.sectionTitles.experience}
                  subtitle="Start from your most recent role or internship."
                  onTitleChange={(v) => updateTitle("experience", v)}
                  showTips={showTips}
                  onToggleTips={() => setShowTips((v) => !v)}
                  tips={STEP_TIPS.experience}
                />

                <div className="space-y-3">
                  {doc.experience.map((exp, idx) => (
                    <AccordionCard
                      key={exp.id}
                      index={idx}
                      total={doc.experience.length}
                      title={
                        [exp.position, exp.company].filter(Boolean).join(" at ") ||
                        `Experience ${idx + 1}`
                      }
                      subtitle={[exp.location, exp.startYear || ""].filter(Boolean).join(" · ")}
                      expanded={Boolean(openIds[exp.id])}
                      onToggleExpand={() => toggleOpen(exp.id)}
                      onMove={(from, to) =>
                        setDoc((p) => ({ ...p, experience: moveItem(p.experience, from, to) }))
                      }
                      onDelete={() =>
                        setDoc((p) => ({
                          ...p,
                          experience: p.experience.filter((x) => x.id !== exp.id),
                        }))
                      }
                      dragIndex={dragIndex}
                      onDragStart={setDragIndex}
                      onDragEnd={() => setDragIndex(null)}
                    >
                      <div className="grid gap-3 sm:grid-cols-2">
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Company Name
                          </label>
                          <input
                            type="text"
                            value={exp.company}
                            onChange={(e) => updateExp(exp.id, { company: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Role / Position
                          </label>
                          <input
                            type="text"
                            value={exp.position}
                            onChange={(e) => updateExp(exp.id, { position: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Company Location (City, Country)
                        </label>
                        <input
                          type="text"
                          value={exp.location}
                          onChange={(e) => updateExp(exp.id, { location: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Company Description (Optional)
                        </label>
                        <input
                          type="text"
                          placeholder="Short one-line context about the company"
                          value={exp.companyDescription}
                          onChange={(e) => updateExp(exp.id, { companyDescription: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div className="grid gap-3 sm:grid-cols-2">
                        <MonthYearPicker
                          label="Start Date"
                          month={exp.startMonth}
                          year={exp.startYear}
                          onMonthChange={(v) => updateExp(exp.id, { startMonth: v })}
                          onYearChange={(v) => updateExp(exp.id, { startYear: v })}
                        />
                        <MonthYearPicker
                          label="End Date"
                          disabled={exp.isCurrent}
                          month={exp.endMonth}
                          year={exp.endYear}
                          onMonthChange={(v) => updateExp(exp.id, { endMonth: v })}
                          onYearChange={(v) => updateExp(exp.id, { endYear: v })}
                        />
                      </div>

                      <label className="inline-flex cursor-pointer items-center gap-2 text-xs font-medium text-[var(--ink-primary)]">
                        <input
                          type="checkbox"
                          checked={exp.isCurrent}
                          onChange={(e) => updateExp(exp.id, { isCurrent: e.target.checked })}
                          className="h-4 w-4 rounded border-[var(--border-subtle)] accent-[var(--accent)]"
                        />
                        I currently work here
                      </label>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Responsibilities & Achievements (one bullet per line)
                        </label>
                        <textarea
                          rows={4}
                          value={exp.description}
                          onChange={(e) => updateExp(exp.id, { description: e.target.value })}
                          className={`${inputClass} text-xs leading-relaxed`}
                        />
                      </div>
                    </AccordionCard>
                  ))}

                  <button
                    type="button"
                    onClick={() => {
                      const id = uid();
                      setDoc((p) => ({
                        ...p,
                        experience: [
                          ...p.experience,
                          {
                            id,
                            company: "New Company",
                            position: "Software Engineer",
                            location: "Remote",
                            companyDescription: "",
                            startMonth: "Jan",
                            startYear: "2024",
                            endMonth: "",
                            endYear: "",
                            isCurrent: true,
                            description: "Shipped reliable user-facing features with measurable impact.",
                          },
                        ],
                      }));
                      setOpenIds((o) => ({ ...o, [id]: true }));
                    }}
                    className="cm-btn flex w-full items-center justify-center gap-1.5 rounded-xl border border-dashed border-[var(--border-strong)] py-2.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--ink-muted)] hover:bg-[var(--bg-subtle)]"
                  >
                    <Plus className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> Add experience
                  </button>
                </div>
              </>
            )}

            {step === "education" && (
              <>
                <SectionHeader
                  title={doc.sectionTitles.education}
                  subtitle="List your degrees or latest academic programs."
                  onTitleChange={(v) => updateTitle("education", v)}
                  showTips={showTips}
                  onToggleTips={() => setShowTips((v) => !v)}
                  tips={STEP_TIPS.education}
                />

                <div className="flex items-center justify-between rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-band)] px-3.5 py-2.5">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-[var(--ink-primary)]">
                      Move Education to first section
                    </span>
                    <HelpTip text="Places Education above Work Experience—recommended for students and fresh graduates." />
                  </div>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={doc.style.educationFirst}
                    onClick={() =>
                      handleStyleChange({ educationFirst: !doc.style.educationFirst })
                    }
                    className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${
                      doc.style.educationFirst ? "bg-[var(--accent)]" : "bg-neutral-300 dark:bg-neutral-700"
                    }`}
                  >
                    <span
                      className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                        doc.style.educationFirst ? "translate-x-6" : "translate-x-1"
                      }`}
                    />
                  </button>
                </div>

                <div className="space-y-3">
                  {doc.education.map((edu, idx) => (
                    <AccordionCard
                      key={edu.id}
                      index={idx}
                      total={doc.education.length}
                      title={
                        [edu.degree && `${edu.degree} Degree`, edu.institution]
                          .filter(Boolean)
                          .join(" in ") || `Education ${idx + 1}`
                      }
                      subtitle={[edu.fieldOfStudy, edu.gpa && `GPA ${edu.gpa}`].filter(Boolean).join(" · ")}
                      expanded={Boolean(openIds[edu.id])}
                      onToggleExpand={() => toggleOpen(edu.id)}
                      onMove={(from, to) =>
                        setDoc((p) => ({ ...p, education: moveItem(p.education, from, to) }))
                      }
                      onDelete={() =>
                        setDoc((p) => ({
                          ...p,
                          education: p.education.filter((x) => x.id !== edu.id),
                        }))
                      }
                      dragIndex={dragIndex}
                      onDragStart={setDragIndex}
                      onDragEnd={() => setDragIndex(null)}
                    >
                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          School / University
                        </label>
                        <input
                          type="text"
                          value={edu.institution}
                          onChange={(e) => updateEdu(edu.id, { institution: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Location (City, Country)
                        </label>
                        <input
                          type="text"
                          value={edu.location}
                          onChange={(e) => updateEdu(edu.id, { location: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div className="grid gap-3 sm:grid-cols-2">
                        <MonthYearPicker
                          label="Start Date"
                          month={edu.startMonth}
                          year={edu.startYear}
                          onMonthChange={(v) => updateEdu(edu.id, { startMonth: v })}
                          onYearChange={(v) => updateEdu(edu.id, { startYear: v })}
                        />
                        <MonthYearPicker
                          label="Graduation / End Date"
                          disabled={edu.isCurrent}
                          month={edu.endMonth}
                          year={edu.endYear}
                          onMonthChange={(v) => updateEdu(edu.id, { endMonth: v })}
                          onYearChange={(v) => updateEdu(edu.id, { endYear: v })}
                        />
                      </div>

                      <div className="grid gap-3 sm:grid-cols-2">
                        <div>
                          <label className="mb-1 block text-xs font-medium text-[var(--ink-secondary)]">
                            Degree
                          </label>
                          <BasecoatSelect
                            size="sm"
                            ariaLabel="Degree level"
                            value={edu.degree}
                            onChange={(v) => updateEdu(edu.id, { degree: v })}
                            options={DEGREE_OPTIONS.map((d) => ({ value: d, label: d }))}
                          />
                        </div>
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Field / Major Description
                          </label>
                          <input
                            type="text"
                            value={edu.fieldOfStudy}
                            onChange={(e) => updateEdu(edu.id, { fieldOfStudy: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                      </div>

                      <div className="grid gap-3 sm:grid-cols-2">
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            GPA (Optional)
                          </label>
                          <input
                            type="text"
                            placeholder="3.75"
                            value={edu.gpa}
                            onChange={(e) => updateEdu(edu.id, { gpa: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Max GPA
                          </label>
                          <input
                            type="text"
                            placeholder="4.00"
                            value={edu.maxGpa}
                            onChange={(e) => updateEdu(edu.id, { maxGpa: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Activities & Achievements (one bullet per line)
                        </label>
                        <textarea
                          rows={3}
                          value={edu.description}
                          onChange={(e) => updateEdu(edu.id, { description: e.target.value })}
                          className={`${inputClass} text-xs leading-relaxed`}
                        />
                      </div>
                    </AccordionCard>
                  ))}

                  <button
                    type="button"
                    onClick={() => {
                      const id = uid();
                      setDoc((p) => ({
                        ...p,
                        education: [
                          ...p.education,
                          {
                            id,
                            institution: "Institute of Technology",
                            location: "Indonesia",
                            degree: "Bachelor",
                            fieldOfStudy: "Computer Science",
                            startMonth: "Aug",
                            startYear: "2020",
                            endMonth: "Jul",
                            endYear: "2024",
                            isCurrent: false,
                            gpa: "",
                            maxGpa: "4.00",
                            description: "",
                          },
                        ],
                      }));
                      setOpenIds((o) => ({ ...o, [id]: true }));
                    }}
                    className="cm-btn flex w-full items-center justify-center gap-1.5 rounded-xl border border-dashed border-[var(--border-strong)] py-2.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--ink-muted)] hover:bg-[var(--bg-subtle)]"
                  >
                    <Plus className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> Add education
                  </button>
                </div>
              </>
            )}

            {step === "organizations" && (
              <>
                <SectionHeader
                  title={doc.sectionTitles.organizations}
                  subtitle="Add campus clubs, communities, or volunteer leadership."
                  onTitleChange={(v) => updateTitle("organizations", v)}
                  showTips={showTips}
                  onToggleTips={() => setShowTips((v) => !v)}
                  tips={STEP_TIPS.organizations}
                />

                <div className="space-y-3">
                  {doc.organizations.map((org, idx) => (
                    <AccordionCard
                      key={org.id}
                      index={idx}
                      total={doc.organizations.length}
                      title={
                        [org.position, org.organization].filter(Boolean).join(" — ") ||
                        `Organization ${idx + 1}`
                      }
                      subtitle={org.location}
                      expanded={Boolean(openIds[org.id])}
                      onToggleExpand={() => toggleOpen(org.id)}
                      onMove={(from, to) =>
                        setDoc((p) => ({
                          ...p,
                          organizations: moveItem(p.organizations, from, to),
                        }))
                      }
                      onDelete={() =>
                        setDoc((p) => ({
                          ...p,
                          organizations: p.organizations.filter((x) => x.id !== org.id),
                        }))
                      }
                      dragIndex={dragIndex}
                      onDragStart={setDragIndex}
                      onDragEnd={() => setDragIndex(null)}
                    >
                      <div className="grid gap-3 sm:grid-cols-2">
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Organization / Event Name
                          </label>
                          <input
                            type="text"
                            value={org.organization}
                            onChange={(e) => updateOrg(org.id, { organization: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                        <div>
                          <label className="text-xs font-medium text-[var(--ink-secondary)]">
                            Position / Title
                          </label>
                          <input
                            type="text"
                            value={org.position}
                            onChange={(e) => updateOrg(org.id, { position: e.target.value })}
                            className={inputClass}
                          />
                        </div>
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Organization Description (Optional)
                        </label>
                        <input
                          type="text"
                          value={org.organizationDescription}
                          onChange={(e) =>
                            updateOrg(org.id, { organizationDescription: e.target.value })
                          }
                          className={inputClass}
                        />
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Location / Event Format
                        </label>
                        <input
                          type="text"
                          value={org.location}
                          onChange={(e) => updateOrg(org.id, { location: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div className="grid gap-3 sm:grid-cols-2">
                        <MonthYearPicker
                          label="Start Date"
                          month={org.startMonth}
                          year={org.startYear}
                          onMonthChange={(v) => updateOrg(org.id, { startMonth: v })}
                          onYearChange={(v) => updateOrg(org.id, { startYear: v })}
                        />
                        <MonthYearPicker
                          label="End Date"
                          disabled={org.isCurrent}
                          month={org.endMonth}
                          year={org.endYear}
                          onMonthChange={(v) => updateOrg(org.id, { endMonth: v })}
                          onYearChange={(v) => updateOrg(org.id, { endYear: v })}
                        />
                      </div>

                      <label className="inline-flex cursor-pointer items-center gap-2 text-xs font-medium text-[var(--ink-primary)]">
                        <input
                          type="checkbox"
                          checked={org.isCurrent}
                          onChange={(e) => updateOrg(org.id, { isCurrent: e.target.checked })}
                          className="h-4 w-4 rounded border-[var(--border-subtle)] accent-[var(--accent)]"
                        />
                        Currently active here
                      </label>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Responsibilities & Impact (one bullet per line)
                        </label>
                        <textarea
                          rows={3}
                          value={org.description}
                          onChange={(e) => updateOrg(org.id, { description: e.target.value })}
                          className={`${inputClass} text-xs leading-relaxed`}
                        />
                      </div>
                    </AccordionCard>
                  ))}

                  <button
                    type="button"
                    onClick={() => {
                      const id = uid();
                      setDoc((p) => ({
                        ...p,
                        organizations: [
                          ...p.organizations,
                          {
                            id,
                            organization: "Developer Community",
                            position: "Core Lead",
                            location: "Surabaya",
                            organizationDescription: "",
                            startMonth: "Jan",
                            startYear: "2023",
                            endMonth: "Dec",
                            endYear: "2023",
                            isCurrent: false,
                            description: "Coordinated workshops for 80+ student members.",
                          },
                        ],
                      }));
                      setOpenIds((o) => ({ ...o, [id]: true }));
                    }}
                    className="cm-btn flex w-full items-center justify-center gap-1.5 rounded-xl border border-dashed border-[var(--border-strong)] py-2.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--ink-muted)] hover:bg-[var(--bg-subtle)]"
                  >
                    <Plus className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> Add organization
                  </button>
                </div>
              </>
            )}

            {step === "other" && (
              <>
                <SectionHeader
                  title={doc.sectionTitles.other}
                  subtitle="Add Hard Skills, Soft Skills, Projects, Certifications, and Languages."
                  onTitleChange={(v) => updateTitle("other", v)}
                  showTips={showTips}
                  onToggleTips={() => setShowTips((v) => !v)}
                  tips={STEP_TIPS.other}
                />

                <div className="space-y-3">
                  {doc.other.map((item, idx) => (
                    <AccordionCard
                      key={item.id}
                      index={idx}
                      total={doc.other.length}
                      title={item.customLabel.trim() || item.category || `Entry ${idx + 1}`}
                      subtitle={item.year ? `${item.year} · ${item.description.slice(0, 48)}` : item.description.slice(0, 56)}
                      expanded={Boolean(openIds[item.id])}
                      onToggleExpand={() => toggleOpen(item.id)}
                      onMove={(from, to) =>
                        setDoc((p) => ({ ...p, other: moveItem(p.other, from, to) }))
                      }
                      onDelete={() =>
                        setDoc((p) => ({
                          ...p,
                          other: p.other.filter((x) => x.id !== item.id),
                        }))
                      }
                      dragIndex={dragIndex}
                      onDragStart={setDragIndex}
                      onDragEnd={() => setDragIndex(null)}
                    >
                      <div className="grid gap-3 sm:grid-cols-2">
                        <div>
                          <label className="mb-1 block text-xs font-medium text-[var(--ink-secondary)]">
                            Category / Activity
                          </label>
                          <BasecoatSelect
                            size="sm"
                            ariaLabel="Category"
                            value={item.category}
                            onChange={(v) => updateOther(item.id, { category: v })}
                            options={OTHER_CATEGORIES.map((c) => ({ value: c, label: c }))}
                          />
                        </div>
                        <div>
                          <label className="mb-1 block text-xs font-medium text-[var(--ink-secondary)]">
                            Year (Optional)
                          </label>
                          <BasecoatSelect
                            size="sm"
                            ariaLabel="Year"
                            placeholder="Select year"
                            value={item.year}
                            onChange={(v) => updateOther(item.id, { year: v })}
                            options={[
                              { value: "", label: "None" },
                              ...YEARS.map((y) => ({ value: y, label: y })),
                            ]}
                          />
                        </div>
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Custom Label / Project Name (Optional)
                        </label>
                        <input
                          type="text"
                          placeholder="Project DokterEMR (leave blank to use category)"
                          value={item.customLabel}
                          onChange={(e) => updateOther(item.id, { customLabel: e.target.value })}
                          className={inputClass}
                        />
                      </div>

                      <div>
                        <label className="text-xs font-medium text-[var(--ink-secondary)]">
                          Details / Skills List
                        </label>
                        <textarea
                          rows={3}
                          value={item.description}
                          onChange={(e) => updateOther(item.id, { description: e.target.value })}
                          className={`${inputClass} text-xs leading-relaxed`}
                        />
                      </div>
                    </AccordionCard>
                  ))}

                  <button
                    type="button"
                    onClick={() => {
                      const id = uid();
                      setDoc((p) => ({
                        ...p,
                        other: [
                          ...p.other,
                          {
                            id,
                            category: "Project",
                            customLabel: "New Project",
                            year: "2025",
                            description: "Built production workflow with clean metrics.",
                          },
                        ],
                      }));
                      setOpenIds((o) => ({ ...o, [id]: true }));
                    }}
                    className="cm-btn flex w-full items-center justify-center gap-1.5 rounded-xl border border-dashed border-[var(--border-strong)] py-2.5 text-xs font-medium text-[var(--ink-primary)] hover:border-[var(--ink-muted)] hover:bg-[var(--bg-subtle)]"
                  >
                    <Plus className="h-3.5 w-3.5 text-[var(--ink-muted)]" /> Add skill or project entry
                  </button>
                </div>
              </>
            )}

            {step === "review" && (
              <div className="space-y-5">
                <div>
                  <h2 className="text-base font-semibold tracking-tight text-[var(--ink-primary)]">
                    Review Your CV
                  </h2>
                  <p className="mt-0.5 text-xs text-[var(--ink-secondary)]">
                    Verify your ATS readiness score, adjust styling on the right, and export your PDF.
                  </p>
                </div>

                <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-[var(--border-subtle)] bg-[var(--bg-band)] p-4">
                  <svg width="72" height="72" viewBox="0 0 72 72" className="shrink-0">
                    <circle
                      cx="36"
                      cy="36"
                      r="30"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="5"
                      className="text-[var(--border-subtle)]"
                    />
                    <circle
                      cx="36"
                      cy="36"
                      r="30"
                      fill="none"
                      stroke={ats.score >= 80 ? "var(--state-ok-fg)" : "currentColor"}
                      strokeWidth="5"
                      strokeLinecap="round"
                      strokeDasharray={`${(ats.score / 100) * 188.5} 188.5`}
                      transform="rotate(-90 36 36)"
                      className={
                        ats.score >= 80
                          ? ""
                          : ats.score >= 60
                            ? "text-amber-500"
                            : "text-red-500"
                      }
                    />
                    <text
                      x="36"
                      y="41"
                      textAnchor="middle"
                      className="fill-[var(--ink-primary)] text-sm font-medium tabular-nums"
                    >
                      {ats.score}%
                    </text>
                  </svg>

                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-semibold text-[var(--ink-primary)]">
                      Overall ATS Score: {ats.score}%
                    </p>
                    <p className="mt-0.5 text-xs text-[var(--ink-secondary)]">
                      {ats.score >= 80
                        ? "Strong ATS structure—clean single-column sections and measurable highlights."
                        : "Address highlighted items to sharpen keyword clarity and impact."}
                    </p>
                  </div>
                </div>

                <ul className="space-y-2">
                  {ats.checks.map((c) => (
                    <li
                      key={c.id}
                      className="flex items-start justify-between gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-band)] px-3.5 py-2.5"
                    >
                      <div className="flex items-start gap-2.5">
                        {c.ok ? (
                          <CheckCircle2
                            className="mt-0.5 h-4 w-4 shrink-0"
                            style={{ color: "var(--state-ok-fg)" }}
                          />
                        ) : (
                          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
                        )}
                        <div>
                          <p className="text-xs font-medium text-[var(--ink-primary)]">{c.label}</p>
                          {!c.ok && c.hint && (
                            <p className="mt-0.5 text-[11px] text-[var(--ink-muted)]">{c.hint}</p>
                          )}
                        </div>
                      </div>
                      <span className="shrink-0 font-mono text-[11px] text-[var(--ink-muted)] tabular-nums">
                        {c.points}/{c.maxPoints}
                      </span>
                    </li>
                  ))}
                </ul>

                <div className="flex flex-wrap gap-3 pt-1">
                  <button
                    type="button"
                    onClick={handleSaveDraft}
                    className="cm-btn inline-flex flex-1 items-center justify-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-4 py-2.5 text-xs font-semibold text-[var(--ink-primary)] hover:border-[var(--border-strong)] hover:bg-[var(--bg-subtle)]"
                  >
                    <BookmarkCheck className="h-4 w-4 text-[var(--ink-muted)]" /> Save Draft
                  </button>
                  <button
                    type="button"
                    disabled={busy || !doc.personalInfo.fullName.trim()}
                    onClick={handleGeneratePdf}
                    className="cm-btn inline-flex flex-1 items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-4 py-2.5 text-xs font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
                  >
                    {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
                    {busy ? "Generating PDF…" : "Finish & Download PDF"}
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Sticky Frosted Form Footer (cmpro DashboardFormLayout pattern) */}
          <div className="sticky bottom-0 z-20 -mx-5 -mb-5 mt-5 flex items-center justify-between rounded-b-2xl border-t border-[var(--border-subtle)] bg-[var(--bg-surface)]/85 px-5 py-3.5 backdrop-blur-md sm:-mx-6 sm:-mb-6 sm:px-6">
            <button
              type="button"
              disabled={stepIndex <= 0}
              onClick={() => stepIndex > 0 && goStep(STEP_ITEMS[stepIndex - 1].id)}
              className="cm-btn inline-flex items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)] disabled:opacity-40"
            >
              <ChevronLeft className="h-3.5 w-3.5" /> Back
            </button>

            {stepIndex < STEP_ITEMS.length - 1 && (
              <button
                type="button"
                onClick={() => goStep(STEP_ITEMS[stepIndex + 1].id)}
                className="cm-btn inline-flex items-center gap-1.5 rounded-xl bg-[var(--accent)] px-4 py-2 text-xs font-semibold text-white hover:bg-[var(--accent-hover)]"
              >
                Save &amp; Continue <ChevronRight className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>

        {/* Right: Formatting Toolbar + ATS Strip + Live A4 Sheet */}
        <ResumePreview
          doc={doc}
          onStyleChange={handleStyleChange}
          onExportPdf={handleGeneratePdf}
          busy={busy}
          error={error}
          result={result}
          ats={ats}
        />
      </div>
    </ToolShell>
  );
}
