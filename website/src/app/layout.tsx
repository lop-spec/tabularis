import type { Metadata } from "next";
import { Analytics } from "@/components/Analytics";
import { CookieConsent } from "@/components/CookieConsent";
import { SearchModal } from "@/components/SearchModal";
import { getAllPosts } from "@/lib/posts";
import { getAllWikiPages } from "@/lib/wiki";
import { getAllPlugins } from "@/lib/plugins";
import { OG_IMAGE_URL } from "@/lib/siteConfig";
import "./globals.css";
import "highlight.js/styles/atom-one-dark.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://tabularis.dev"),
  title: "Tabularis | Enjoy your queries again",
  description:
    "A lightweight, developer-focused database management tool, built with Tauri and React.",
  icons: { icon: "/img/logo.png" },
  openGraph: {
    type: "website",
    url: "https://tabularis.dev/",
    title: "Tabularis | Enjoy your queries again",
    description:
      "A lightweight, developer-focused database management tool, built with Tauri and React.",
    images: [
      OG_IMAGE_URL,
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Tabularis | Enjoy your queries again",
    description:
      "A lightweight, developer-focused database management tool, built with Tauri and React.",
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
  const posts = getAllPosts();
  const wikiPages = getAllWikiPages();
  const plugins = getAllPlugins();
  return (
    <html lang="en">
      <body>
        {children}
        <Analytics />
        <CookieConsent />
        <SearchModal posts={posts} wikiPages={wikiPages} plugins={plugins} />
      </body>
    </html>
  );
}
