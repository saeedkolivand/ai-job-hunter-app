import { SiteFooter } from '@/components/SiteFooter';

import { Boot } from './sections/Boot';
import { CheatSheet } from './sections/CheatSheet';
import { Flows } from './sections/Flows';
import { IpcReference } from './sections/IpcReference';
import { Overview } from './sections/Overview';
import { Sidebar } from './sections/Sidebar';
import { Subsystems } from './sections/Subsystems';

// Body markup for /how-it-works, converted 1:1 from the deleted
// src/content/how-it-works/body.html. The rendered DOM must stay
// byte-identical — public/scripts/how-it-works-0.js (the sidebar tab
// switcher + boot/flow players + IPC filter) and how-it-works-1.js (the
// console egg) both bind to it by id/class/data-attr (ADR 0018). The root
// <div style={{display:'contents'}}> replaces the old <RawHtml> wrapper so
// the serialized DOM — including the CSS's `body footer` descendant
// selector — stays identical to the baseline. The <aside> and the 6 <main>
// <section id="view-*"> are split into src/components/how-it-works/sections/
// purely for file size — no props, same mechanical conversion; this file
// only composes them in the original DOM order.
export function HowItWorksBody() {
  return (
    <div style={{ display: 'contents' }}>
      <Sidebar />

      <main>
        <Overview />
        <Boot />
        <Flows />
        <IpcReference />
        <Subsystems />
        <CheatSheet />
      </main>
      <SiteFooter current="how-it-works" />
    </div>
  );
}
