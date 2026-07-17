import LegacyBoot from "@/fallback/LegacyBoot";
import Beat1 from "@/semantic/Beat1";
import Beat2 from "@/semantic/Beat2";
import Beat3 from "@/semantic/Beat3";
import Beat4 from "@/semantic/Beat4";
import Chrome from "@/semantic/Chrome";
import Features from "@/semantic/Features";
import Finale from "@/semantic/Finale";
import Hero from "@/semantic/Hero";
import Testimonials from "@/semantic/Testimonials";

export default function Page() {
  return (
    <>
      <Chrome />
      <main>
        <Hero />
        <Beat1 />
        <Beat2 />
        <Beat3 />
        <Beat4 />
        <Features />
        <Testimonials />
        <Finale />
      </main>
      <LegacyBoot />
    </>
  );
}
