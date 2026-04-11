import type { Metadata } from "next";
import Link from "next/link";
import { SiteHeader } from "@/components/SiteHeader";
import { Footer } from "@/components/Footer";
import { getSeoPagesBySection, getSeoPagePath } from "@/lib/seoPages";

export const metadata: Metadata = {
  title: "Compare | Tabularis",
  description:
    "Comparison pages for teams evaluating Tabularis against other database clients and SQL tools.",
  alternates: { canonical: "/compare" },
};

export default function ComparePage() {
  const pages = getSeoPagesBySection("compare");

  return (
    <div className="container">
      <SiteHeader crumbs={[{ label: "compare" }]} />

      <section>
        <div className="blog-intro">
          <img
            src="/img/logo.png"
            alt="Tabularis Logo"
            className="blog-intro-logo"
          />
          <div className="blog-intro-body">
            <h3>Compare</h3>
            <p>
            Comparison pages for developer teams evaluating Tabularis against
            other database tools and SQL workflows.
            </p>
          </div>
        </div>

        <div className="plugin-list">
          {pages.map((page) => (
            <div key={page.slug} className="plugin-entry">
              <div className="plugin-entry-info">
                <div className="plugin-entry-header">
                  <Link
                    href={getSeoPagePath("compare", page.slug)}
                    className="plugin-name"
                  >
                    {page.title}
                  </Link>
                </div>
                <p className="plugin-desc">{page.excerpt}</p>
              </div>
              <Link
                href={getSeoPagePath("compare", page.slug)}
                className="btn-plugin"
              >
                Open &rarr;
              </Link>
            </div>
          ))}
        </div>
      </section>

      <Footer />
    </div>
  );
}
