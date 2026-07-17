// Faithful port of the two inline IIFEs from landing/index.html (console egg +
// the main interactivity boot). This is a transcription, not a refactor: same
// function names, same math, same knobs. All human-visible copy is \uXXXX
// escaped to keep this source strictly ASCII (Turbopack multi-byte sourcemap
// crash). initLegacy() is only ever called client-side by LegacyBoot.

let started = false;

export function initLegacy(): void {
  if (typeof window === "undefined") return; // SSR guard: never touch window at module scope
  if (started) return; // double-init guard
  started = true;

  /* ---- console easter egg: a hello for whoever opens devtools, in house voice.
          (no reliable "console opened" event exists, so we just leave it waiting.) ---- */
  (function () {
    try {
      var head = "color:#0b0b0b;background:#ffd000;font:700 22px/1.5 monospace;padding:8px 14px;text-shadow:1px 1px 0 #ff00d4";
      var soft = "color:#9fb0c6;font:13px/1.7 monospace";
      var link = "color:#6cc6ff;font:13px/1.7 monospace";
      console.log("%c PLEASE HIRE HIM. ", head);
      console.log("%cmade it this far? you\u2019re either hiring or you\u2019re me at 3am. if it\u2019s the former \u2014 he\u2019s alarmingly available \u{1F449} https://github.com/saeedkolivand/ai-job-hunter-app", link);
      console.log("%c(source\u2019s PolyForm Noncommercial \u2014 yours to read & fork, not to sell. like my prospects.)", soft);
    } catch {}
  })();

  (function () {
    var reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

    /* ---- gate SMIL <animate> elements that CSS prefers-reduced-motion cannot reach ---- */
    if (reduce) {
      document.querySelectorAll("animate").forEach(function (a) {
        a.setAttribute("dur", "0.001s");
        a.setAttribute("repeatCount", "1");
      });
    }

    /* ===== shared "gibberish voice" engine -- one context, one mute, a voice per mood ===== */
    /* each guy's emotion lives in these knobs: base pitch, tempo (dur), syllable count,
       glide (>1 rises within a syllable, <1 falls), drift (pitch trend across syllables),
       jitter (wobble), timbre (wave + optional lowpass lp + optional distortion drive).
       pitchUp/sylUp/faster = how it escalates the more you poke. */
    var VOICES: { [k: string]: any } = {
      annoyed:  { wave: "sawtooth", base: 200, q: 7, syl: 2, dur: 0.105, glide: 1.06, vol: 0.15, climb: 0.05,  jitter: 0.20, pitchUp: 0.55, sylUp: 3, faster: 0.32 },
      sad:      { wave: "sawtooth", base: 140, q: 5, syl: 2, dur: 0.17,  glide: 0.82, vol: 0.17, climb: -0.05, drift: -0.18, jitter: 0.07, pitchUp: 0.04, sylUp: 1, faster: 0, lp: 1500 },
      frazzled: { wave: "sawtooth", base: 300, q: 9, syl: 4, dur: 0.07,  glide: 1.12, vol: 0.13, climb: 0.03,  jitter: 0.50, pitchUp: 0.25, sylUp: 3, faster: 0.25 },
      fried:    { wave: "sawtooth", base: 300, q: 7, syl: 3, dur: 0.085, glide: 1.12, vol: 0.11, climb: 0.12,  jitter: 0.22, pitchUp: 0.25, sylUp: 2, faster: 0.12, lp: 2400 },
      smug:     { wave: "sawtooth", base: 165, q: 9, syl: 2, dur: 0.14,  glide: 0.90, vol: 0.16, climb: -0.03, drift: -0.06, jitter: 0.06, pitchUp: 0.04, sylUp: 1, faster: 0, lp: 2100 }
    };

    var AUDIO = (function () {
      var ac: any = null, comp: any = null, muted = reduce, toggle: any = null, shown = false, curves: { [k: string]: Float32Array } = {};
      var vowels = [[730, 1090], [530, 1840], [570, 840], [400, 2000], [300, 870], [660, 1700]];
      function curve(k: number) {
        if (curves[k]) return curves[k];
        var n = 1024, c = new Float32Array(n);
        for (var i = 0; i < n; i++) { var x = i / (n - 1) * 2 - 1; c[i] = Math.tanh(x * k); }
        return curves[k] = c;
      }
      function ctx() {
        if (!ac) { try { ac = new ((window as any).AudioContext || (window as any).webkitAudioContext)(); } catch { ac = null; } }
        if (ac && !comp) { comp = ac.createDynamicsCompressor(); comp.connect(ac.destination); }
        if (ac && ac.state === "suspended") { ac.resume(); }
        return ac;
      }
      function syllable(when: number, freq: number, dur: number, vol: number, p: any) {
        var t = ac.currentTime + when, g = p.glide || 1.05;
        var o = ac.createOscillator(); o.type = p.wave || "sawtooth";
        o.frequency.setValueAtTime(freq / g, t);
        o.frequency.linearRampToValueAtTime(freq * g, t + dur);
        var v = vowels[(Math.random() * vowels.length) | 0]!;
        var f1 = ac.createBiquadFilter(); f1.type = "bandpass"; f1.frequency.value = v[0]!; f1.Q.value = p.q || 7;
        var f2 = ac.createBiquadFilter(); f2.type = "bandpass"; f2.frequency.value = v[1]!; f2.Q.value = (p.q || 7) + 2;
        var amp = ac.createGain();
        amp.gain.setValueAtTime(0.0001, t);
        amp.gain.linearRampToValueAtTime(vol, t + (p.attack || 0.012));
        amp.gain.exponentialRampToValueAtTime(0.0001, t + dur);
        o.connect(f1); o.connect(f2);
        var stage: any = null;
        if (p.drive) { var ws = ac.createWaveShaper(); ws.curve = curve(p.drive); ws.oversample = "2x"; f1.connect(ws); f2.connect(ws); stage = ws; }
        if (p.lp) { var lp = ac.createBiquadFilter(); lp.type = "lowpass"; lp.frequency.value = p.lp;
          if (stage) { stage.connect(lp); } else { f1.connect(lp); f2.connect(lp); } stage = lp; }
        if (stage) { stage.connect(amp); } else { f1.connect(amp); f2.connect(amp); }
        amp.connect(comp || ac.destination);
        o.start(t); o.stop(t + dur + 0.04);
      }
      function say(p: any, e: number) {
        reveal(); /* surface the mute button on first poke, even if muted */
        if (muted || !p) return;
        ctx(); if (!ac) return;
        e = Math.max(0, Math.min(e || 0, 1));
        var syl = Math.max(1, Math.round((p.syl || 2) + (p.sylUp || 0) * e));
        var dur = (p.dur || 0.1) * (1 - (p.faster || 0) * e);
        var base = (p.base || 220) * (1 + (p.pitchUp || 0) * e);
        var jit = (p.jitter != null ? p.jitter : 0.18);
        for (var i = 0; i < syl; i++) {
          var prog = syl > 1 ? i / (syl - 1) : 0;
          var f = base * (1 + (p.climb || 0) * i) * (1 + (p.drift || 0) * prog) * (1 + (Math.random() * 2 - 1) * jit);
          syllable(i * dur, f, dur * 0.9, p.vol || 0.14, p);
        }
      }
      function reveal() { if (toggle && !shown) { toggle.classList.add("show"); shown = true; } }
      function bind(btn: any) {
        toggle = btn; if (!toggle) return;
        toggle.classList.toggle("muted", muted);
        toggle.setAttribute("aria-pressed", muted ? "true" : "false");
        toggle.addEventListener("click", function () {
          muted = !muted;
          toggle.classList.toggle("muted", muted);
          toggle.setAttribute("aria-pressed", muted ? "true" : "false");
          if (!muted) ctx();
        });
      }
      return { say, bind, reveal };
    })();
    AUDIO.bind(document.getElementById("sound-toggle"));

    /* ---- loader cycle ---- */
    var msgs = ["summoning a sad little man\u2026", "drafting applications you didn't ask for\u2026", "scraping LinkedIn (allegedly)\u2026", "calculating your worth (loading forever)\u2026"];
    var lt = document.getElementById("loader-text") as HTMLElement, loader = document.getElementById("loader") as HTMLElement, i = 0;
    var iv: any;
    function lift() { loader.classList.add("gone"); }
    if (reduce) { setTimeout(lift, 500); }
    else {
      iv = setInterval(function () { i++; if (i < msgs.length) { lt.textContent = msgs[i]!; } else { clearInterval(iv); lift(); } }, 1500);
    }
    loader.addEventListener("click", function () { try { clearInterval(iv); } catch {} lift(); });

    /* ---- reveal + counters ---- */
    function runCounters(sec: HTMLElement) {
      sec.querySelectorAll("[data-to]").forEach(function (el0) {
        var el = el0 as HTMLElement;
        if (el.dataset.done) return; el.dataset.done = "1";
        var to = +(el.dataset.to as string), dur = 1100, t0: number | null = null;
        function step(t: number) { if (!t0) t0 = t; var p = Math.min((t - t0) / dur, 1); var v = Math.floor(p * to);
          el.textContent = v.toLocaleString("en-US"); if (p < 1) requestAnimationFrame(step); else el.textContent = to.toLocaleString("en-US"); }
        requestAnimationFrame(step);
      });
    }
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) { if (e.isIntersecting) { e.target.classList.add("in"); runCounters(e.target as HTMLElement); } });
    }, { threshold: 0.3 });
    /* .testi/.finale aren't .stage but carry draw-on scribbles -- observe them too */
    document.querySelectorAll(".stage, .testi, .finale").forEach(function (s) { io.observe(s); });

    /* ---- shared cached document height (avoids forced layout every frame) ---- */
    var docMax = 0;
    function measureDoc() {
      docMax = document.documentElement.scrollHeight - (innerHeight || document.documentElement.clientHeight);
      if (docMax < 0) docMax = 0;
    }
    addEventListener("load", measureDoc);
    addEventListener("resize", measureDoc);
    measureDoc();

    /* ---- progress bar + odometer (rAF-throttled, uses cached docMax) ---- */
    var bar = document.getElementById("progress") as HTMLElement, plabel = document.getElementById("progress-label") as HTMLElement, odo = document.getElementById("odometer") as HTMLElement;
    var progTick = false;
    function paintProgress() {
      var f = docMax > 0 ? scrollY / docMax : 0; var pct = Math.round(f * 100);
      bar.style.width = pct + "%";
      plabel.textContent = "job search: " + pct + "% complete \u00b7 est. time remaining: \u221e";
      odo.textContent = "rejections survived: " + (1247 + Math.floor(f * 312)).toLocaleString("en-US");
      progTick = false;
    }
    function onScroll() { if (!progTick) { progTick = true; requestAnimationFrame(paintProgress); } }
    addEventListener("scroll", onScroll, { passive: true }); paintProgress();

    /* ---- scroll scenes: per-section --p (0 to 1 through the viewport) and --c
           (1 when centred) drive the parallax/scrub CSS above. rAF-throttled.
           READ pass (all getBoundingClientRect calls) happens before any WRITE,
           and writes are skipped when the rounded value hasn't changed. ---- */
    var scenes = document.querySelectorAll(".stage, .finale"), sceneTick = false;
    function paintScenes() {
      var sy = scrollY;
      var vh = innerHeight || document.documentElement.clientHeight;
      var maxS = docMax;
      /* READ pass: collect all rects first, no style writes */
      var rects: DOMRect[] = [];
      for (var i = 0; i < scenes.length; i++) rects.push(scenes[i]!.getBoundingClientRect());
      /* WRITE pass: compute and conditionally apply --p / --c */
      for (var i = 0; i < scenes.length; i++) {
        var s = scenes[i] as HTMLElement, r = rects[i]!;
        var p = (vh - r.top) / (vh + r.height);
        /* sections near the document end can never travel fully past the
           viewport, so raw --p tops out below 1 and their scroll-scrubbed
           strokes would stay half-drawn -- rescale by the progress this
           section can actually reach at full scroll */
        var pm = (vh - (r.top + sy - maxS)) / (vh + r.height);
        if (pm > 0 && pm < 1) p /= pm;
        p = p < 0 ? 0 : (p > 1 ? 1 : p);
        var mid = r.top + r.height / 2;
        var c = 1 - Math.min(1, Math.abs(mid - vh / 2) / (vh / 2 + r.height / 2));
        var pf = p.toFixed(4), cf = c.toFixed(4);
        if (pf !== (s as any).__p) { s.style.setProperty("--p", pf); (s as any).__p = pf; }
        if (cf !== (s as any).__c) { s.style.setProperty("--c", cf); (s as any).__c = cf; }
      }
      sceneTick = false;
    }
    function scheduleScenes() { if (!sceneTick) { sceneTick = true; requestAnimationFrame(paintScenes); } }
    if (!reduce) {
      addEventListener("scroll", scheduleScenes, { passive: true });
      addEventListener("resize", scheduleScenes);
      paintScenes();
    }

    /* ---- the journey lines: built from real section positions, two mirrored
           paths snaking down opposite side gutters around each content column,
           both ending beside the CTA. Dashoffset is scrubbed by total scroll
           progress (reversible) and each drawing tip is an arrowhead riding its
           path tangent. ---- */
    (function () {
      var wrap = document.getElementById("journey");
      if (!wrap || reduce) return;
      var svg = wrap.querySelector("svg") as SVGSVGElement, line = document.getElementById("journey-path") as unknown as SVGPathElement, tip = document.getElementById("journey-tip") as unknown as SVGElement;
      var line2 = document.getElementById("journey-path-2") as unknown as SVGPathElement, tip2 = document.getElementById("journey-tip-2") as unknown as SVGElement;
      var len = 0, len2 = 0, built = false, tbl: any = null, tbl2: any = null, tipS = 1;

      /* Catmull-Rom through waypoints -> smooth cubic path */
      function pts2path(P: number[][]) {
        var d = "M" + P[0]![0]!.toFixed(1) + " " + P[0]![1]!.toFixed(1);
        for (var i = 0; i < P.length - 1; i++) {
          var p0 = P[Math.max(0, i - 1)]!, p1 = P[i]!, p2 = P[i + 1]!, p3 = P[Math.min(P.length - 1, i + 2)]!;
          d += " C " + (p1[0]! + (p2[0]! - p0[0]!) / 6).toFixed(1) + " " + (p1[1]! + (p2[1]! - p0[1]!) / 6).toFixed(1)
            + " " + (p2[0]! - (p3[0]! - p1[0]!) / 6).toFixed(1) + " " + (p2[1]! - (p3[1]! - p1[1]!) / 6).toFixed(1)
            + " " + p2[0]!.toFixed(1) + " " + p2[1]!.toFixed(1);
        }
        return d;
      }

      function build() {
        if (getComputedStyle(wrap as HTMLElement).display === "none") { built = false; return; }
        /* release our own stale height before measuring -- otherwise a layout
           shrink (font swap, resize) leaves scrollHeight pinned to the old
           overlay height and the dark body bg shows as a band below the finale */
        (wrap as HTMLElement).style.height = "0px";
        var docH = document.documentElement.scrollHeight, vw = document.documentElement.clientWidth;
        (wrap as HTMLElement).style.height = docH + "px";
        svg.setAttribute("width", String(vw)); svg.setAttribute("height", String(docH));
        svg.setAttribute("viewBox", "0 0 " + vw + " " + docH);
        /* responsive routing: >=1100px snakes through the real gutters around the
           1100px column, weaving INWARD; narrower viewports have no gutter, so the
           line hugs the 22px edge padding and weaves OUTWARD (toward the edge) --
           it never crosses the content column at any width */
        var wide = vw >= 1100;
        var g = wide ? Math.max(22, (vw - 1100) / 2) : Math.max(11, Math.min(22, vw * 0.028));
        var amp = wide ? 28 : Math.min(16, g * 0.7, g - 4);
        var dir = wide ? 1 : -1;
        tipS = vw < 700 ? 0.78 : 1;
        var Lx = g, Rx = vw - g;
        var secs = document.querySelectorAll("main > section");
        /* boxes the side-swap crossing must clear -- exit those sections below
           them so the X never lands on a box (secret dialog, last feature card) */
        var clearEls = document.querySelectorAll(".dialog, .feat-grid"), clearB: number[] = [];
        for (var c = 0; c < clearEls.length; c++) clearB.push(clearEls[c]!.getBoundingClientRect().bottom + scrollY);
        /* ...and headings the crossing must finish ABOVE -- pull that section's
           entry waypoint up so the swap completes before them */
        var gateEls = document.querySelectorAll(".testi h2"), gateT: number[] = [];
        for (var c2 = 0; c2 < gateEls.length; c2++) gateT.push(gateEls[c2]!.getBoundingClientRect().top + scrollY);
        /* both lines share the gutter snake on opposite parities: the second
           starts right wherever the first starts left, so they mirror -- each
           enters from the top of its own gutter, not the center */
        function route(startSide: number) {
          var P: number[][] = [[(startSide % 2 === 0) ? Lx : Rx, 0]], side = startSide, prevEy = 0;
          for (var i = 0; i < secs.length; i++) {
            var s = secs[i]!; if (s.classList.contains("finale")) break;
            var r = s.getBoundingClientRect(), top = r.top + scrollY, h = r.height;
            var x = (side % 2 === 0) ? Lx : Rx, inw = ((side % 2 === 0) ? amp : -amp) * dir;
            var en = top + h * 0.18;
            for (var b = 0; b < gateT.length; b++) {
              if (gateT[b]! > top && gateT[b]! < top + h) en = Math.min(en, Math.max(prevEy + 60, gateT[b]! - 40));
            }
            var ey = top + h * 0.85;
            for (var b2 = 0; b2 < clearB.length; b2++) {
              if (clearB[b2]! > top && clearB[b2]! < top + h) ey = Math.min(top + h, Math.max(ey, clearB[b2]! + 40));
            }
            P.push([x, en]);
            P.push([x + inw, top + h * 0.5]);
            P.push([x, ey]);
            prevEy = ey;
            side++;
          }
          return P;
        }
        var cr = (document.getElementById("cta") as HTMLElement).getBoundingClientRect();
        var ctaY = cr.top + scrollY + cr.height / 2;
        /* clamp both landings so each approach stays on-canvas when the CTA
           sits near either edge on small screens */
        var P = route(0), endX = Math.max(g + amp + 12, cr.left - 38);
        P.push([Lx + 44, ctaY - 170]);
        P.push([Math.max(8, endX - 90), ctaY - 14]);
        P.push([endX, ctaY]);
        var P2 = route(1), endX2 = Math.min(vw - (g + amp + 12), cr.right + 38);
        P2.push([Rx - 44, ctaY - 170]);
        P2.push([Math.min(vw - 8, endX2 + 90), ctaY - 14]);
        P2.push([endX2, ctaY]);
        line.setAttribute("d", pts2path(P));
        line2.setAttribute("d", pts2path(P2));
        len = line.getTotalLength(); len2 = line2.getTotalLength();
        line.setAttribute("stroke-dasharray", String(len));
        line2.setAttribute("stroke-dasharray", String(len2));
        tbl = makeTable(line, len); tbl2 = makeTable(line2, len2);
        built = true;
        paintJourney();
      }

      /* length-by-document-Y lookup so each tip can track the viewport instead
         of lagging where path length outpaces page height (forced monotonic) */
      function makeTable(ln: SVGPathElement, L: number) {
        var TL: number[] = [], TY: number[] = [], steps = Math.max(40, Math.round(L / 24)), prevY = 0;
        for (var k = 0; k <= steps; k++) {
          var l = L * k / steps, y = ln.getPointAtLength(l).y;
          if (y < prevY) y = prevY; prevY = y;
          TL.push(l); TY.push(y);
        }
        return { L: TL, Y: TY };
      }

      function lengthAtY(t: any, y: number) {
        var TL = t.L, TY = t.Y, hi = TY.length - 1, lo = 0;
        if (y <= TY[0]) return TL[0];
        if (y >= TY[hi]) return TL[hi];
        while (hi - lo > 1) { var mid = (hi + lo) >> 1; if (TY[mid] < y) lo = mid; else hi = mid; }
        var f = (y - TY[lo]) / ((TY[hi] - TY[lo]) || 1);
        return TL[lo] + (TL[hi] - TL[lo]) * f;
      }

      /* tip rides ~60% down the current viewport; near the end blend to the
         full path so each arrow finishes exactly on the CTA */
      function drawOne(ln: SVGPathElement, tp: SVGElement, t: any, L: number, p: number) {
        var at = lengthAtY(t, scrollY + innerHeight * 0.6);
        if (p > 0.85) { var f = (p - 0.85) / 0.15; at = at + (L - at) * f; }
        (ln as any).style.strokeDashoffset = Math.max(0, L - at);
        var pt = ln.getPointAtLength(at), pb = ln.getPointAtLength(Math.max(0, at - 2));
        var ang = Math.atan2(pt.y - pb.y, pt.x - pb.x) * 180 / Math.PI;
        tp.setAttribute("transform", "translate(" + pt.x.toFixed(1) + " " + pt.y.toFixed(1) + ") rotate(" + ang.toFixed(1) + ") scale(" + tipS + ")");
      }

      function paintJourney() {
        if (!built) return;
        var p = docMax > 0 ? Math.min(1, Math.max(0, scrollY / docMax)) : 0;
        drawOne(line, tip, tbl, len, p);
        drawOne(line2, tip2, tbl2, len2, p);
      }

      var jt = false;
      addEventListener("scroll", function () { if (!jt) { jt = true; requestAnimationFrame(function () { jt = false; paintJourney(); }); } }, { passive: true });
      var rt: any; addEventListener("resize", function () { clearTimeout(rt); rt = setTimeout(function () { build(); measureDoc(); }, 180); });
      addEventListener("load", function () { build(); measureDoc(); });
      if (document.fonts && document.fonts.ready) { /* re-measure after webfonts settle layout */
        document.fonts.ready.then(function () { build(); measureDoc(); });
      } else {
        setTimeout(function () { build(); measureDoc(); }, 1800);
      }
      build(); measureDoc();
    })();

    /* ---- poke a doodle: it mumbles in-character (voice per mood) ---- */
    var screams = ["AAAAAAA", "STOP TOUCHING ME", "i'm trying my BEST", "do you have any openings??", "please"];
    document.querySelectorAll("[data-scream]").forEach(function (d0) {
      var d = d0 as HTMLElement;
      var b = d.querySelector(".bubble") as HTMLElement | null, n = 0, to: any;
      var lines = d.dataset.lines ? (d.dataset.lines as string).split("|") : screams;
      var voice = VOICES[d.dataset.voice as string] || null;
      function poke() {
        if (b) { b.textContent = lines[n % lines.length]!;
          b.classList.add("show"); clearTimeout(to); to = setTimeout(function () { b!.classList.remove("show"); }, 1600); }
        AUDIO.say(voice, Math.min(n * 0.18, 1));
        n++;
      }
      d.addEventListener("click", poke);
      d.addEventListener("keydown", function (e) {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); poke(); }
      });
    });

    /* ---- hero: the bait sign + escalating annoyed mumbles (uses shared engine) ---- */
    (function () {
      var hero = document.getElementById("hero-dood");
      if (!hero) return;
      var bubble = hero.querySelector(".bubble") as HTMLElement | null;
      var sign = hero.querySelector(".dc-text") as HTMLElement | null;
      var protests = ["i said don't click me", "stop. seriously.", "...why are you like this", "okay\u2014 you win.", "...are you hiring?? hiring?? HIRING??"];
      var n = 0, to: any;
      function pokeHero() {
        var step = n % protests.length;
        if (bubble) { bubble.textContent = protests[step]!;
          bubble.classList.add("show"); clearTimeout(to); to = setTimeout(function () { bubble!.classList.remove("show"); }, 1600); }
        if (sign) { sign.classList.remove("pop"); void sign.offsetWidth; sign.classList.add("pop"); }
        hero!.classList.remove("shake"); void hero!.offsetWidth; hero!.classList.add("shake");
        AUDIO.say(VOICES.annoyed, step / 4);
        n++;
      }
      hero.addEventListener("click", pokeHero);
      hero.addEventListener("keydown", function (e) {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pokeHero(); }
      });
    })();

    /* ---- scroll-speed flail ---- */
    var lastY = scrollY, lastT = performance.now(), toast = document.getElementById("slowdown") as HTMLElement, flailEls = document.querySelectorAll("[data-flail]"), ft: any;
    addEventListener("scroll", function () {
      var now = performance.now(), dy = Math.abs(scrollY - lastY), dt = now - lastT || 1, v = dy / dt;
      lastY = scrollY; lastT = now;
      if (v > 2.6 && !reduce) {
        flailEls.forEach(function (el) { el.classList.add("flailing"); });
        toast.classList.add("show"); clearTimeout(ft);
        ft = setTimeout(function () { flailEls.forEach(function (el) { el.classList.remove("flailing"); }); toast.classList.remove("show"); }, 360);
      }
    }, { passive: true });

    /* ---- download hover: doodle nods ---- */
    var cta = document.getElementById("cta") as HTMLElement, fd = document.getElementById("finale-dood") as HTMLElement, fb = fd.querySelector(".bubble") as HTMLElement;
    cta.addEventListener("mouseenter", function () { fb.textContent = "you sure? \u2026ok."; fb.classList.add("show"); });
    cta.addEventListener("mouseleave", function () { fb.classList.remove("show"); });

    /* ---- cookie notice: dismiss (mirrors the original inline onclick) ---- */
    var cookieBtn = document.querySelector("#cookie button");
    if (cookieBtn) cookieBtn.addEventListener("click", function () { document.getElementById("cookie")!.remove(); });

    /* ---- the secret "Are you sure?" dialog: yes / YES do nothing (by design),
            but the fried guy completely loses it -- escalating, reuses the shared
            voice engine, and finally admits the buttons are fake (you still press
            send yourself). ---- */
    (function () {
      var dlg = document.querySelector(".beat3 .dialog"); if (!dlg) return;
      var guy = document.querySelector(".beat3-dood"), gb = guy ? (guy.querySelector(".bubble") as HTMLElement | null) : null;
      var dq = dlg.querySelector(".dq") as HTMLElement | null, y2 = dlg.querySelector(".y2") as HTMLElement | null;
      var steps = [
        { yell: "YESSS",             ask: "\u2026are you SURE sure?" },
        { yell: "NO TAKEBACKS",      ask: "there is no undo button. you get that?" },
        { yell: "DOING IT DOING IT", ask: "ok. ok ok ok. okay." },
        { yell: "IT'S HAPPENING",    ask: "IT'S HAPPENING." },
        { yell: "\u2026oh",          ask: "(it did nothing. these buttons are fake. you still have to press send yourself. sorry.)" }
      ];
      var n = 0, gt: any;
      dlg.querySelectorAll("button").forEach(function (b) {
        b.addEventListener("click", function () {
          var s = steps[Math.min(n, steps.length - 1)]!;
          if (gb) { gb.textContent = s.yell; gb.classList.add("show"); clearTimeout(gt);
            gt = setTimeout(function () { gb!.classList.remove("show"); }, 1500); }
          if (guy && !reduce) { guy.classList.remove("flailing"); void (guy as HTMLElement).offsetWidth;
            guy.classList.add("flailing"); setTimeout(function () { guy!.classList.remove("flailing"); }, 440); }
          if (dq) dq.textContent = s.ask;
          (b as HTMLElement).style.transform = "scale(1.16)"; setTimeout(function () { (b as HTMLElement).style.transform = ""; }, 150);
          if (y2) y2.style.fontSize = (13 * Math.min(1 + n * 0.16, 1.8)).toFixed(0) + "px";
          AUDIO.say(VOICES.fried, Math.min(n * 0.25, 1));
          n++;
        });
      });
    })();

    /* ---- konami: rejections become offers for 3s ---- */
    var seq = [38, 38, 40, 40, 37, 39, 37, 39, 66, 65], pos = 0;
    var flips = [
      { el: document.querySelector(".beat1 .counter") as HTMLElement | null, offer: "applications sent: 247 \u00b7 OFFERS: 247" },
      { el: document.querySelector(".beat2 .counter") as HTMLElement | null, offer: "applications: 1,000 \u00b7 OFFERS: 1,000 \u00b7 dignity: restored" }
    ];
    var jk = document.getElementById("jk") as HTMLElement;
    addEventListener("keydown", function (e) {
      pos = (e.keyCode === seq[pos]) ? pos + 1 : (e.keyCode === seq[0] ? 1 : 0);
      if (pos === seq.length) { pos = 0;
        flips.forEach(function (f) { if (f.el) { f.el.dataset.orig = f.el.innerHTML; f.el.innerHTML = f.offer; f.el.style.color = "#39d353"; } });
        setTimeout(function () {
          flips.forEach(function (f) { if (f.el) { f.el.innerHTML = f.el.dataset.orig as string; f.el.style.color = ""; } });
          jk.classList.add("show"); setTimeout(function () { jk.classList.remove("show"); }, 1800);
        }, 3000);
      }
    });
  })();
}
