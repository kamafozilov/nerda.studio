# The backdrop

A draft session's card shows a pattern behind its composer, drawn from an image of the
owner's if they add one, so a new session opens on a picture of their own rather than on
a blank card. Once the session has seen work the pattern goes, and with it the image
(`crates/studio/src/backdrop/`, `crates/studio/src/composer/backdrop*.rs`).

## What it draws

Five patterns, each a field of lightness over the card marked in one ink (`crates/backdrop`):

| Pattern | Its own field | With an image | How it moves |
| --- | --- | --- | --- |
| Horizon | Light rising from below the card's bottom edge, ordered dither | The image, dithered, rising from the bottom the same way | Breathes, its edge wavering |
| Clouds | Fractal clouds, ordered dither | The image, dithered, thinned where the clouds part | Drifts sideways |
| Contours | A fractal terrain's contour lines, ordered dither | The image's own contour lines | The lines flow |
| Halftone | Round dots on a 6 pt grid, larger toward the top-left corner | Dots sized by the image | A ripple runs across |
| Characters | `.` `:` `-` `=` `+` `*` `#` in 5 by 7 pixel glyphs on a 6 by 9 pt grid, denser where the clouds are brighter | Denser where the image is | Drifts with the clouds |

With an image, None becomes **Image only**: the image itself, every pixel.

- The dither marks 2 pt dots against an 8 by 8 Bayer threshold, fixed to the card, so a
  moving pattern changes only where its field does. Sizes are in points, scaled to the
  display: a larger card shows more of a pattern, not a stretched one. Horizon never
  marks every dot, so its brightest band keeps its texture.
- **An image** is scaled to cover the card, centered. On a dark card its light areas draw
  the marks, on a light card its dark areas, so it reads the right way round in both.
- **Clear around the composer.** The pattern is gone within 16 pt of the composer's
  column (the context row, the box and the problem line), measured as the column lays
  out, and full 120 pt away. Nothing is read on the pattern, so its ink may be faint.
- **Ink.** Gray is `backdrop` (white at 17% dark, `text_primary` at 14% light), Warm is
  `backdrop_warm` (`claude` at 50% dark, 45% light), Image is each mark in the image's
  color under it at 45%. **Strength** scales that alpha: Subtle 0.6, Medium 1, Bold 1.7.
  The image's colors wait for an image; until it reads, the pattern is gray.

## Choosing it

The draft card's tray, centered 16 pt above its bottom edge (`composer/tray.rs`), holds
the way in. Closed, it is one quiet round chevron, `CONTROL_SIZE`, on the card's own fill
under a faint `border` hairline, pointing up, its tooltip "More options". A click raises
its options over it: they rise 8 pt and fade in on the panel spring while the chevron
turns to point down, with no tooltip; a second click lowers them on the exit spring, and
they take no clicks while they go. Under Reduce motion both are instant. Its one option
for now is a quiet pill, its small icon and "Background" in `text_tertiary` at
`TEXT_SMALL` on the card's own fill under a faint `border` hairline; under the pointer,
and while the panel is open, it takes `composer_background`, its label `text_primary`.
It opens the panel above it, `SPACE_2` clear; the panel stays open while choices are
tried, and a click outside it (the chevron included), Escape or the pill closes it and
gives the cursor back to the text. Lowering the tray closes it too. Left and Right pick
the pattern before or after.

The panel, 392 wide on `popover_background` with `DIALOG_RADIUS` corners, scrolls when the
window is too short for it, staying under the top row:

- **Pattern**: six cards in a three-column grid that fills the row. Each shows its pattern as the card would, a
  fifth the size with its ink 2.5 times stronger, around a small composer; the chosen one
  has a `focus_ring` ring and its name in `text_primary`.
- **Image**: a dashed box, "Add an image", that opens the system's picker for one file; or
  the image, small, with Replace and Remove. Why an image couldn't be added shows under it.
- **Color**: Gray, Warm and Image as chips with a dot of each; Image's dot is the image.
- **Strength**: Subtle, Medium, Bold in one segmented control.
- **Move slowly**, with a switch.

A pick applies at once and is saved in `workspace.json` as `backdrop` (`{"style":
"horizon", "tint": "gray", "strength": "medium", "moving": false}`, the default, and
`"image"` once there is one). A file from before it has none and gets the default; a name
this build doesn't know reads as the default, and the rest of the file loads.

## The image

- Adding one reads the file, at most 64 MB, and decodes it (PNG, JPEG, WebP, GIF, BMP,
  TIFF) to make sure it can be used, then copies it into `backdrops/` beside the workspace
  under a new name, so it outlives the file it came from. Replacing or removing it deletes
  the copy before it.
- It is decoded off the UI thread to at most 2048 pixels a side. A file that can't be read
  is refused with why ("That image can't be read. Try a PNG, JPEG, WebP or GIF."), and
  nothing changes. A copy gone missing or unreadable later shows "It no longer reads" in
  the panel; the pattern draws without it until it is replaced or removed.
- A saved name that would reach outside `backdrops/` (a separator, a leading dot) reads
  as no image.

## Drawing and motion

- The pattern is drawn off the UI thread into an image of the card's device pixels, 2 to
  12 ms for a full Retina card (Image only, every pixel, about 40 ms), the crate built
  optimized even in dev builds. It is drawn again when the card's size, the composer's
  place, the theme or the choice changes; the last image stays, stretched, until the next
  is ready, and a replaced one is freed from the window's atlas.
- **Move slowly** draws a new frame 12 times a second, about 12% of one core, and only
  while the card is painted: a draft off screen, a session's conversation, a window the
  system doesn't draw ask for no frames. Image only holds still and is not drawn again.
  Turned off, the pattern stops where it is. While the system reduces motion the pattern
  holds still whatever the choice, and the panel says so.
