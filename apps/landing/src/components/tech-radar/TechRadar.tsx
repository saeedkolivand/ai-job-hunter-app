import { RadarLegend } from './RadarLegend';
import { RadarList } from './RadarList';
import { RadarSvg } from './RadarSvg';

// The /tech-radar page body. Server component — no client JS: the SVG blips
// are plain SVG <a> anchors, and the visual radar is hidden below a
// breakpoint in tech-radar.css in favor of <RadarList>, which is always
// rendered regardless of viewport (that's what makes the whole radar
// reachable without a mouse, without JS, and on a phone).
export function TechRadar() {
  return (
    <div className="tr-root">
      <RadarLegend />
      <div className="tr-canvas">
        <RadarSvg />
      </div>
      <RadarList />
    </div>
  );
}
