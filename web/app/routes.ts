import { type RouteConfig, index, route } from "@react-router/dev/routes";

export default [
  index("routes/home.tsx"),
  route("drop", "routes/drop.tsx"),
  route("login", "routes/login.tsx"),
  route("merge-pdfs", "routes/merge-pdfs.tsx"),
  route("split-pdf", "routes/split-pdf.tsx"),
  route("compress-pdf", "routes/compress-pdf.tsx"),
  route("compress-image", "routes/compress-image.tsx"),
  route("pdf-to-image", "routes/pdf-to-image.tsx"),
  route("image-to-pdf", "routes/image-to-pdf.tsx"),
  route("watermark-pdf", "routes/watermark-pdf.tsx"),
  route("lock-pdf", "routes/lock-pdf.tsx"),
  route("sign-pdf", "routes/sign-pdf.tsx"),
  route("edit-pdf", "routes/edit-pdf.tsx"),
  route("equation-to-image", "routes/equation-to-image.tsx"),
  route("latex-resume", "routes/latex-resume.tsx"),
  route("resume-builder", "routes/resume-builder.tsx"),
  route("tools/:toolId", "routes/tool-workbench.tsx"),
  route("*", "routes/not-found.tsx"),
] satisfies RouteConfig;
