import type { Metadata } from "next";
import { Analytics } from "@/components/Analytics";
import { CookieConsent } from "@/components/CookieConsent";
import { SearchModal } from "@/components/SearchModal";
import { OG_IMAGE_URL } from "@/lib/siteConfig";
import "./globals.css";
import "highlight.js/styles/atom-one-dark.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://tabularis.dev"),
  title: "Tabularis | Open-Source Desktop Client for Modern Databases",
  description:
    "Open-source desktop database client with support for PostgreSQL, MySQL/MariaDB, and SQLite. Hackable with plugins, with notebooks, AI, and MCP built in.",
  icons: { icon: "/img/logo.png" },
  openGraph: {
    type: "website",
    url: "https://tabularis.dev/",
    title: "Tabularis | Open-Source Desktop Client for Modern Databases",
    description:
      "Open-source desktop database client with support for PostgreSQL, MySQL/MariaDB, and SQLite. Hackable with plugins, with notebooks, AI, and MCP built in.",
    images: [
      OG_IMAGE_URL,
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Tabularis | Open-Source Desktop Client for Modern Databases",
    description:
      "Open-source desktop database client with support for PostgreSQL, MySQL/MariaDB, and SQLite. Hackable with plugins, with notebooks, AI, and MCP built in.",
    images: [
      OG_IMAGE_URL,
    ],
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        {children}
        <Analytics />
        <CookieConsent />
        <SearchModal />
      </body>
    </html>
  );
}
