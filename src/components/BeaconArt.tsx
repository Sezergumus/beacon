import { useId, useRef } from "react";
import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import type { ServerStatus } from "../lib/tauri";

gsap.registerPlugin(useGSAP);

interface BeaconArtProps {
  variant?: "sidebar" | "card";
  status?: ServerStatus;
}

/** A transparent brand mark, with independently animated light on the card. */
export function BeaconArt({ variant = "sidebar", status = "stopped" }: BeaconArtProps) {
  const root = useRef<HTMLSpanElement>(null);
  const lightFilter = `beacon-light-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;

  useGSAP(() => {
    const isTransitioning = status === "provisioning" || status === "starting" || status === "stopping";
    if (variant !== "card" || !isTransitioning) return;

    const media = gsap.matchMedia();
    media.add("(prefers-reduced-motion: no-preference)", () => {
      const animations = [
        gsap.fromTo(".beacon-art__light", { autoAlpha: 0 }, {
          autoAlpha: 1,
          duration: 0.7,
          ease: "sine.inOut",
          repeat: -1,
          yoyo: true,
        }),
        gsap.fromTo(".beacon-art__halo", { autoAlpha: 0 }, {
          autoAlpha: 0.55,
          duration: 0.7,
          ease: "sine.inOut",
          repeat: -1,
          yoyo: true,
        }),
      ];

      const syncVisibility = () => animations.forEach((animation) => animation.paused(document.hidden));
      document.addEventListener("visibilitychange", syncVisibility);
      syncVisibility();
      return () => document.removeEventListener("visibilitychange", syncVisibility);
    }, root);

    return () => media.revert();
  }, { scope: root, dependencies: [variant, status], revertOnUpdate: true });

  const src = variant === "sidebar" ? "/beacon-mark.png" : "/beacon-card-front.png";

  return (
    <span ref={root} className={`beacon-art beacon-art--${variant}`} data-state={status} aria-hidden="true">
      {variant === "card" && (
        <svg width="0" height="0" className="absolute" focusable="false">
          <defs>
            <filter id={lightFilter} colorInterpolationFilters="sRGB">
              {/* Extract the teal channels, leaving the charcoal frame and pedestal unlit. */}
              <feColorMatrix type="matrix" values="0 0 0 0 0.24  0 0 0 0 0.86  0 0 0 0 0.85  -6 6 0 0 -0.2" />
            </filter>
          </defs>
        </svg>
      )}
      {variant === "card" && <span className="beacon-art__halo" />}
      <span className="beacon-art__body">
        <img src={src} alt="" width={variant === "sidebar" ? 44 : 96} height={variant === "sidebar" ? 44 : 96} draggable={false} />
        {variant === "card" && (
          <span className="beacon-art__light" style={{ filter: `url(#${lightFilter})` }}>
            <img src={src} alt="" width={96} height={96} draggable={false} />
          </span>
        )}
      </span>
    </span>
  );
}
