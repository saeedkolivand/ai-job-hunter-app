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

// Escape "<" so a "</script>" inside any serialized field (e.g. SITE.description)
// can't close the element early -- the standard JSON-LD injection guard.
const serialized = JSON.stringify(SCHEMA).replace(/</g, "\\u003c");

export function JsonLd() {
  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: serialized }}
    />
  );
}
