# The backdrop

A draft session's card shows a pattern behind its composer, so a new session opens on a
picture rather than on a blank card. Once the session has seen work the pattern goes,
and with it the image (`crates/studio/src/backdrop.rs`,
`crates/studio/src/composer/backdrop.rs`).

## What it draws

Five patterns, each a field of lightness over the card marked in one ink
(`crates/backdrop`):

| Pattern | What it is | How it moves |
| --- | --- | --- |
| Horizon | Light rising from below the card's bottom edge, ordered dither | Breathes, its edge wavering |
| Clouds | Fractal clouds, ordered dither | Drifts sideways |
| Contours | A fractal terrain's contour lines, ordered dither | The lines flow |
| Halftone | Round dots on a 6 pt grid, larger toward the top-left corner | A ripple runs out from the corner |
| Characters | `.` `:` `-` `=` `+` `*` `#` in 5 by 7 pixel glyphs on a 6 by 9 pt grid, denser where the clouds are brighter | Drifts with the clouds |

- The dither marks 2 pt dots against an 8 by 8 Bayer threshold, fixed to the card, so a
  moving pattern changes only where its field does. Sizes are in points, scaled to the
  display: a larger card shows more of a pattern, not a stretched one. Horizon never
  marks every dot, so its brightest band keeps its texture.
- **Clear around the composer.** The pattern is gone within 16 pt of the composer's
  column (the context row, the box and the problem line), measured as the column lays
  out, and full 120 pt away. Nothing is read on the pattern, so its ink may be faint.
- **Ink.** `backdrop` (gray: white at 17% dark, `text_primary` at 14% light) or
  `backdrop_warm` (`claude` at 50% dark, 45% light), as the owner picks.

## Choosing it

The card's bottom-right corner, `SPACE_2` in, holds an icon button on
`composer_background` under a `border` hairline, so it reads the same over any
pattern; its tooltip says "Background". Its menu opens above it: Pattern (None and the
five), Color (Gray, Warm), then Move slowly, each checked while it applies. A pick
applies at once and is saved in `workspace.json` as `backdrop`
(`{"style": "horizon", "tint": "gray", "moving": false}`, the default). A file from
before it has none and gets the default; a style or a color this build doesn't know
reads as the default, and the rest of the file loads.

## Drawing and motion

- The pattern is drawn off the UI thread into an image of the card's device pixels,
  10 to 15 ms for a full Retina card, the crate built optimized even in dev builds.
  It is drawn again when the card's size, the composer's place, the theme or the
  choice changes; the last image stays, stretched, until the next is ready, and a
  replaced one is freed from the window's atlas.
- **Move slowly** draws a new frame 12 times a second, about 12% of one core, and only
  while the card is painted: a draft off screen, a session's conversation, a window the
  system doesn't draw ask for no frames. Turned off, the pattern stops where it is.
  While the system reduces motion the pattern holds still whatever the choice.
