# The backdrop

A draft session's card shows a pattern behind its composer, drawn from an image of the
owner's if they add one, so a new session opens on a picture of their own rather than on
a blank card. Once the session has seen work the pattern goes, and with it the image
(`crates/studio/src/backdrop/`, `crates/studio/src/composer/backdrop*.rs`, and the saved
choice in `crates/workspace/src/backdrop.rs`).

## What it draws

Five patterns, each a field of lightness over the card marked in one ink (`crates/backdrop`):

| Pattern | Its own field | With an image | How it moves |
| --- | --- | --- | --- |
| Horizon | Light rising from below the card's bottom edge, ordered dither | The image, dithered, rising from the bottom the same way | Breathes, its edge wavering |
| Clouds | Fractal clouds, ordered dither | The image, dithered, thinned where the clouds part | Drifts sideways |
| Contours | A fractal terrain's contour lines, ordered dither | The image's own contour lines | The lines flow |
| Halftone | Round dots on a 6 pt grid, larger toward the top-left corner | Dots sized by the image | A ripple runs across |
| Characters | `.` `:` `-` `=` `+` `*` `#` in 5 by 7 pixel glyphs on a 6 by 9 pt grid, denser where the clouds are brighter | Denser where the image is | Drifts with the clouds |

With an image, None becomes **Image only**: the image itself, every pixel, read between
pixels so a small image scaled up stays smooth.

**The composer stands on glass** whenever a pattern or an image shows: its box and the
tray's controls turn `composer_glass`, `composer_background` at 88%, so what is under
them shows faintly through. The placeholder keeps at least 5.40:1 over a Bold pattern
(6.88:1 light, the Warm ink the worst; 5.71:1 and 7.21:1 in Gray) and, over the worst
image (white behind dark, black behind light, at Bold), 4.76:1 (dark) and 6.24:1
(light), measured on the running app. With None and no image in use the surfaces are
opaque.

**The drawer under a draft's box** takes the box's glass too, so its small words keep
the placeholder's figures: at least 4.76:1 over any Bold pattern or image (6.24:1 light) (`composer/context.rs`;
[composer.md](composer.md)).

- The dither marks 2 pt dots against an 8 by 8 Bayer threshold, fixed to the card, so a
  moving pattern changes only where its field does. Sizes are in points, scaled to the
  display: a larger card shows more of a pattern, not a stretched one. Horizon never
  marks every dot, so its brightest band keeps its texture.
- **An image** is scaled to cover the card, centered. On a dark card its light areas draw
  the marks, on a light card its dark areas, so it reads the right way round in both.
- **Under the composer, not around it.** Every pattern covers the card whole, the
  composer included: a pattern cleared around it read as a hole cut in the card. The
  composer's surfaces set it apart instead (below).
- **Ink.** Gray is `backdrop` (white at 17% dark, `text_primary` at 14% light), Warm is
  `backdrop_warm` (`claude` at 50% dark, 45% light), Image is each mark in the image's
  color under it at 45%. **Strength** scales that alpha: Subtle 0.6, Medium 1, Bold 1.7.
  The image's colors wait for an image; until it reads, the pattern is gray.

## Choosing it

The draft card's tray, centered 16 pt above its bottom edge (`composer/tray.rs`), holds
the way in. Closed, it is one quiet round chevron, `CONTROL_SIZE`, on the card's own fill
(`composer_glass` over a pattern or an image) under a faint `border` hairline, pointing
up, its tooltip "More options". A click raises its options over it: they rise 8 pt and
fade in on the panel spring while the chevron turns to point down, with no tooltip; a
second click lowers them on the exit spring, and they take no clicks while they go. Under Reduce motion both are instant. Its one option
for now is a quiet pill, its small icon and "Background" in `text_tertiary` at
`TEXT_SMALL` on the card's own fill under a faint `border` hairline; under the pointer,
and while the panel is open, it takes the composer's fill, its label `text_primary`.
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
  the image, small, with Replace and Remove. The small image is itself the switch that
  sets it aside without letting go of it: under the pointer a struck-through eye shows
  over it ("Hide image"), and a click hides it; hidden, it stays in gray at half its
  strength with an open eye on it ("Show image"), the patterns draw alone and the image's
  colors wait, and a click brings both back. Why an image couldn't be added shows under
  it.
- **Color**: Gray, Warm and Image as chips with a dot of each; Image's dot is the image,
  and it waits, faded with "Add an image first" for its tooltip, until an image reads.
- **Strength**: Subtle, Medium, Bold in one segmented control.
- **Move slowly**, with a switch.

A pick applies at once and is saved in `workspace.json` as `backdrop` (`{"style":
"horizon", "tint": "gray", "strength": "medium", "moving": false}`, the default, and
`"image"` once there is one, `"image_hidden": true` while it is set aside). A file from
before it has none and gets the default; a name this build doesn't know reads as the
default, and the rest of the file loads.

## The image

- Adding one reads the file, at most 64 MB, and decodes it (PNG, JPEG, WebP, GIF, BMP,
  TIFF, ICO) to make sure it can be used, then copies it into `backdrops/` beside the
  workspace under a new name, so it outlives the file it came from. Replacing or removing
  it deletes the copy before it.
- It is decoded off the UI thread to at most 2048 pixels a side. A file that can't be used
  is refused with why ("That image can't be read. Try a PNG, JPEG, WebP or GIF.", "That
  image is larger than 64 MB.", "That file can't be opened."), and nothing changes. A copy
  gone missing or unreadable later shows "It no longer reads" in the panel; the pattern
  draws without it until it is replaced or removed.
- A saved name that would reach outside `backdrops/` (a separator, a colon, a leading
  dot) reads as no image.

## Drawing and motion

- The pattern is drawn off the UI thread into an image of the card's device pixels, 2 to
  12 ms for a full Retina card (Image only, every pixel read between four, 80 to 100 ms,
  drawn once), the crate built optimized even in dev builds. It is drawn again when the
  card's size, the theme or the choice changes; the last image stays, stretched, until
  the next is ready, and a replaced one is freed from the window's atlas.
- **Move slowly** draws a new frame 12 times a second, about 12% of one core, and only
  while the card is painted: a draft off screen, a session's conversation, a window the
  system doesn't draw ask for no frames. Image only holds still and is not drawn again.
  Turned off, the pattern stops where it is. While the system reduces motion the pattern
  holds still whatever the choice, and the panel says so.
