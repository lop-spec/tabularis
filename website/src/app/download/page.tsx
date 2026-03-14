import type { Metadata } from "next";
import Link from "next/link";
import { SiteHeader } from "@/components/SiteHeader";
import { Footer } from "@/components/Footer";
import { DownloadInline } from "@/components/DownloadInline";
import { APP_VERSION } from "@/lib/version";
import { OG_IMAGE_URL } from "@/lib/siteConfig";

export const metadata: Metadata = {
  title: "Download | Tabularis",
  description:
    "Download Tabularis for Windows, macOS, and Linux. Available via WinGet, Homebrew, Snap, AUR and more.",
  alternates: { canonical: "/download" },
  openGraph: {
    type: "website",
    url: "https://tabularis.dev/download/",
    title: "Download | Tabularis",
    description:
      "Download Tabularis for Windows, macOS, and Linux. Available via WinGet, Homebrew, Snap, AUR and more.",
    images: [OG_IMAGE_URL],
  },
  twitter: {
    card: "summary_large_image",
    title: "Download | Tabularis",
    description:
      "Download Tabularis for Windows, macOS, and Linux. Available via WinGet, Homebrew, Snap, AUR and more.",
    images: [OG_IMAGE_URL],
  },
};

export default function DownloadPage() {
  const releaseDate = new Date("2026-03-14T12:00:00Z").toLocaleDateString(
    "en-US",
    { year: "numeric", month: "long", day: "numeric", timeZone: "UTC" },
  );

  return (
    <div className="container">
      <SiteHeader crumbs={[{ label: "download" }]} />

      <section className="dl-page">
        <div className="dl-page-hero">
          <img src="/img/logo.png" alt="Tabularis" className="dl-page-logo" />
          <div className="dl-page-meta">
            <h1 className="dl-page-version">v{APP_VERSION}</h1>
            <div className="dl-page-submeta">
              <time dateTime="2026-03-14">{releaseDate}</time>
              <span className="dl-page-sep">·</span>
              <Link href="/changelog" className="dl-page-changelog-link">
                View changelog →
              </Link>
            </div>
          </div>
        </div>

        <DownloadInline />

        <div className="dl-page-footer-links">
          <a
            href={`https://github.com/debba/tabularis/releases/tag/v${APP_VERSION}`}
            target="_blank"
            rel="noopener noreferrer"
          >
            Release notes on GitHub →
          </a>
          <a
            href="https://github.com/debba/tabularis/releases"
            target="_blank"
            rel="noopener noreferrer"
          >
            All releases →
          </a>
        </div>
      </section>

      <Footer />
    </div>
  );
}
