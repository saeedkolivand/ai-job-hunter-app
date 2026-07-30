// #frame/#hud/#controls/#progress of /creature, split out of
// CreatureBody.tsx purely for file size — verbatim ported markup, no props.
// See CreatureBody.tsx for the shared conversion notes; creature-0.js (the
// SVG film engine) binds to this DOM entirely by id (#stage, #caps, #fig,
// #frameline, #sceneRoot, #hud1/2/3, #controls, #pauseBtn, #speedBtn,
// #muteBtn, #skipBtn, #replayBtn, #bar, #barFill, #plabel, ...) — the
// rendered DOM must stay byte-identical (ADR 0018).
//
// `inert` (#controls) is written as a bare boolean prop, NOT `="" `: React's
// boolean-attribute props (hidden/inert/disabled/…) treat an empty-string
// VALUE as falsy and drop the attribute entirely — only `true` (the
// bare-prop shorthand) serializes to the empty-string DOM attribute the
// original HTML has and creature-0.js's hasAttribute() checks expect.
export function Stage() {
  return (
    <>
      <div id="frame">
        <div id="stagebox">
          <svg
            id="stage"
            viewBox="0 0 1200 675"
            preserveAspectRatio="xMidYMid meet"
            aria-hidden="true"
          >
            <defs>
              <filter id="boilA" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.015"
                  numOctaves="2"
                  seed="1"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="3.2" />
              </filter>
              <filter id="boilB" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.02"
                  numOctaves="2"
                  seed="4"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="4.2" />
              </filter>
              <filter id="boilC" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.012"
                  numOctaves="2"
                  seed="7"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="2.6" />
              </filter>
            </defs>
            <g id="frameline" />
            <g id="sceneRoot" />
          </svg>
          <div id="caps" aria-live="polite" aria-atomic="false" />
          <div id="fig" />
        </div>
      </div>

      <div id="hud" aria-hidden="true">
        <div id="hud1" />
        <div id="hud2" />
        <div id="hud3" />
      </div>

      <div id="controls" inert>
        <button id="pauseBtn" aria-label="pause">
          ⏸
        </button>
        <button id="speedBtn" aria-label="speed 1x">
          1×
        </button>
        <button id="muteBtn" aria-label="mute">
          🔊
        </button>
        <button id="skipBtn" aria-label="skip to next scene">
          ⏭
        </button>
        <button id="replayBtn" aria-label="replay from the start">
          ↺
        </button>
      </div>

      <div id="progress" aria-hidden="true">
        <div id="bar">
          <i id="barFill" />
        </div>
        <span id="plabel" />
      </div>
    </>
  );
}
