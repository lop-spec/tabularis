import type { Metadata } from "next";
import Link from "next/link";
import { SiteHeader } from "@/components/SiteHeader";
import { Footer } from "@/components/Footer";

export const metadata: Metadata = {
  title: "Cookie Policy | Tabularis",
  description:
    "Learn how Tabularis uses cookies and how you can control them.",
};

export default function CookiePolicyPage() {
  return (
    <div className="container">
      <SiteHeader crumbs={[{ label: "cookie policy" }]} />

      <article className="wiki-article">
        <h1>Cookie Policy</h1>
        <p className="cookie-policy-updated">Last updated: March 2026</p>

        <section className="cookie-policy-section">
          <h2>What are cookies?</h2>
          <p>
            Cookies are small text files stored in your browser when you visit a
            website. They allow the site to remember certain information across
            page visits and sessions.
          </p>
        </section>

        <section className="cookie-policy-section">
          <h2>How we use cookies</h2>
          <p>
            This website uses a minimal set of cookies, grouped into the
            categories below. You can manage your preferences at any time using
            the cookie banner.
          </p>
        </section>

        <section className="cookie-policy-section">
          <h2>Cookie categories</h2>

          <div className="cookie-policy-category">
            <div className="cookie-policy-category-header">
              <span className="cookie-policy-badge cookie-policy-badge--required">
                Always active
              </span>
              <h3>Necessary</h3>
            </div>
            <p>
              These cookies are required for the website to function correctly.
              They cannot be disabled. They do not store any personally
              identifiable information.
            </p>
            <table className="cookie-policy-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Provider</th>
                  <th>Purpose</th>
                  <th>Expiry</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>
                    <code>tabularis-cookie-consent</code>
                  </td>
                  <td>tabularis.dev</td>
                  <td>Stores your cookie consent preferences</td>
                  <td>Persistent (localStorage)</td>
                </tr>
              </tbody>
            </table>
          </div>

          <div className="cookie-policy-category">
            <div className="cookie-policy-category-header">
              <span className="cookie-policy-badge cookie-policy-badge--optional">
                Optional
              </span>
              <h3>Measurement</h3>
            </div>
            <p>
              These cookies help us understand how visitors interact with the
              website by collecting anonymised usage statistics. We use{" "}
              <a
                href="https://matomo.org"
                target="_blank"
                rel="noopener noreferrer"
              >
                Matomo
              </a>
              , a self-hosted, privacy-friendly analytics platform. No data is
              shared with third parties.
            </p>
            <table className="cookie-policy-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Provider</th>
                  <th>Purpose</th>
                  <th>Expiry</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>
                    <code>_pk_id.*</code>
                  </td>
                  <td>analytics.debbaweb.it</td>
                  <td>Identifies a unique visitor</td>
                  <td>13 months</td>
                </tr>
                <tr>
                  <td>
                    <code>_pk_ses.*</code>
                  </td>
                  <td>analytics.debbaweb.it</td>
                  <td>Tracks a single visit session</td>
                  <td>30 minutes</td>
                </tr>
                <tr>
                  <td>
                    <code>_pk_ref.*</code>
                  </td>
                  <td>analytics.debbaweb.it</td>
                  <td>Stores referral source for the visit</td>
                  <td>6 months</td>
                </tr>
              </tbody>
            </table>
          </div>

          <div className="cookie-policy-category">
            <div className="cookie-policy-category-header">
              <span className="cookie-policy-badge cookie-policy-badge--optional">
                Optional
              </span>
              <h3>Marketing</h3>
            </div>
            <p>
              Marketing cookies are used to deliver relevant advertisements and
              track campaign performance. This website does not currently use
              any marketing cookies.
            </p>
          </div>
        </section>

        <section className="cookie-policy-section">
          <h2>Managing your preferences</h2>
          <p>
            You can review and change your cookie preferences at any time. Your
            current consent is stored locally in your browser and is never sent
            to any server.
          </p>
          <p>
            To withdraw consent or change your choices, clear the{" "}
            <code>tabularis-cookie-consent</code> entry from your browser&apos;s
            local storage, or reload the page after clearing site data —the
            consent banner will reappear.
          </p>
        </section>

        <section className="cookie-policy-section">
          <h2>Contact</h2>
          <p>
            If you have any questions about this cookie policy, please open an
            issue on{" "}
            <a
              href="https://github.com/debba/tabularis"
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub
            </a>{" "}
            or join our{" "}
            <a
              href="https://discord.gg/YrZPHAwMSG"
              target="_blank"
              rel="noopener noreferrer"
            >
              Discord
            </a>
            .
          </p>
        </section>
      </article>

      <Footer />
    </div>
  );
}
