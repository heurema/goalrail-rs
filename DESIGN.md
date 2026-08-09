# Goalrail Web Design

## Intent

The landing page should feel like a quiet control surface, not a dashboard. Its
single visual idea is an animated handoff from Goalrail to a coding agent and
back as evidence.

## Theme

- Dark, aubergine-drenched surface.
- Coral is reserved for the action, the active path, and live status.
- No gradients, decorative cards, or terminal styling.

## Color

- Background: `oklch(20.34% 0.0317 318.90)`
- Deep background: `oklch(16.05% 0.0063 285.67)`
- Foreground: `oklch(95.75% 0.0117 342.64)`
- Muted foreground: `oklch(71.27% 0.0352 343.28)`
- Secondary foreground: `oklch(63.98% 0.0352 342.25)`
- Coral: `oklch(71.48% 0.1297 34.34)`
- Path: `oklch(29.50% 0.0372 344.38)`

## Typography

Use one neutral sans-serif stack with strong weight and size contrast. The hero
heading uses a fluid 52–80px range on desktop and remains below 52px on narrow
screens. Body copy stays at 18–21px with a short readable measure.

## Layout

- Three vertical zones: wordmark, main handoff, repository status.
- Desktop uses an asymmetric two-column composition.
- Mobile becomes one column; the handoff remains visible and interactive.
- Touch targets are at least 44px.

## Motion

- One 2-second path traversal on load explains the product.
- Replay on handoff focus/hover and after the copy action.
- The final state remains visible without JavaScript or WebAssembly.
- Reduced motion renders the final state immediately.

## Components

- Wordmark: coral signal dot plus Goalrail.
- Primary action: solid coral button with inline copied confirmation.
- Secondary action: quiet GitHub text link.
- Handoff: SVG path, moving signal, midpoint label, returned-evidence check.
- Repository status: one concise sentence plus latest-main activity.
