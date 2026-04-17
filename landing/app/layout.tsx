import type { Metadata } from "next";
import { Instrument_Serif, Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  display: "swap",
});

const instrumentSerif = Instrument_Serif({
  variable: "--font-instrument-serif",
  subsets: ["latin"],
  weight: "400",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  metadataBase: new URL("https://hellodb.dev"),
  title: "hellodb — sovereign memory for Claude Code",
  description:
    "Local-first, end-to-end encrypted, branchable memory for Claude Code. You own the keys, the data, and the bill.",
  openGraph: {
    title: "hellodb — sovereign memory for Claude Code",
    description:
      "Local-first, end-to-end encrypted, branchable memory for Claude Code.",
    type: "website",
    url: "https://hellodb.dev",
  },
  twitter: {
    card: "summary_large_image",
    title: "hellodb — sovereign memory for Claude Code",
    description:
      "Local-first, end-to-end encrypted, branchable memory for Claude Code.",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${instrumentSerif.variable} ${jetbrainsMono.variable}`}
    >
      <body className="min-h-screen bg-bg text-fg antialiased">
        {children}
      </body>
    </html>
  );
}
