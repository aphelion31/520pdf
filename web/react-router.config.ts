import type { Config } from "@react-router/dev/config";

// SPA mode: static build in build/client, served by the 520pdf Rust binary.
export default {
  ssr: false,
} satisfies Config;
