# In-app Beacon artwork

Both PNGs have real transparency and were created with the built-in image
generation tool. The desktop app icon remains a separate asset.

- `public/beacon-mark.png`: isometric cutout for the sidebar, displayed at 44 px.
- `public/beacon-card-front.png`: unlit, straight-on core and stone pedestal,
  without the glass enclosure, displayed at 96 px in the status card.

`src/components/BeaconArt.tsx` renders the appropriate artwork. The card uses
an aligned copy and an SVG color filter to illuminate only the teal channels;
the stone pedestal stays dark. No separate lit bitmap is required.

| Server state | Light |
| --- | --- |
| Stopped / stopping | Off |
| Provisioning / starting | Smooth on/off blink |
| Running | Steady on |

Reduced motion replaces blinking with a steady intermediate light. Blinking
pauses in hidden tabs, and its animations are cleaned up on status changes
and unmount.

## Image prompt specifications

Sidebar: extract only the isometric cyan glass beacon and compact stone
pedestal from the original app icon. Remove the rounded tile, backdrop and
white fringe. Keep the recognizable geometric cyan core and use controlled
illumination, crisp edges and genuine transparent margins. Optimize detail
for a 40–48 px sidebar mark; no external halo, floor, text or watermark.

Status card: use a perfectly level, straight-on orthographic front elevation,
with no isometric angle or visible top/side faces. Remove the entire outer
glass enclosure. Show the square geometric beacon core above symmetrical
horizontal stone steps. Keep the core's channels dark desaturated teal and
the body charcoal slate, with no baked-in emission, bloom or glow. Preserve
precise geometry so an aligned UI layer can illuminate it. Center the object
on a square RGBA canvas with transparent margins; no backdrop, floor, cast
shadow, badge, text or watermark. Keep details readable at 96 px.

## Validation

`npm run build` passes. Browser checks confirmed that images load, the stopped
state is dark, startup opacity changes over time, the running light remains
steady, and reduced motion disables blinking. The status checks changed only
the browser's in-memory UI store; no server was started or stopped.
