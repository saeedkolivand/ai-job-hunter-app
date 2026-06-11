# AI Job Hunter — Context Glossary

Shared vocabulary for the AI Job Hunter desktop app, resolved during design/grilling
sessions. This is a **glossary only** — definitions of project-specific terms, not a
spec and not implementation notes. Pick one word per concept; list rejected synonyms
under _Avoid_. Grow it lazily as terms are resolved.

## UI controls (`@ajh/ui` design system)

**Switch**:
The binary on/off control rendering ARIA `role="switch"` (a sliding track + thumb). The
single `@ajh/ui` primitive for boolean toggles; supports a `size` and an optional
inline label.
_Avoid_: Toggle, ToggleSwitch, checkbox (when a switch is meant)

**SegmentedControl**:
The control for choosing exactly one option from a small fixed set of mutually-exclusive
segments. Distinct from a Switch, which is boolean.
_Avoid_: Tab bar, radio group (when the segmented control is meant)
