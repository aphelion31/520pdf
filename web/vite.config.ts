import { reactRouter } from "@react-router/dev/vite";
import tailwindcss from "@tailwindcss/vite";
import { fumadocsMdx } from "fumadocs-mdx/vite";
import { defineConfig } from "vite";

const api = process.env.PDF520_URL ?? "http://127.0.0.1:8080";

export default defineConfig({
  base: "/",
  plugins: [tailwindcss(), fumadocsMdx(), reactRouter()],
  resolve: { tsconfigPaths: true },
  server: {
    proxy: {
      "/api": api,
      "/health": api,
      "/ready": api,
    },
  },
});
