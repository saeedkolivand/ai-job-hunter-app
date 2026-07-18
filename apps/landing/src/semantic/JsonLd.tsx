// SoftwareApplication JSON-LD for the Semantic layer (ADR-0016 loading
// choreography). Server-rendered into the static export so crawlers see it with
// no JS. ASCII-only.

import { SITE } from "@/content/story";

const SCHEMA = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: SITE.name,
  applicationCategory: "BusinessApplication",
  operatingSystem: "macOS, Windows, Linux",
  description: SITE.description,
  url: SITE.url,
  offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
};

export function JsonLd() {
  return (
    <script
      type="application/ld+json"
      // JSON.stringify output is inert; this is the standard way to embed JSON-LD.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(SCHEMA) }}
    />
  );
}
