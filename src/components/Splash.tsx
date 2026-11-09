import React from "react";
import { BeaconArt } from "./BeaconArt";

/** Shown while Beacon loads its worlds and settings. Fades out once `leaving` is set. */
export const Splash: React.FC<{ leaving: boolean }> = ({ leaving }) => (
  <div
    role="status"
    aria-label="Loading Beacon"
    className={`fixed inset-0 z-[100] bg-bg flex flex-col items-center justify-center gap-5 transition-opacity duration-300 ${
      leaving ? "opacity-0 pointer-events-none" : "opacity-100"
    }`}
  >
    <BeaconArt variant="card" status="starting" />
    <div className="text-center">
      <div className="text-3xl font-extrabold tracking-tight">Beacon</div>
      <div className="text-soft mt-1">Getting things ready…</div>
    </div>
  </div>
);
