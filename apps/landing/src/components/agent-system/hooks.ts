'use client';

import { useEffect, useRef, useState } from 'react';

import { PAIRS, STATIONS } from '@/data/agent-fleet';
import { activeStationX, beltProgress, nearestStationIndex } from '@/lib/agent-system/belt';
import {
  MOBILE_BREAKPOINT_QUERY,
  RESIZE_DEBOUNCE_MS,
  REVEAL_IO_ROOT_MARGIN,
  REVEAL_IO_THRESHOLD,
} from '@/lib/agent-system/constants';
import { linkPathD } from '@/lib/agent-system/links';
import { countUp, sceneProgress } from '@/lib/agent-system/reveal';

// The three layout-dependent effects that used to live inline in
// AgentFleet.tsx, each still `[]`-scoped and self-contained — every ref is
// owned by its hook (so React recognizes it as stable and no exhaustive-deps
// warning fires) and returned for the section that renders the matching DOM
// to attach.

// ── fleet-map link geometry (layout-dependent → measured in an effect, then
//    rendered declaratively as <path> state; hover lighting is React state) ──
export function useMapLinks() {
  const gridRef = useRef<HTMLDivElement>(null);
  const [links, setLinks] = useState<{ pair: string; d: string }[]>([]);
  const [linksViewBox, setLinksViewBox] = useState('0 0 0 0');

  useEffect(() => {
    function buildLinks() {
      const grid = gridRef.current;
      if (!grid) return;
      if (window.matchMedia(MOBILE_BREAKPOINT_QUERY).matches) {
        setLinks([]);
        return;
      }
      const gr = grid.getBoundingClientRect();
      setLinksViewBox(`0 0 ${gr.width} ${gr.height}`);
      const out: { pair: string; d: string }[] = [];
      for (const [author, critics] of PAIRS) {
        const aEl = grid.querySelector<HTMLElement>(`.node[data-name="${author}"]`);
        if (!aEl) continue;
        const ar = aEl.getBoundingClientRect();
        for (const critic of critics) {
          const cEl = grid.querySelector<HTMLElement>(`.node[data-name="${critic}"]`);
          if (!cEl) continue;
          const cr = cEl.getBoundingClientRect();
          out.push({ pair: `${author}|${critic}`, d: linkPathD(gr, ar, cr) });
        }
      }
      setLinks(out);
    }

    buildLinks();
    let resizeTimer = 0;
    const onResize = () => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(buildLinks, RESIZE_DEBOUNCE_MS);
    };
    window.addEventListener('resize', onResize);
    if (document.fonts?.ready) void document.fonts.ready.then(buildLinks);

    return () => {
      window.removeEventListener('resize', onResize);
      window.clearTimeout(resizeTimer);
    };
  }, []);

  return { gridRef, links, linksViewBox };
}

// ── belt scroll scrub (layout-dependent → imperative) ────────────────────────
export function useBeltScrub() {
  const beltSectionRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const stepRef = useRef<HTMLElement>(null);
  const nameRef = useRef<HTMLElement>(null);
  const labelRef = useRef<HTMLSpanElement>(null);
  const stampRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const section = beltSectionRef.current;
    const track = trackRef.current;
    if (!section || !track) return;
    const viewport = track.parentElement;
    let ticking = false;
    let lastStep = -1;

    const horizontal = () => !window.matchMedia(MOBILE_BREAKPOINT_QUERY).matches;

    function paintBelt() {
      ticking = false;
      if (!section || !track || !horizontal()) return;
      const rect = section.getBoundingClientRect();
      const progress = beltProgress(rect.top, section.offsetHeight, window.innerHeight);
      const stations = Array.from(track.querySelectorAll<HTMLElement>('.station'));
      const first = stations[0];
      const last = stations[stations.length - 1];
      if (!first || !last) return;

      const firstC = first.offsetLeft + first.offsetWidth / 2;
      const lastC = last.offsetLeft + last.offsetWidth / 2;
      const activeX = activeStationX(firstC, lastC, progress);
      const center = (viewport?.clientWidth ?? window.innerWidth) / 2;
      track.style.transform = `translateX(${(center - activeX).toFixed(1)}px)`;

      const centers = stations.map((s) => s.offsetLeft + s.offsetWidth / 2);
      const step = nearestStationIndex(centers, activeX);
      stations.forEach((s, i) => s.classList.toggle('lit', i === step));

      if (step !== lastStep) {
        lastStep = step;
        const st = STATIONS[step];
        if (st) {
          if (stepRef.current) stepRef.current.textContent = String(step + 1);
          if (nameRef.current) nameRef.current.textContent = st.title;
          if (labelRef.current) labelRef.current.textContent = st.title;
          if (stampRef.current) stampRef.current.textContent = st.stamp;
        }
      }
    }

    const schedule = () => {
      if (!ticking) {
        ticking = true;
        window.requestAnimationFrame(paintBelt);
      }
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    if (document.fonts?.ready) void document.fonts.ready.then(paintBelt);
    paintBelt();

    return () => {
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
    };
  }, []);

  return { beltSectionRef, trackRef, stepRef, nameRef, labelRef, stampRef };
}

// ── reveal-on-scroll, draw-on-scroll, count-up ───────────────────────────────
export function useReveal() {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = rootRef.current;
    if (!root || !('IntersectionObserver' in window)) return;
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    const countEl = root.querySelector<HTMLElement>('[data-count]');
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add('in');
          if (entry.target === countEl) {
            countUp(countEl, Number(countEl.getAttribute('data-count')) || 0, reduce);
          }
          io.unobserve(entry.target);
        }
      },
      { threshold: REVEAL_IO_THRESHOLD, rootMargin: REVEAL_IO_ROOT_MARGIN }
    );
    root.querySelectorAll('.reveal').forEach((el) => io.observe(el));
    if (countEl) io.observe(countEl);

    const scenes = Array.from(root.querySelectorAll<HTMLElement>('.draw'));
    let ticking = false;
    function paintScenes() {
      const vh = window.innerHeight;
      for (const scene of scenes) {
        const r = scene.getBoundingClientRect();
        const p = sceneProgress(r.top, r.height, vh);
        scene.style.setProperty('--p', p.toFixed(4));
      }
      ticking = false;
    }
    const schedule = () => {
      if (!ticking) {
        ticking = true;
        window.requestAnimationFrame(paintScenes);
      }
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    paintScenes();

    return () => {
      io.disconnect();
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
    };
  }, []);

  return rootRef;
}
