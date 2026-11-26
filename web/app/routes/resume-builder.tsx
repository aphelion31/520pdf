import { useState } from "react";
import { Download, Loader2, Plus, Trash2 } from "lucide-react";
import { BasecoatSelect } from "~/components/BasecoatSelect";
import { ResultBanner } from "~/components/ResultBanner";
import { ToolShell } from "~/components/ToolShell";
import { runTool, type ToolResponse } from "~/lib/api";

interface Experience {
  id: string;
  company: string;
  position: string;
  location: string;
  startDate: string;
  endDate: string;
  description: string;
}

interface Education {
  id: string;
  institution: string;
  degree: string;
  fieldOfStudy: string;
  startDate: string;
  endDate: string;
  description: string;
}

interface Skill {
  id: string;
  name: string;
  level: "Beginner" | "Intermediate" | "Advanced" | "Expert";
}

const uid = () => Math.random().toString(36).slice(2, 9);

export default function ResumeBuilder() {
  const [template, setTemplate] = useState<"minimal" | "modern" | "professional">("modern");
  const [tab, setTab] = useState<"personal" | "experience" | "education" | "skills">("personal");
  const [personal, setPersonal] = useState({
    fullName: "Nadia Kusuma",
    title: "Principal Systems Engineer",
    email: "nadia@520pdf.dev",
    phone: "+62 812 0000 520",
    address: "Jakarta / Singapore",
    summary:
      "Full-stack Rust and React Router engineer specializing in single-binary web systems, PDF processing pipelines, and high-throughput HTTP services.",
  });
  const [experience, setExperience] = useState<Experience[]>([
    {
      id: "e1",
      company: "520pdf Workbench",
      position: "Lead Systems Architect",
      location: "Remote",
      startDate: "2024",
      endDate: "Present",
      description:
        "Architected Axum 0.8 + lopdf + pdf-writer document engine serving 28 PDF tools.\nEliminated Python/Node runtime dependencies for core document rendering.",
    },
  ]);
  const [education, setEducation] = useState<Education[]>([
    {
      id: "ed1",
      institution: "Institute of Technology",
      degree: "B.S.",
      fieldOfStudy: "Computer Science",
      startDate: "2016",
      endDate: "2020",
      description: "Distributed Systems & Compilers",
    },
  ]);
  const [skills, setSkills] = useState<Skill[]>([
    { id: "s1", name: "Rust / Axum", level: "Expert" },
    { id: "s2", name: "TypeScript / React Router", level: "Expert" },
    { id: "s3", name: "PDF Internals (lopdf)", level: "Advanced" },
  ]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ToolResponse | null>(null);

  const payload = {
    template,
    personalInfo: personal,
    experience: experience.map(({ id: _id, ...rest }) => rest),
    education: education.map(({ id: _id, ...rest }) => rest),
    skills: skills.map(({ id: _id, ...rest }) => rest),
  };

  const handleGenerate = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await runTool("resume-builder", [], {
        resume_json: JSON.stringify(payload),
        template,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Resume PDF generation failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToolShell
      title="Resume Builder"
      description="Create structured resumes visually with live A4 preview and pure-Rust pdf-writer PDF export."
      category="Create"
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div className="flex gap-2">
          {(["minimal", "modern", "professional"] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTemplate(t)}
              className={`rounded-lg px-3.5 py-1.5 text-xs font-semibold capitalize transition-colors ${
                template === t
                  ? "bg-[var(--ink-primary)] text-[var(--bg-surface)]"
                  : "border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--ink-secondary)]"
              }`}
            >
              {t}
            </button>
          ))}
        </div>

        <div className="flex gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-surface)] p-1">
          {(["personal", "experience", "education", "skills"] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={`rounded-lg px-3 py-1 text-xs font-semibold capitalize ${
                tab === t ? "bg-[var(--accent)] text-white" : "text-[var(--ink-secondary)]"
              }`}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-[1fr_1.05fr]">
        <div className="surface-card space-y-4 rounded-2xl p-6">
          {tab === "personal" && (
            <div className="space-y-3">
              {[
                ["fullName", "Full name"],
                ["title", "Headline / Role"],
                ["email", "Email"],
                ["phone", "Phone"],
                ["address", "Location"],
              ].map(([k, label]) => (
                <div key={k}>
                  <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                    {label}
                  </label>
                  <input
                    type="text"
                    value={personal[k as keyof typeof personal]}
                    onChange={(e) => setPersonal((p) => ({ ...p, [k]: e.target.value }))}
                    className="mt-1 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3.5 py-2 text-sm"
                  />
                </div>
              ))}
              <div>
                <label className="block text-xs font-semibold text-[var(--ink-secondary)]">
                  Summary
                </label>
                <textarea
                  rows={4}
                  value={personal.summary}
                  onChange={(e) => setPersonal((p) => ({ ...p, summary: e.target.value }))}
                  className="mt-1 w-full rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-3 text-sm"
                />
              </div>
            </div>
          )}

          {tab === "experience" && (
            <div className="space-y-4">
              {experience.map((exp) => (
                <div
                  key={exp.id}
                  className="space-y-2.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-4"
                >
                  <div className="flex justify-between">
                    <span className="text-xs font-bold text-[var(--ink-muted)] uppercase">Role</span>
                    <button
                      type="button"
                      onClick={() => setExperience((list) => list.filter((x) => x.id !== exp.id))}
                      className="text-xs text-red-500 hover:underline"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <input
                      placeholder="Position"
                      value={exp.position}
                      onChange={(e) =>
                        setExperience((list) =>
                          list.map((x) => (x.id === exp.id ? { ...x, position: e.target.value } : x)),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="Company"
                      value={exp.company}
                      onChange={(e) =>
                        setExperience((list) =>
                          list.map((x) => (x.id === exp.id ? { ...x, company: e.target.value } : x)),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="Start"
                      value={exp.startDate}
                      onChange={(e) =>
                        setExperience((list) =>
                          list.map((x) => (x.id === exp.id ? { ...x, startDate: e.target.value } : x)),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="End"
                      value={exp.endDate}
                      onChange={(e) =>
                        setExperience((list) =>
                          list.map((x) => (x.id === exp.id ? { ...x, endDate: e.target.value } : x)),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                  </div>
                  <textarea
                    rows={3}
                    placeholder="Highlights (one per line)"
                    value={exp.description}
                    onChange={(e) =>
                      setExperience((list) =>
                        list.map((x) => (x.id === exp.id ? { ...x, description: e.target.value } : x)),
                      )
                    }
                    className="w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] p-2.5 text-xs"
                  />
                </div>
              ))}
              <button
                type="button"
                onClick={() =>
                  setExperience((list) => [
                    ...list,
                    {
                      id: uid(),
                      company: "New Company",
                      position: "Engineer",
                      location: "",
                      startDate: "2023",
                      endDate: "Present",
                      description: "Built reliable systems.",
                    },
                  ])
                }
                className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--accent)]"
              >
                <Plus className="h-3.5 w-3.5" /> Add experience
              </button>
            </div>
          )}

          {tab === "education" && (
            <div className="space-y-4">
              {education.map((edu) => (
                <div
                  key={edu.id}
                  className="space-y-2.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-page)] p-4"
                >
                  <div className="flex justify-between">
                    <span className="text-xs font-bold text-[var(--ink-muted)] uppercase">Degree</span>
                    <button
                      type="button"
                      onClick={() => setEducation((list) => list.filter((x) => x.id !== edu.id))}
                      className="text-xs text-red-500"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <input
                      placeholder="Institution"
                      value={edu.institution}
                      onChange={(e) =>
                        setEducation((list) =>
                          list.map((x) =>
                            x.id === edu.id ? { ...x, institution: e.target.value } : x,
                          ),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="Degree"
                      value={edu.degree}
                      onChange={(e) =>
                        setEducation((list) =>
                          list.map((x) => (x.id === edu.id ? { ...x, degree: e.target.value } : x)),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="Field of study"
                      value={edu.fieldOfStudy}
                      onChange={(e) =>
                        setEducation((list) =>
                          list.map((x) =>
                            x.id === edu.id ? { ...x, fieldOfStudy: e.target.value } : x,
                          ),
                        )
                      }
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                    <input
                      placeholder="Dates"
                      value={`${edu.startDate}-${edu.endDate}`}
                      onChange={(e) => {
                        const [s, en = ""] = e.target.value.split("-");
                        setEducation((list) =>
                          list.map((x) =>
                            x.id === edu.id ? { ...x, startDate: s.trim(), endDate: en.trim() } : x,
                          ),
                        );
                      }}
                      className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-1.5 text-sm"
                    />
                  </div>
                </div>
              ))}
              <button
                type="button"
                onClick={() =>
                  setEducation((list) => [
                    ...list,
                    {
                      id: uid(),
                      institution: "State University",
                      degree: "M.S.",
                      fieldOfStudy: "Software Systems",
                      startDate: "2020",
                      endDate: "2022",
                      description: "",
                    },
                  ])
                }
                className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--accent)]"
              >
                <Plus className="h-3.5 w-3.5" /> Add education
              </button>
            </div>
          )}

          {tab === "skills" && (
            <div className="space-y-3">
              {skills.map((sk) => (
                <div key={sk.id} className="flex items-center gap-2">
                  <input
                    value={sk.name}
                    onChange={(e) =>
                      setSkills((list) =>
                        list.map((x) => (x.id === sk.id ? { ...x, name: e.target.value } : x)),
                      )
                    }
                    className="flex-1 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-page)] px-3 py-1.5 text-sm"
                  />
                  <div className="w-36 shrink-0">
                    <BasecoatSelect
                      size="sm"
                      ariaLabel={`Skill level for ${sk.name}`}
                      value={sk.level}
                      onChange={(val) =>
                        setSkills((list) =>
                          list.map((x) =>
                            x.id === sk.id ? { ...x, level: val as Skill["level"] } : x,
                          ),
                        )
                      }
                      options={[
                        { value: "Beginner", label: "Beginner" },
                        { value: "Intermediate", label: "Intermediate" },
                        { value: "Advanced", label: "Advanced" },
                        { value: "Expert", label: "Expert" },
                      ]}
                    />
                  </div>
                  <button
                    type="button"
                    onClick={() => setSkills((list) => list.filter((x) => x.id !== sk.id))}
                    className="p-1.5 text-red-500"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
              <button
                type="button"
                onClick={() =>
                  setSkills((list) => [...list, { id: uid(), name: "Docker", level: "Advanced" }])
                }
                className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--accent)]"
              >
                <Plus className="h-3.5 w-3.5" /> Add skill
              </button>
            </div>
          )}

          {error && (
            <p className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-600">
              {error}
            </p>
          )}

          <button
            type="button"
            disabled={busy || !personal.fullName.trim()}
            onClick={handleGenerate}
            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--accent)] px-5 py-3 text-sm font-semibold text-white hover:bg-[var(--accent-hover)] disabled:opacity-50"
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            {busy ? "Generating PDF…" : "Export Resume PDF (Rust)"}
          </button>

          {result && <ResultBanner blob={result.blob} filename={result.filename} />}
        </div>

        <div className="surface-card rounded-2xl p-6">
          <p className="mb-3 text-xs font-semibold tracking-wider text-[var(--ink-muted)] uppercase">
            Live Resume Sheet ({template})
          </p>
          <div className="min-h-[520px] rounded-xl border border-[var(--border-subtle)] bg-white p-8 text-neutral-900 shadow-inner">
            <div
              className={`border-b-2 pb-4 ${
                template === "modern"
                  ? "border-[#e63228]"
                  : template === "professional"
                    ? "border-neutral-900 text-center"
                    : "border-neutral-300"
              }`}
            >
              <h2 className="text-2xl font-extrabold tracking-tight">{personal.fullName || "Your Name"}</h2>
              {personal.title && (
                <p className={`text-sm font-medium ${template === "modern" ? "text-[#e63228]" : "text-neutral-600"}`}>
                  {personal.title}
                </p>
              )}
              <p className="mt-1 text-xs text-neutral-500">
                {[personal.email, personal.phone, personal.address].filter(Boolean).join(" · ")}
              </p>
            </div>

            {personal.summary && (
              <div className="mt-4">
                <h3 className="text-[11px] font-bold tracking-wider text-neutral-500 uppercase">Profile</h3>
                <p className="mt-1 text-xs leading-relaxed text-neutral-700">{personal.summary}</p>
              </div>
            )}

            {experience.length > 0 && (
              <div className="mt-5">
                <h3 className="border-b border-neutral-200 pb-1 text-[11px] font-bold tracking-wider text-neutral-500 uppercase">
                  Experience
                </h3>
                <div className="mt-2 space-y-3">
                  {experience.map((exp) => (
                    <div key={exp.id}>
                      <div className="flex justify-between text-xs font-bold">
                        <span>
                          {exp.position} — {exp.company}
                        </span>
                        <span className="font-normal text-neutral-500">
                          {exp.startDate} – {exp.endDate}
                        </span>
                      </div>
                      <ul className="mt-1 space-y-0.5 pl-4 text-[11px] text-neutral-700 list-disc">
                        {exp.description
                          .split("\n")
                          .filter(Boolean)
                          .map((line, i) => (
                            <li key={i}>{line}</li>
                          ))}
                      </ul>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {education.length > 0 && (
              <div className="mt-5">
                <h3 className="border-b border-neutral-200 pb-1 text-[11px] font-bold tracking-wider text-neutral-500 uppercase">
                  Education
                </h3>
                <div className="mt-2 space-y-1.5">
                  {education.map((edu) => (
                    <div key={edu.id} className="flex justify-between text-xs">
                      <span>
                        <strong>
                          {edu.degree} {edu.fieldOfStudy}
                        </strong>{" "}
                        · {edu.institution}
                      </span>
                      <span className="text-neutral-500">
                        {edu.startDate} – {edu.endDate}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {skills.length > 0 && (
              <div className="mt-5">
                <h3 className="border-b border-neutral-200 pb-1 text-[11px] font-bold tracking-wider text-neutral-500 uppercase">
                  Skills
                </h3>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {skills.map((s) => (
                    <span
                      key={s.id}
                      className="rounded-full bg-neutral-100 px-2.5 py-0.5 text-[11px] font-medium text-neutral-800"
                    >
                      {s.name} ({s.level})
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </ToolShell>
  );
}
