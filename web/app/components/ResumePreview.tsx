import { useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Download,
  Loader2,
} from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { ResultBanner } from "~/components/ResultBanner";
import type { ToolResponse } from "~/lib/api";
import {
  formatRange,
  SECTION_DEFAULTS_EN,
  SECTION_DEFAULTS_ID,
  type AtsScoreResult,
  type ResumeDocument,
  type ResumeStyle,
} from "~/lib/resume-types";

interface ResumePreviewProps {
  doc: ResumeDocument;
  onStyleChange: (patch: Partial<ResumeStyle>) => void;
  onExportPdf: () => void;
  busy: boolean;
  error: string | null;
  result: ToolResponse | null;
  ats: AtsScoreResult;
}

function splitBullets(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim().replace(/^[•\-*]\s*/, ""))
    .filter(Boolean);
}

export function ResumePreview({
  doc,
  onStyleChange,
  onExportPdf,
  busy,
  error,
  result,
  ats,
}: ResumePreviewProps) {
  const [showScoreDetails, setShowScoreDetails] = useState(false);
  const { personalInfo, experience, education, organizations, other, sectionTitles, style } = doc;
  const isId = style.language === "id";
  const defaults = isId ? SECTION_DEFAULTS_ID : SECTION_DEFAULTS_EN;

  const fontStack =
    style.font === "times"
      ? "'Times New Roman', Georgia, serif"
      : style.font === "courier"
        ? "'Courier New', Courier, monospace"
        : "'Inter', Arial, Helvetica, sans-serif";

  const marginClass =
    style.margin === "small" ? "p-5" : style.margin === "large" ? "p-9" : "p-7";

  const bodyPx = Number((style.fontSize * 1.1).toFixed(1));
  const headingPx = Number((bodyPx * 1.06).toFixed(1));
  const namePx = Number((bodyPx * 1.48).toFixed(1));
  const subPx = Number((bodyPx * 0.88).toFixed(1));

  const isModern = style.headerStyle === "modern";
  const accentHex = isModern ? style.accentColor || "#b7281e" : "#161210";
  const justifyClass = style.alignment === "justify" ? "text-justify" : "text-left";

  const eduTitle = sectionTitles.education.trim() || defaults.education;
  const expTitle = sectionTitles.experience.trim() || defaults.experience;
  const orgTitle = sectionTitles.organizations.trim() || defaults.organizations;
  const otherTitle = sectionTitles.other.trim() || defaults.other;

  const contactItems = [
    personalInfo.phone.trim(),
    personalInfo.email.trim(),
    personalInfo.linkedin.trim(),
    personalInfo.website.trim(),
  ].filter(Boolean);

  const renderEducationSection = () => {
    if (!education.length) return null;
    return (
      <section className="mt-3">
        <h3
          className="border-b pb-0.5 font-bold tracking-tight"
          style={{
            fontSize: `${headingPx}px`,
            color: isModern ? accentHex : "#111318",
            borderColor: isModern ? accentHex : "#1f2329",
          }}
        >
          {eduTitle}
        </h3>
        <div className="mt-1.5 space-y-2">
          {education.map((ed) => {
            const dates = formatRange(
              ed.startMonth,
              ed.startYear,
              ed.endMonth,
              ed.endYear,
              ed.isCurrent,
              style.language,
            );
            const degParts: string[] = [];
            const deg = ed.degree.trim();
            const field = ed.fieldOfStudy.trim();
            if (deg && field) {
              const lower = field.toLowerCase();
              if (lower.startsWith("of ") || lower.startsWith("in ")) {
                degParts.push(`${deg} ${field}`);
              } else if (lower.startsWith("majoring") || deg.endsWith(".") || deg.length <= 4) {
                degParts.push(`${deg} — ${field}`);
              } else {
                degParts.push(`${deg} in ${field}`);
              }
            } else if (deg && deg !== ed.institution.trim()) {
              degParts.push(deg);
            } else if (field) {
              degParts.push(field);
            }
            if (ed.gpa.trim()) {
              degParts.push(ed.maxGpa.trim() ? `${ed.gpa.trim()}/${ed.maxGpa.trim()}` : `GPA ${ed.gpa.trim()}`);
            }
            const bullets = splitBullets(ed.description);
            return (
              <div key={ed.id}>
                <div className="flex items-baseline justify-between gap-2">
                  <div className="min-w-0">
                    <span className="font-bold text-neutral-900">
                      {ed.institution.trim() || ed.degree.trim() || "Institution"}
                    </span>
                    {ed.location.trim() && (
                      <span className="text-neutral-600"> - {ed.location.trim()}</span>
                    )}
                  </div>
                  {dates && <span className="shrink-0 text-neutral-800">{dates}</span>}
                </div>
                {degParts.length > 0 && (
                  <p className="italic text-neutral-800">{degParts.join(", ")}</p>
                )}
                {bullets.length > 0 && (
                  <ul className={`mt-0.5 list-disc space-y-0.5 pl-4 text-neutral-800 ${justifyClass}`}>
                    {bullets.map((b, i) => (
                      <li key={i}>{b}</li>
                    ))}
                  </ul>
                )}
              </div>
            );
          })}
        </div>
      </section>
    );
  };

  const renderExperienceSection = () => {
    if (!experience.length) return null;
    return (
      <section className="mt-3">
        <h3
          className="border-b pb-0.5 font-bold tracking-tight"
          style={{
            fontSize: `${headingPx}px`,
            color: isModern ? accentHex : "#111318",
            borderColor: isModern ? accentHex : "#1f2329",
          }}
        >
          {expTitle}
        </h3>
        <div className="mt-1.5 space-y-2">
          {experience.map((exp) => {
            const dates = formatRange(
              exp.startMonth,
              exp.startYear,
              exp.endMonth,
              exp.endYear,
              exp.isCurrent,
              style.language,
            );
            const bullets = splitBullets(exp.description);
            return (
              <div key={exp.id}>
                <div className="flex items-baseline justify-between gap-2">
                  <div className="min-w-0">
                    <span className="font-bold text-neutral-900">
                      {exp.company.trim() || exp.position.trim() || "Company"}
                    </span>
                    {exp.location.trim() && (
                      <span className="text-neutral-600"> - {exp.location.trim()}</span>
                    )}
                  </div>
                  {dates && <span className="shrink-0 text-neutral-800">{dates}</span>}
                </div>
                {exp.position.trim() && exp.position.trim() !== exp.company.trim() && (
                  <p className="italic text-neutral-900">{exp.position.trim()}</p>
                )}
                {exp.companyDescription.trim() && (
                  <p className={`italic text-neutral-600 ${justifyClass}`} style={{ fontSize: `${subPx}px` }}>
                    {exp.companyDescription.trim()}
                  </p>
                )}
                {bullets.length > 0 && (
                  <ul className={`mt-0.5 list-disc space-y-0.5 pl-4 text-neutral-800 ${justifyClass}`}>
                    {bullets.map((b, i) => (
                      <li key={i}>{b}</li>
                    ))}
                  </ul>
                )}
              </div>
            );
          })}
        </div>
      </section>
    );
  };

  return (
    <div className="space-y-3 lg:sticky lg:top-20">
      {/* Studio Style Controls */}
      <div className="surface-card rounded-2xl p-3">
        <div className="flex flex-wrap items-center gap-2">
          <div className="w-[152px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Resume font family"
              value={style.font}
              onChange={(v) => onStyleChange({ font: v as ResumeStyle["font"] })}
              options={[
                { value: "times", label: "Times New Roman" },
                { value: "arial", label: "Arial / Sans" },
                { value: "courier", label: "Courier Mono" },
              ]}
            />
          </div>
          <div className="w-[84px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Font size"
              value={String(style.fontSize)}
              onChange={(v) => onStyleChange({ fontSize: Number(v) })}
              options={[
                { value: "9.5", label: "9.5pt" },
                { value: "10", label: "10pt" },
                { value: "10.5", label: "10.5pt" },
                { value: "11", label: "11pt" },
                { value: "11.5", label: "11.5pt" },
              ]}
            />
          </div>
          <div className="w-[96px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Page margins"
              value={style.margin}
              onChange={(v) => onStyleChange({ margin: v as ResumeStyle["margin"] })}
              options={[
                { value: "small", label: "Small" },
                { value: "normal", label: "Normal" },
                { value: "large", label: "Large" },
              ]}
            />
          </div>
          <div className="w-[86px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Line spacing"
              value={String(style.lineSpacing)}
              onChange={(v) => onStyleChange({ lineSpacing: Number(v) })}
              options={[
                { value: "1.15", label: "1.15×" },
                { value: "1.25", label: "1.25×" },
                { value: "1.35", label: "1.35×" },
                { value: "1.45", label: "1.45×" },
              ]}
            />
          </div>
          <div className="w-[132px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Header style"
              value={style.headerStyle}
              onChange={(v) => onStyleChange({ headerStyle: v as ResumeStyle["headerStyle"] })}
              options={[
                { value: "ats", label: "Classic ATS" },
                { value: "modern", label: "Modern Accent" },
                { value: "centered", label: "Centered" },
              ]}
            />
          </div>
          <div className="w-[90px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="Text alignment"
              value={style.alignment}
              onChange={(v) => onStyleChange({ alignment: v as ResumeStyle["alignment"] })}
              options={[
                { value: "left", label: "Left" },
                { value: "justify", label: "Justify" },
              ]}
            />
          </div>
          <div className="w-[74px]">
            <BasecoatSelect
              size="sm"
              ariaLabel="CV Language"
              value={style.language}
              onChange={(v) => onStyleChange({ language: v as ResumeStyle["language"] })}
              options={[
                { value: "en", label: "EN" },
                { value: "id", label: "ID" },
              ]}
            />
          </div>

          <button
            type="button"
            disabled={busy || !personalInfo.fullName.trim()}
            onClick={onExportPdf}
            className="cm-btn ml-auto inline-flex h-8 items-center gap-1.5 rounded-xl bg-[var(--accent)] px-3.5 text-xs font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            {busy ? "Generating PDF…" : "Export Resume PDF"}
          </button>
        </div>

        {error && (
          <p className="mt-3 rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        {result && (
          <div className="mt-3">
            <ResultBanner blob={result.blob} filename={result.filename} />
          </div>
        )}
      </div>

      {/* Calm ATS Score Strip */}
      <div className="surface-card rounded-2xl px-4 py-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-3">
            <span
              style={
                ats.score >= 80
                  ? {
                      color: "var(--state-ok-fg)",
                      backgroundColor: "var(--state-ok-bg)",
                      borderColor: "var(--state-ok-border)",
                    }
                  : undefined
              }
              className={`inline-flex h-8 min-w-11 items-center justify-center rounded-lg border px-2 text-xs font-semibold tabular-nums ${
                ats.score >= 80
                  ? ""
                  : "border-[var(--border-subtle)] bg-[var(--bg-subtle)] text-[var(--ink-primary)]"
              }`}
            >
              {ats.score}%
            </span>
            <div>
              <p className="text-xs font-semibold text-[var(--ink-primary)]">
                Overall resume score: {ats.score}%
              </p>
              <p className="text-[11px] text-[var(--ink-muted)]">
                {ats.score >= 80
                  ? "Single-column structure with measurable highlights"
                  : "Complete flagged items to improve recruiter & ATS match"}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => setShowScoreDetails((s) => !s)}
            className="cm-btn inline-flex items-center gap-1 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-2.5 py-1 text-xs font-medium text-[var(--ink-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--ink-primary)]"
          >
            {showScoreDetails ? "Hide details" : "Details"}
            {showScoreDetails ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
          </button>
        </div>

        <div className="cm-accordion" data-open={showScoreDetails ? "true" : "false"}>
          <div>
            <ul className="mt-3 grid gap-1.5 border-t border-[var(--border-subtle)] pt-3 sm:grid-cols-2">
              {ats.checks.map((c) => (
                <li key={c.id} className="flex items-start gap-2 text-[11px]">
                  {c.ok ? (
                    <CheckCircle2
                      className="mt-0.5 h-3.5 w-3.5 shrink-0"
                      style={{ color: "var(--state-ok-fg)" }}
                    />
                  ) : (
                    <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-600 dark:text-amber-400" />
                  )}
                  <div>
                    <p className={c.ok ? "font-medium text-[var(--ink-primary)]" : "font-semibold text-[var(--ink-secondary)]"}>
                      {c.label}
                    </p>
                    {!c.ok && c.hint && <p className="text-[10px] text-[var(--ink-muted)]">{c.hint}</p>}
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>

      {/* The Bright Moment: A4 Sheet Mounted on Neutral Studio Mat */}
      <div className="rounded-2xl border border-[var(--border-subtle)] bg-[var(--bg-subtle)] p-3.5 sm:p-4">
        <div
          className={`min-h-[640px] rounded-lg border border-neutral-200/80 bg-white text-neutral-900 shadow-[0_1px_2px_rgba(0,0,0,0.06),0_12px_32px_-12px_rgba(0,0,0,0.14)] ${marginClass}`}
          style={{
            fontFamily: fontStack,
            fontSize: `${bodyPx}px`,
            lineHeight: style.lineSpacing,
          }}
        >
          {/* Header */}
          {personalInfo.photoDataUrl ? (
            <div className="flex items-start gap-3.5">
              <img
                src={personalInfo.photoDataUrl}
                alt="Candidate portrait"
                className="h-24 w-[76px] shrink-0 rounded-[2px] border border-neutral-200 object-cover object-top"
              />
              <div className="min-w-0 flex-1">
                <h2
                  className="font-bold tracking-tight uppercase text-neutral-950"
                  style={{ fontSize: `${namePx}px`, lineHeight: 1.15 }}
                >
                  {personalInfo.fullName || "YOUR NAME"}
                </h2>
                {personalInfo.title.trim() && (
                  <p
                    className="italic font-medium"
                    style={{ color: isModern ? accentHex : "#525866" }}
                  >
                    {personalInfo.title.trim()}
                  </p>
                )}
                {contactItems.length > 0 && (
                  <p className="mt-0.5 break-words text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                    {contactItems.join(" | ")}
                  </p>
                )}
                {personalInfo.address.trim() && (
                  <p className="text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                    {personalInfo.address.trim()}
                  </p>
                )}
                {personalInfo.summary.trim() && (
                  <p className={`mt-1.5 text-neutral-900 ${justifyClass}`}>
                    {personalInfo.summary.trim()}
                  </p>
                )}
              </div>
            </div>
          ) : isModern ? (
            <div className="border-b-2 pb-2.5" style={{ borderColor: accentHex }}>
              <h2
                className="font-bold tracking-tight uppercase text-neutral-950"
                style={{ fontSize: `${namePx}px`, lineHeight: 1.15 }}
              >
                {personalInfo.fullName || "YOUR NAME"}
              </h2>
              {personalInfo.title.trim() && (
                <p className="italic font-medium" style={{ color: accentHex }}>
                  {personalInfo.title.trim()}
                </p>
              )}
              {contactItems.length > 0 && (
                <p className="mt-0.5 break-words text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                  {contactItems.join(" | ")}
                </p>
              )}
              {personalInfo.address.trim() && (
                <p className="text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                  {personalInfo.address.trim()}
                </p>
              )}
              {personalInfo.summary.trim() && (
                <p className={`mt-1.5 text-neutral-900 ${justifyClass}`}>
                  {personalInfo.summary.trim()}
                </p>
              )}
            </div>
          ) : (
            <div className="text-center">
              <h2
                className="font-bold tracking-tight uppercase text-neutral-950"
                style={{ fontSize: `${namePx}px`, lineHeight: 1.15 }}
              >
                {personalInfo.fullName || "YOUR NAME"}
              </h2>
              {personalInfo.title.trim() && (
                <p className="italic text-neutral-600">{personalInfo.title.trim()}</p>
              )}
              {contactItems.length > 0 && (
                <p className="mt-0.5 break-words text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                  {contactItems.join(" | ")}
                </p>
              )}
              {personalInfo.address.trim() && (
                <p className="text-neutral-600" style={{ fontSize: `${subPx}px` }}>
                  {personalInfo.address.trim()}
                </p>
              )}
              {personalInfo.summary.trim() && (
                <p className={`mt-2 text-neutral-900 ${justifyClass}`}>
                  {personalInfo.summary.trim()}
                </p>
              )}
            </div>
          )}

          {/* Ordered Primary Sections */}
          {style.educationFirst ? (
            <>
              {renderEducationSection()}
              {renderExperienceSection()}
            </>
          ) : (
            <>
              {renderExperienceSection()}
              {renderEducationSection()}
            </>
          )}

          {/* Organizations */}
          {organizations.length > 0 && (
            <section className="mt-3">
              <h3
                className="border-b pb-0.5 font-bold tracking-tight"
                style={{
                  fontSize: `${headingPx}px`,
                  color: isModern ? accentHex : "#111318",
                  borderColor: isModern ? accentHex : "#1f2329",
                }}
              >
                {orgTitle}
              </h3>
              <div className="mt-1.5 space-y-2">
                {organizations.map((org) => {
                  const dates = formatRange(
                    org.startMonth,
                    org.startYear,
                    org.endMonth,
                    org.endYear,
                    org.isCurrent,
                    style.language,
                  );
                  const bullets = splitBullets(org.description);
                  return (
                    <div key={org.id}>
                      <div className="flex items-baseline justify-between gap-2">
                        <div className="min-w-0">
                          <span className="font-bold text-neutral-900">
                            {org.organization.trim() || org.position.trim() || "Organization"}
                          </span>
                          {org.location.trim() && (
                            <span className="text-neutral-600"> - {org.location.trim()}</span>
                          )}
                        </div>
                        {dates && <span className="shrink-0 text-neutral-800">{dates}</span>}
                      </div>
                      {org.position.trim() && org.position.trim() !== org.organization.trim() && (
                        <p className="italic text-neutral-900">{org.position.trim()}</p>
                      )}
                      {org.organizationDescription.trim() && (
                        <p className={`italic text-neutral-600 ${justifyClass}`} style={{ fontSize: `${subPx}px` }}>
                          {org.organizationDescription.trim()}
                        </p>
                      )}
                      {bullets.length > 0 && (
                        <ul className={`mt-0.5 list-disc space-y-0.5 pl-4 text-neutral-800 ${justifyClass}`}>
                          {bullets.map((b, i) => (
                            <li key={i}>{b}</li>
                          ))}
                        </ul>
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          {/* Skills & Other */}
          {other.length > 0 && (
            <section className="mt-3">
              <h3
                className="border-b pb-0.5 font-bold tracking-tight"
                style={{
                  fontSize: `${headingPx}px`,
                  color: isModern ? accentHex : "#111318",
                  borderColor: isModern ? accentHex : "#1f2329",
                }}
              >
                {otherTitle}
              </h3>
              <ul className={`mt-1.5 list-disc space-y-1 pl-4 text-neutral-800 ${justifyClass}`}>
                {other.map((item) => {
                  const label = (item.customLabel || item.category).trim();
                  const yearSuffix = item.year.trim() ? ` (${item.year.trim()})` : "";
                  return (
                    <li key={item.id}>
                      {label && <strong>{`${label}${yearSuffix}: `}</strong>}
                      <span>{item.description.trim()}</span>
                    </li>
                  );
                })}
              </ul>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
