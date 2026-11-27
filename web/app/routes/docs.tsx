import { useMemo } from "react";
import type * as PageTree from "fumadocs-core/page-tree";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from "fumadocs-ui/layouts/docs/page";
import defaultMdxComponents from "fumadocs-ui/mdx";
import browserCollections from "../../.source/browser";
import type { Route } from "./+types/docs";

const DOC_PAGES = [
  { slug: "index", title: "Introduction", url: "/docs" },
  { slug: "authentication", title: "Authentication (Argon2id)", url: "/docs/authentication" },
  { slug: "api-tokens", title: "API Tokens & Scopes", url: "/docs/api-tokens" },
  { slug: "compress-pdf", title: "Compress PDF (KB / DPI)", url: "/docs/compress-pdf" },
  { slug: "tools-reference", title: "36-Tool Reference", url: "/docs/tools-reference" },
  { slug: "examples", title: "Code Examples", url: "/docs/examples" },
] as const;

const DOCS_TREE: PageTree.Root = {
  name: "520pdf Documentation",
  children: DOC_PAGES.map((p) => ({
    type: "page",
    name: p.title,
    url: p.url,
  })),
};

function resolveCollectionPath(slug: string): string {
  const keys = Object.keys(browserCollections.docs.raw).map((k) =>
    k.startsWith("./") ? k.slice(2) : k,
  );
  const candidates = [`${slug}.mdx`, `${slug}/index.mdx`];
  for (const cand of candidates) {
    if (keys.includes(cand)) return cand;
  }
  const fallback = keys.find((k) => k.endsWith(`/${slug}.mdx`));
  if (!fallback) {
    throw new Response("Documentation page not found", { status: 404 });
  }
  return fallback;
}

const mdxLoader = browserCollections.docs.createClientLoader({
  component({ default: Mdx, frontmatter, toc }) {
    const title = typeof frontmatter?.title === "string" ? frontmatter.title : "Documentation";
    const description =
      typeof frontmatter?.description === "string" ? frontmatter.description : undefined;
    return (
      <DocsPage toc={toc}>
        <DocsTitle>{title}</DocsTitle>
        {description && <DocsDescription>{description}</DocsDescription>}
        <DocsBody>
          <Mdx components={defaultMdxComponents} />
        </DocsBody>
      </DocsPage>
    );
  },
});

export async function clientLoader({ params }: Route.ClientLoaderArgs) {
  const raw = (params["*"] ?? "").replace(/^\/+|\/+$/g, "");
  const slug = raw || "index";
  const path = resolveCollectionPath(slug);
  await mdxLoader.preload(path);
  return { path, slug };
}

export default function DocsRoute({ loaderData }: Route.ComponentProps) {
  const tree = useMemo(() => DOCS_TREE, []);
  const content = mdxLoader.useContent(loaderData.path);

  return (
    <DocsLayout
      tree={tree}
      nav={{
        title: (
          <span className="inline-flex items-center gap-2 font-bold">
            <img src="/brand-assets/icon.svg" alt="" className="h-6 w-6" />
            <span>520pdf Docs</span>
          </span>
        ),
        url: "/",
      }}
      links={[
        { text: "Workbench (36 Tools)", url: "/", active: "none" },
        { text: "API Keys", url: "/api-keys", active: "url" },
        { text: "Sign In", url: "/login", active: "url" },
        { text: "OpenAPI JSON", url: "/api/openapi.json", external: true },
      ]}
    >
      {content}
    </DocsLayout>
  );
}
