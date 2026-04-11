import type { Metadata } from "next";
import Link from "next/link";
import { SiteHeader } from "@/components/SiteHeader";
import { Footer } from "@/components/Footer";
import { getSeoPagesBySection, getSeoPagePath } from "@/lib/seoPages";

export const metadata: Metadata = {
  title: "Solutions | Tabularis",
  description:
    "Explore high-intent Tabularis pages for PostgreSQL, SQL notebooks, MCP workflows, and other database use cases.",
  alternates: { canonical: "/solutions" },
};

export default function SolutionsPage() {
  const pages = getSeoPagesBySection("solutions");

  return (
    <div className="container">
      <SiteHeader crumbs={[{ label: "solutions" }]} />

      <section>
        <div className="blog-intro">
          <img
            src="/img/logo.png"
            alt="Tabularis Logo"
            className="blog-intro-logo"
          />
          <div className="blog-intro-body">
            <h3>Solutions</h3>
            <p>
            Explore Tabularis by workflow and use case, from PostgreSQL work to
            SQL notebooks and AI agent database flows.
            </p>
          </div>
        </div>

        <div className="plugin-list">
          {pages.map((page) => (
            <div key={page.slug} className="plugin-entry">
              <div className="plugin-entry-info">
                <div className="plugin-entry-header">
                  <Link
                    href={getSeoPagePath("solutions", page.slug)}
                    className="plugin-name"
                  >
                    {page.title}
                  </Link>
                </div>
                <p className="plugin-desc">{page.excerpt}</p>
              </div>
              <Link
                href={getSeoPagePath("solutions", page.slug)}
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
