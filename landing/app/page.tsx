import { Nav } from "@/components/site/nav";
import { Hero } from "@/components/site/hero";
import { GatewayDiagram } from "@/components/site/gateway-diagram";
import { Properties } from "@/components/site/properties";
import { BrainLoop } from "@/components/site/brain-loop";
import { InAction } from "@/components/site/in-action";
import { Comparison } from "@/components/site/comparison";
import { Install } from "@/components/site/install";
import { Footer } from "@/components/site/footer";
import { ConsoleSig } from "@/components/site/console-sig";
import { SectionDivider } from "@/components/site/section-divider";

export default function Home() {
  return (
    <>
      <Nav />
      <main className="flex flex-col">
        <Hero />
        <SectionDivider />
        <GatewayDiagram />
        <SectionDivider />
        <Properties />
        <SectionDivider />
        <BrainLoop />
        <SectionDivider />
        <InAction />
        <SectionDivider />
        <Comparison />
        <SectionDivider />
        <Install />
      </main>
      <Footer />
      <ConsoleSig />
    </>
  );
}
