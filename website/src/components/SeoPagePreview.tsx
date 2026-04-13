import type { SeoMeta } from "@/lib/seoPages";

type ComparePreview =
  | {
      variant?: "duo";
      left: string;
      right: string;
      accent: string;
    }
  | {
      variant: "trio";
      left: string;
      center: string;
      right: string;
      accent: string;
    };

const COMPARE_PREVIEW_MAP: Record<
  string,
  ComparePreview
> = {
  "dbeaver-alternative": {
    left: "DBeaver",
    right: "Tabularis",
    accent: "Open-source SQL workspace",
  },
  "tableplus-alternative": {
    left: "TablePlus",
    right: "Tabularis",
    accent: "Cross-platform SQL workflow",
  },
  "datagrip-alternative": {
    left: "DataGrip",
    right: "Tabularis",
    accent: "IDE vs workspace",
  },
  "beekeeper-studio-alternative": {
    left: "Beekeeper",
    right: "Tabularis",
    accent: "Simple client vs broader workflow",
  },
  "dbgate-alternative": {
    left: "DbGate",
    right: "Tabularis",
    accent: "Modern open-source workflow",
  },
  "tableplus-vs-datagrip-vs-tabularis": {
    variant: "trio",
    left: "TablePlus",
    center: "DataGrip",
    right: "Tabularis",
    accent: "Polished GUI vs IDE vs open workspace",
  },
};

function getComparisonPreview(meta: SeoMeta): ComparePreview {
  return (
    COMPARE_PREVIEW_MAP[meta.slug] || {
      left: "Compare",
      right: "Tabularis",
      accent: meta.useCase || "Workflow evaluation",
    }
  );
}

export function SeoPageThumb({ meta, className = "" }: { meta: SeoMeta; className?: string }) {
  if (meta.section === "compare") {
    const preview = getComparisonPreview(meta);

    if (preview.variant === "trio") {
      return (
        <div
          className={`seo-compare-thumb seo-compare-thumb--trio ${className}`.trim()}
          aria-hidden="true"
        >
          <div className="seo-compare-grid">
            <span className="seo-compare-panel">{preview.left}</span>
            <span className="seo-compare-panel">{preview.center}</span>
            <span className="seo-compare-panel seo-compare-panel--tabularis">
              {preview.right}
            </span>
          </div>
          <div className="seo-compare-accent">{preview.accent}</div>
        </div>
      );
    }

    return (
      <div className={`seo-compare-thumb ${className}`.trim()} aria-hidden="true">
        <div className="seo-compare-stack">
          <span className="seo-compare-wordmark">{preview.left}</span>
          <span className="seo-compare-vs">vs</span>
          <span className="seo-compare-wordmark seo-compare-wordmark--tabularis">
            {preview.right}
          </span>
        </div>
        <div className="seo-compare-accent">{preview.accent}</div>
      </div>
    );
  }

  return (
    <img
      src={meta.image || "/img/logo.png"}
      alt={meta.title}
      className={className}
    />
  );
}

export function SeoPageHeroVisual({ meta }: { meta: SeoMeta }) {
  if (meta.section === "compare") {
    return <SeoPageThumb meta={meta} className="seo-compare-hero" />;
  }

  return (
    <img
      src={meta.image || "/img/logo.png"}
      alt={meta.title}
      className="blog-intro-logo"
    />
  );
}
