import type { NextConfig } from "next";

// Static export for Cloudflare Pages deployment at hellodb.dev.
// Landing is SSG-only (no server actions, no API routes). If dynamic
// server features are ever needed, switch to @opennextjs/cloudflare and
// deploy as a Worker; Pages will still host the static assets either way.
const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  images: {
    // Cloudflare Pages doesn't run the Next.js image optimizer; serve raw.
    unoptimized: true,
  },
};

export default nextConfig;
