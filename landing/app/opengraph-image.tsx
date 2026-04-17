import { ImageResponse } from "next/og";

export const size = { width: 1200, height: 630 };
export const contentType = "image/png";
export const alt = "hellodb — sovereign memory for Claude Code";
export const dynamic = "force-static";

const BG = "#1a1815";
const FG = "#f1efea";
const FG_MUTED = "#8b8779";
const ACCENT = "#e0a96d";

export default function Image() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          background: BG,
          display: "flex",
          flexDirection: "column",
          padding: "72px 80px",
          fontFamily: "ui-serif, Georgia, serif",
          position: "relative",
        }}
      >
        <div
          style={{
            position: "absolute",
            inset: 0,
            background:
              "radial-gradient(60% 50% at 80% 0%, rgba(224,169,109,0.18), transparent 70%)",
          }}
        />
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 14,
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            fontSize: 28,
            color: FG,
          }}
        >
          <span style={{ color: ACCENT }}>›</span>
          <span>hellodb</span>
          <span style={{ color: FG_MUTED, fontSize: 22, marginLeft: 16 }}>
            v0.1.0 · phase 1 shipped
          </span>
        </div>

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginTop: 96,
            fontSize: 110,
            lineHeight: 1.02,
            color: FG,
            letterSpacing: -1,
          }}
        >
          <div style={{ display: "flex" }}>Sovereign memory</div>
          <div style={{ display: "flex", gap: 24 }}>
            <span>for</span>
            <span style={{ fontStyle: "italic", color: ACCENT }}>
              Claude Code.
            </span>
          </div>
        </div>

        <div
          style={{
            display: "flex",
            marginTop: 56,
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            fontSize: 26,
            color: FG_MUTED,
            letterSpacing: 0.2,
          }}
        >
          local-first · end-to-end encrypted · branchable · MCP-native
        </div>

        <div
          style={{
            marginTop: "auto",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "flex-end",
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            fontSize: 22,
            color: FG_MUTED,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 14,
              padding: "14px 22px",
              border: `1px solid ${ACCENT}`,
              borderRadius: 999,
              color: ACCENT,
            }}
          >
            <span>$</span>
            <span>curl -fsSL hellodb.dev/install | sh</span>
          </div>
          <div>your machine · your Cloudflare · ~$0</div>
        </div>
      </div>
    ),
    { ...size },
  );
}
