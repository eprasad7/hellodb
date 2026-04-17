import type { MetadataRoute } from "next";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();
  return [
    {
      url: "https://hellodb.dev",
      lastModified: now,
      changeFrequency: "weekly",
      priority: 1,
    },
    {
      url: "https://hellodb.dev/blog",
      lastModified: now,
      changeFrequency: "weekly",
      priority: 0.8,
    },
    {
      url: "https://hellodb.dev/blog/context-is-triage-memory-is-engineering",
      lastModified: new Date("2026-04-17T00:00:00Z"),
      changeFrequency: "monthly",
      priority: 0.7,
    },
  ];
}
