export type StepId =
  | "personal"
  | "experience"
  | "education"
  | "organizations"
  | "other"
  | "review";

export interface PersonalInfo {
  fullName: string;
  title: string;
  phone: string;
  email: string;
  linkedin: string;
  website: string;
  address: string;
  summary: string;
  photoDataUrl: string;
}

export interface ExperienceItem {
  id: string;
  company: string;
  position: string;
  location: string;
  companyDescription: string;
  startMonth: string;
  startYear: string;
  endMonth: string;
  endYear: string;
  isCurrent: boolean;
  description: string;
}

export interface EducationItem {
  id: string;
  institution: string;
  location: string;
  degree: string;
  fieldOfStudy: string;
  startMonth: string;
  startYear: string;
  endMonth: string;
  endYear: string;
  isCurrent: boolean;
  gpa: string;
  maxGpa: string;
  description: string;
}

export interface OrganizationItem {
  id: string;
  organization: string;
  position: string;
  location: string;
  organizationDescription: string;
  startMonth: string;
  startYear: string;
  endMonth: string;
  endYear: string;
  isCurrent: boolean;
  description: string;
}

export interface OtherItem {
  id: string;
  category: string;
  customLabel: string;
  year: string;
  description: string;
}

export interface SectionTitles {
  experience: string;
  education: string;
  organizations: string;
  other: string;
}

export interface ResumeStyle {
  font: "times" | "arial" | "courier";
  fontSize: number;
  margin: "small" | "normal" | "large";
  lineSpacing: number;
  headerStyle: "ats" | "modern" | "centered";
  alignment: "left" | "justify";
  educationFirst: boolean;
  accentColor: string;
  language: "en" | "id";
}

export interface ResumeDocument {
  documentTitle: string;
  personalInfo: PersonalInfo;
  experience: ExperienceItem[];
  education: EducationItem[];
  organizations: OrganizationItem[];
  other: OtherItem[];
  sectionTitles: SectionTitles;
  style: ResumeStyle;
}

export interface AtsCheckItem {
  id: string;
  label: string;
  ok: boolean;
  points: number;
  maxPoints: number;
  hint?: string;
}

export interface AtsScoreResult {
  score: number;
  checks: AtsCheckItem[];
}

export const DRAFT_STORAGE_KEY = "520pdf:resume-draft:v2";

export const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
] as const;

export const YEARS: string[] = Array.from({ length: 2030 - 1975 + 1 }, (_, i) => String(2030 - i));

export const DEGREE_OPTIONS = [
  "Bachelor",
  "Diploma",
  "Master",
  "PhD",
  "Associate",
  "High School",
  "B.S.",
  "M.S.",
] as const;

export const OTHER_CATEGORIES = [
  "Hard Skills",
  "Soft Skills",
  "Languages",
  "Certifications",
  "Awards & Achievements",
  "Project",
  "Volunteering",
  "Publications",
] as const;

export const SECTION_DEFAULTS_EN: SectionTitles = {
  experience: "Work Experience",
  education: "Education",
  organizations: "Organisational Experience",
  other: "Skills, Achievements & Other Experience",
};

export const SECTION_DEFAULTS_ID: SectionTitles = {
  experience: "Pengalaman Kerja",
  education: "Jenjang Pendidikan",
  organizations: "Pengalaman Organisasi",
  other: "Keterampilan, Prestasi & Pengalaman Lain",
};

export const STEP_ITEMS: Array<{ id: StepId; number: number; label: string; shortLabel: string }> = [
  { id: "personal", number: 1, label: "Personal Info", shortLabel: "Personal" },
  { id: "experience", number: 2, label: "Experience", shortLabel: "Experience" },
  { id: "education", number: 3, label: "Education", shortLabel: "Education" },
  { id: "organizations", number: 4, label: "Organizations", shortLabel: "Organizations" },
  { id: "other", number: 5, label: "Skills & More", shortLabel: "Skills & More" },
  { id: "review", number: 6, label: "Review", shortLabel: "Review" },
];

export const STEP_TIPS: Record<Exclude<StepId, "review">, string[]> = {
  personal: [
    "Use an active email, phone number with country code, and a short LinkedIn/portfolio link.",
    "Keep your profile summary around 100–350 characters focusing on your core stack and domain impact.",
    "Portrait photo is optional—if added for ID/Asia applications, pick a clean neutral background.",
  ],
  experience: [
    "List your most recent role first and tick 'I currently work here' for active positions.",
    "Start each bullet with a strong action verb and quantify results (%, revenue, latency, users).",
    "Keep each company's short description to one sentence for quick recruiter scanning.",
  ],
  education: [
    "Students or recent graduates can enable 'Move Education to first section' right below.",
    "Include GPA when above 3.00/4.00 and add relevant thesis, capstone, or honors bullets.",
  ],
  organizations: [
    "Highlight committee leadership, event scale (participants, budget, partners), and concrete deliverables.",
    "Skip high school clubs once you have university or professional experience.",
  ],
  other: [
    "Group technical stack into Hard Skills, interpersonal strengths into Soft Skills, and key builds as Project entries.",
    "Keep skill keywords exact (e.g. React, TypeScript, Rust, PostgreSQL) so ATS search matches cleanly.",
  ],
};

export const uid = () => Math.random().toString(36).slice(2, 9);

export function createDefaultResume(): ResumeDocument {
  return {
    documentTitle: "CV Nadia Kusuma — ATS",
    personalInfo: {
      fullName: "Nadia Kusuma",
      title: "",
      phone: "+62 812 5550 520",
      email: "nadia.kusuma@example.com",
      linkedin: "linkedin.com/in/nadiakusuma",
      website: "nadiakusuma.dev",
      address: "Surabaya / Jakarta, Indonesia",
      summary:
        "Full-stack engineer with 3+ years of experience building scalable web applications and SaaS products. Proven track record shipping high-throughput APIs and accessible React interfaces with Next.js, TypeScript, Rust, and PostgreSQL.",
      photoDataUrl: "",
    },
    education: [
      {
        id: "edu-1",
        institution: "Electronic Engineering Polytechnic Institute of Surabaya",
        location: "Surabaya, Indonesia",
        degree: "Bachelor",
        fieldOfStudy: "Informatics Engineering",
        startMonth: "Jun",
        startYear: "2021",
        endMonth: "Oct",
        endYear: "2025",
        isCurrent: false,
        gpa: "3.78",
        maxGpa: "4.00",
        description:
          "Led engineering of an AI-assisted SaaS clinic Point of Sale capstone platform.\nRecipient of Merit Innovation Scholarship (2023–2024).",
      },
    ],
    experience: [
      {
        id: "exp-1",
        company: "PT Multi Medika Mahadata",
        position: "Full Stack Developer",
        location: "Surabaya, Indonesia",
        companyDescription: "Healthcare technology provider delivering cloud EMR and billing platforms.",
        startMonth: "Sep",
        startYear: "2025",
        endMonth: "",
        endYear: "",
        isCurrent: true,
        description:
          "Architected end-to-end web services using TypeScript, React, and PostgreSQL across 40+ clinic tenants.\nReduced report export p95 latency by 48% through streaming queries and background workers.\nLed frontend architecture reviews, automated E2E verification, and zero-downtime deployments.",
      },
      {
        id: "exp-2",
        company: "Eduskill Studio",
        position: "Frontend Web Developer (Freelance)",
        location: "Remote",
        companyDescription: "Interactive learning platform for cohort-based engineering bootcamps.",
        startMonth: "Aug",
        startYear: "2023",
        endMonth: "Jan",
        endYear: "2024",
        isCurrent: false,
        description:
          "Built responsive course player and progress analytics dashboards with 100% design fidelity.\nImproved Lighthouse performance score from 74 to 96 across core landing and student routes.",
      },
    ],
    organizations: [
      {
        id: "org-1",
        organization: "HIMIT Computing Alumni Forum",
        position: "Chief Executive",
        location: "Surabaya, Indonesia",
        organizationDescription: "Departmental student & alumni networking summit.",
        startMonth: "Sep",
        startYear: "2023",
        endMonth: "Nov",
        endYear: "2023",
        isCurrent: false,
        description:
          "Organized annual career & alumni sharing forum attracting 120+ student and industry attendees.\nCoordinated 5 specialized committees achieving 95% participant satisfaction.",
      },
    ],
    other: [
      {
        id: "oth-1",
        category: "Hard Skills",
        customLabel: "",
        year: "",
        description: "TypeScript, React, Next.js, Rust, Axum, PostgreSQL, Docker, Tailwind CSS, Git",
      },
      {
        id: "oth-2",
        category: "Soft Skills",
        customLabel: "",
        year: "",
        description: "Technical leadership, Cross-team communication, System design, Mentoring",
      },
      {
        id: "oth-3",
        category: "Project",
        customLabel: "Project DokterEMR",
        year: "2024",
        description: "Multi-tenant clinic electronic medical record & pharmacy inventory system.",
      },
    ],
    sectionTitles: { ...SECTION_DEFAULTS_EN },
    style: {
      font: "times",
      fontSize: 10,
      margin: "small",
      lineSpacing: 1.25,
      headerStyle: "ats",
      alignment: "justify",
      educationFirst: true,
      accentColor: "#1e293b",
      language: "en",
    },
  };
}

export function formatMonthYear(month: string, year: string): string {
  return [month.trim(), year.trim()].filter(Boolean).join(" ");
}

export function formatRange(
  startMonth: string,
  startYear: string,
  endMonth: string,
  endYear: string,
  isCurrent: boolean,
  language: "en" | "id",
): string {
  const present = language === "id" ? "Sekarang" : "Present";
  const s = formatMonthYear(startMonth, startYear);
  const e = isCurrent ? present : formatMonthYear(endMonth, endYear);
  if (!s && !e) return "";
  if (s && !e) return `${s} - ${present}`;
  if (!s && e) return e;
  return `${s} - ${e}`;
}

export function toResumePayload(doc: ResumeDocument) {
  const defaults = doc.style.language === "id" ? SECTION_DEFAULTS_ID : SECTION_DEFAULTS_EN;
  return {
    documentTitle: doc.documentTitle.trim() || "Resume",
    template: doc.style.headerStyle || "ats",
    personalInfo: {
      fullName: doc.personalInfo.fullName,
      title: doc.personalInfo.title,
      email: doc.personalInfo.email,
      phone: doc.personalInfo.phone,
      linkedin: doc.personalInfo.linkedin,
      website: doc.personalInfo.website,
      address: doc.personalInfo.address,
      summary: doc.personalInfo.summary,
    },
    experience: doc.experience.map((e) => ({
      company: e.company,
      position: e.position,
      location: e.location,
      companyDescription: e.companyDescription,
      startDate: formatMonthYear(e.startMonth, e.startYear),
      endDate: e.isCurrent ? "" : formatMonthYear(e.endMonth, e.endYear),
      isCurrent: e.isCurrent,
      description: e.description,
    })),
    education: doc.education.map((ed) => ({
      institution: ed.institution,
      location: ed.location,
      degree: ed.degree,
      fieldOfStudy: ed.fieldOfStudy,
      startDate: formatMonthYear(ed.startMonth, ed.startYear),
      endDate: ed.isCurrent ? "" : formatMonthYear(ed.endMonth, ed.endYear),
      isCurrent: ed.isCurrent,
      gpa: ed.gpa,
      maxGpa: ed.maxGpa,
      description: ed.description,
    })),
    organizations: doc.organizations.map((org) => ({
      organization: org.organization,
      position: org.position,
      location: org.location,
      organizationDescription: org.organizationDescription,
      startDate: formatMonthYear(org.startMonth, org.startYear),
      endDate: org.isCurrent ? "" : formatMonthYear(org.endMonth, org.endYear),
      isCurrent: org.isCurrent,
      description: org.description,
    })),
    other: doc.other.map((o) => ({
      category: (o.customLabel || o.category).trim(),
      year: o.year.trim(),
      description: o.description,
    })),
    sectionTitles: {
      experience: doc.sectionTitles.experience.trim() || defaults.experience,
      education: doc.sectionTitles.education.trim() || defaults.education,
      organizations: doc.sectionTitles.organizations.trim() || defaults.organizations,
      other: doc.sectionTitles.other.trim() || defaults.other,
    },
    style: doc.style,
  };
}

export function computeAtsScore(doc: ResumeDocument): AtsScoreResult {
  const { personalInfo, experience, education, organizations, other } = doc;
  const hasContact =
    personalInfo.fullName.trim().length >= 2 &&
    personalInfo.email.includes("@") &&
    personalInfo.phone.trim().length >= 6;
  const hasLinks =
    personalInfo.linkedin.trim().length > 4 || personalInfo.website.trim().length > 4;
  const summaryLen = personalInfo.summary.trim().length;
  const hasSummary = summaryLen >= 80 && summaryLen <= 420;
  const expBullets = experience.flatMap((e) =>
    e.description
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean),
  );
  const hasExperience = experience.length > 0 && expBullets.length >= 2;
  const metricMatches = [...expBullets, ...organizations.map((o) => o.description)]
    .join(" ")
    .match(/\b\d+(?:[.,]\d+)?(?:\s*%|\+|x|ms|s|k|m|rb|jt)?\b/gi);
  const hasMetrics = (metricMatches?.length ?? 0) >= 2;
  const hasEducation =
    education.length > 0 &&
    education.some((e) => e.institution.trim().length > 2 && e.degree.trim().length > 0);
  const hasSkillsOrOther =
    other.filter((o) => o.description.trim().length > 4).length >= 2 || organizations.length > 0;

  const checks: AtsCheckItem[] = [
    {
      id: "contact",
      label: "Essential contact details present",
      ok: hasContact,
      points: hasContact ? 15 : 0,
      maxPoints: 15,
      hint: "Include full name, valid email, and phone number.",
    },
    {
      id: "links",
      label: "LinkedIn or portfolio link included",
      ok: hasLinks,
      points: hasLinks ? 10 : 0,
      maxPoints: 10,
      hint: "Recruiters expect a LinkedIn URL or portfolio link.",
    },
    {
      id: "summary",
      label: "Concise profile summary (80–420 chars)",
      ok: hasSummary,
      points: hasSummary ? 15 : summaryLen > 0 ? 8 : 0,
      maxPoints: 15,
      hint: `Current summary is ${summaryLen} characters; aim for 100–350 characters.`,
    },
    {
      id: "experience",
      label: "Work experience with bullet highlights",
      ok: hasExperience,
      points: hasExperience ? 20 : experience.length > 0 ? 10 : 0,
      maxPoints: 20,
      hint: "Add at least one experience role with 2+ bullet points.",
    },
    {
      id: "metrics",
      label: "Quantified achievements (numbers / %)",
      ok: hasMetrics,
      points: hasMetrics ? 15 : (metricMatches?.length ?? 0) === 1 ? 8 : 0,
      maxPoints: 15,
      hint: "Add measurable outcomes such as latency %, team size, or scale numbers.",
    },
    {
      id: "education",
      label: "Education degree & institution complete",
      ok: hasEducation,
      points: hasEducation ? 15 : 0,
      maxPoints: 15,
      hint: "Specify institution name and degree.",
    },
    {
      id: "skills",
      label: "Skills, projects, or organizations filled",
      ok: hasSkillsOrOther,
      points: hasSkillsOrOther ? 10 : other.length > 0 ? 5 : 0,
      maxPoints: 10,
      hint: "List Hard Skills, Soft Skills, or notable Projects.",
    },
  ];

  const score = Math.min(
    100,
    checks.reduce((sum, c) => sum + c.points, 0),
  );
  return { score, checks };
}

function splitMonthYear(raw: string): { month: string; year: string } {
  const parts = raw.trim().split(/\s+/).filter(Boolean);
  if (parts.length >= 2) return { month: parts[0], year: parts[1] };
  if (parts.length === 1 && /^\d{4}$/.test(parts[0])) return { month: "", year: parts[0] };
  return { month: "", year: raw.trim() };
}

export function normalizeImportedResume(raw: unknown): ResumeDocument {
  const base = createDefaultResume();
  if (!raw || typeof raw !== "object") return base;
  const r = raw as Record<string, unknown>;
  const pi = (r.personalInfo && typeof r.personalInfo === "object"
    ? r.personalInfo
    : {}) as Record<string, unknown>;

  const str = (v: unknown, fallback = "") => (typeof v === "string" ? v : fallback);

  const personalInfo: PersonalInfo = {
    fullName: str(r.name) || str(pi.fullName, base.personalInfo.fullName),
    title: str(r.title) || str(pi.title, ""),
    phone: str(r.phone) || str(pi.phone, base.personalInfo.phone),
    email: str(r.email) || str(pi.email, base.personalInfo.email),
    linkedin: str(r.linkedin) || str(pi.linkedin, base.personalInfo.linkedin),
    website: str(r.website) || str(pi.website, base.personalInfo.website),
    address: str(r.address) || str(r.location) || str(pi.address, base.personalInfo.address),
    summary: str(r.summary) || str(pi.summary, base.personalInfo.summary),
    photoDataUrl: str(pi.photoDataUrl, ""),
  };

  const experience: ExperienceItem[] = Array.isArray(r.experience)
    ? r.experience.map((item) => {
        const it = (item || {}) as Record<string, unknown>;
        const start = it.startMonth
          ? { month: str(it.startMonth), year: str(it.startYear) }
          : splitMonthYear(str(it.startDate));
        const end = it.endMonth
          ? { month: str(it.endMonth), year: str(it.endYear) }
          : splitMonthYear(str(it.endDate));
        return {
          id: str(it.id) || uid(),
          company: str(it.company),
          position: str(it.position),
          location: str(it.location),
          companyDescription: str(it.companyDescription),
          startMonth: start.month,
          startYear: start.year,
          endMonth: end.month,
          endYear: end.year,
          isCurrent: Boolean(it.isCurrent),
          description: str(it.description),
        };
      })
    : base.experience;

  const education: EducationItem[] = Array.isArray(r.education)
    ? r.education.map((item) => {
        const it = (item || {}) as Record<string, unknown>;
        const start = it.startMonth
          ? { month: str(it.startMonth), year: str(it.startYear) }
          : splitMonthYear(str(it.startDate));
        const end = it.endMonth
          ? { month: str(it.endMonth), year: str(it.endYear) }
          : splitMonthYear(str(it.endDate));
        return {
          id: str(it.id) || uid(),
          institution: str(it.institution),
          location: str(it.location),
          degree: str(it.degree, "Bachelor"),
          fieldOfStudy: str(it.fieldOfStudy),
          startMonth: start.month,
          startYear: start.year,
          endMonth: end.month,
          endYear: end.year,
          isCurrent: Boolean(it.isCurrent),
          gpa: str(it.gpa),
          maxGpa: str(it.maxGpa, "4.00"),
          description: str(it.description),
        };
      })
    : base.education;

  const organizations: OrganizationItem[] = Array.isArray(r.organizations)
    ? r.organizations.map((item) => {
        const it = (item || {}) as Record<string, unknown>;
        const start = it.startMonth
          ? { month: str(it.startMonth), year: str(it.startYear) }
          : splitMonthYear(str(it.startDate));
        const end = it.endMonth
          ? { month: str(it.endMonth), year: str(it.endYear) }
          : splitMonthYear(str(it.endDate));
        return {
          id: str(it.id) || uid(),
          organization: str(it.organization),
          position: str(it.position),
          location: str(it.location),
          organizationDescription: str(it.organizationDescription),
          startMonth: start.month,
          startYear: start.year,
          endMonth: end.month,
          endYear: end.year,
          isCurrent: Boolean(it.isCurrent),
          description: str(it.description),
        };
      })
    : Array.isArray(r.organizations)
      ? []
      : base.organizations;

  const other: OtherItem[] = Array.isArray(r.other)
    ? r.other.map((item) => {
        const it = (item || {}) as Record<string, unknown>;
        const cat = str(it.category, "Hard Skills");
        const matched = (OTHER_CATEGORIES as readonly string[]).includes(cat);
        return {
          id: str(it.id) || uid(),
          category: matched ? cat : "Project",
          customLabel: str(it.customLabel) || (matched ? "" : cat),
          year: str(it.year),
          description: str(it.description),
        };
      })
    : base.other;

  const st = (r.style && typeof r.style === "object" ? r.style : {}) as Record<string, unknown>;
  const sec = (r.sectionTitles && typeof r.sectionTitles === "object"
    ? r.sectionTitles
    : {}) as Record<string, unknown>;

  const style: ResumeStyle = {
    font: st.font === "arial" || st.font === "courier" || st.font === "times" ? st.font : base.style.font,
    fontSize: typeof st.fontSize === "number" ? st.fontSize : base.style.fontSize,
    margin:
      st.margin === "small" || st.margin === "normal" || st.margin === "large"
        ? st.margin
        : base.style.margin,
    lineSpacing: typeof st.lineSpacing === "number" ? st.lineSpacing : base.style.lineSpacing,
    headerStyle:
      st.headerStyle === "ats" || st.headerStyle === "modern" || st.headerStyle === "centered"
        ? st.headerStyle
        : base.style.headerStyle,
    alignment: st.alignment === "left" || st.alignment === "justify" ? st.alignment : base.style.alignment,
    educationFirst:
      typeof st.educationFirst === "boolean" ? st.educationFirst : base.style.educationFirst,
    accentColor: str(st.accentColor, base.style.accentColor),
    language: st.language === "en" || st.language === "id" ? st.language : base.style.language,
  };

  return {
    documentTitle: str(r.documentTitle, base.documentTitle),
    personalInfo,
    experience,
    education,
    organizations,
    other,
    sectionTitles: {
      experience: str(sec.experience, base.sectionTitles.experience),
      education: str(sec.education, base.sectionTitles.education),
      organizations: str(sec.organizations, base.sectionTitles.organizations),
      other: str(sec.other, base.sectionTitles.other),
    },
    style,
  };
}

export function loadDraft(): ResumeDocument | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(DRAFT_STORAGE_KEY);
    if (!raw) return null;
    return normalizeImportedResume(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function saveDraft(doc: ResumeDocument): boolean {
  if (typeof window === "undefined") return false;
  try {
    window.localStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(doc));
    return true;
  } catch {
    return false;
  }
}

export function clearDraft(): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(DRAFT_STORAGE_KEY);
  } catch {
    // ignore storage quota / privacy mode
  }
}
