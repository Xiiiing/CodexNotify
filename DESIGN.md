# CodexNotify design system

CodexNotify uses a compact developer-tool interface inspired by the clarity of native command tools. The product must feel calm, precise and functional rather than decorative.

## Foundations

- Typography: 14px body, 12px supporting text, 13–14px controls, 15–16px card titles and 22–24px page titles.
- Spacing: use a 4px base grid, with 8, 12, 16, 20, 24 and 32px as the standard steps.
- Color: neutral surfaces carry hierarchy; purple is the only interactive accent. Green, amber and red are reserved for semantic status.
- Shape: controls use 8px radii, cards 12px and major status surfaces 14–16px.
- Motion: only short state transitions and progress indicators. Respect `prefers-reduced-motion`.
- Accessibility: every interactive element needs a visible keyboard focus state. Text and paths must wrap without reducing the base font size.

## Brand mark

`apps/desktop/src/assets/app-icon.png` is the only editable logo source. It combines a notification bell with a code symbol in the product's blue-to-purple gradient. Generated desktop and tray assets must be refreshed with `npm run icon` after the PNG changes; the script preserves the artwork while centering it on a square transparent canvas.

## Layout

- Default desktop window: 1240×800; minimum 980×680.
- Sidebar: 232px. Top bar: 80px. Main content: no wider than 1184px.
- At widths below 1100px, two-column working areas collapse to one column.
- Empty states should explain the next action without creating a tall blank panel.
