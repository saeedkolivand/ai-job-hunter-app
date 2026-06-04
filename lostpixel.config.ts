// Visual regression over the existing Storybook stories. Self-contained: no
// hosted API — baselines live in the repo under .lostpixel/baseline/, generated
// once (lost-pixel update) then committed. Advisory in CI until baselines exist
// (see .github/workflows/visual.yml). No `lost-pixel` import — it's run via dlx,
// not a dependency — so this stays a plain config object.
export const config = {
  storybookShots: {
    storybookUrl: './packages/ui/storybook-static',
  },
};
