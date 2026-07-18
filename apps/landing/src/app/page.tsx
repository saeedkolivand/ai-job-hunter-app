// TERMINAL VELOCITY home. The SemanticLayer is a SERVER component (prerendered
// into the static export for SEO + crawlable copy + scroll height); it is passed
// as a prop to the client <Experience>, which runs the gate and mounts GL on top
// only when the Experience gate passes.
import { Experience } from "@/engine/Experience";
import { SemanticLayer } from "@/semantic/SemanticLayer";

export default function Page() {
  return <Experience semantic={<SemanticLayer />} />;
}
