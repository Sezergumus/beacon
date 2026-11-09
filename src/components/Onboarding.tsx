import React, { useEffect, useState } from "react";
import { Gamepad2, Play, Rocket, Share2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { Button } from "./ui";

interface Step {
  icon: LucideIcon;
  title: string;
  text: string;
  tip?: string;
}

const STEPS: Step[] = [
  {
    icon: Rocket,
    title: "Welcome to Beacon",
    text: "Host a Minecraft world on your own PC and play with friends. No port forwarding, no VPN, no paid server.",
  },
  {
    icon: Play,
    title: "Start your world",
    text: "Name a world on the Home page and press Start server. The first start downloads Java and the game server for you, so it takes a few minutes. After that it's quick.",
    tip: "If Windows asks about the firewall, choose Allow so friends can connect.",
  },
  {
    icon: Share2,
    title: "Share your code",
    text: "Once your world is open, Beacon shows a room code like crimson-dragon-42. Copy it from the sidebar and send it to your friends.",
    tip: "Keep Beacon running while you play. Closing the window can keep it in the system tray.",
  },
  {
    icon: Gamepad2,
    title: "Friends join with the code",
    text: "Your friends open Beacon, type the code under “Join a friend”, then open Minecraft. Your world appears under Multiplayer, in LAN worlds.",
  },
];

/** A short first-run tour. Calls `onDone` when finished or skipped. */
export const Onboarding: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const [index, setIndex] = useState(0);
  const step = STEPS[index];
  const last = index === STEPS.length - 1;
  const Icon = step.icon;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onDone();
      if (e.key === "ArrowRight") setIndex((i) => Math.min(i + 1, STEPS.length - 1));
      if (e.key === "ArrowLeft") setIndex((i) => Math.max(i - 1, 0));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onDone]);

  return (
    <div className="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-6">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Welcome tour"
        className="w-full max-w-lg rounded-3xl bg-surface shadow-2xl p-8"
      >
        <div className="w-16 h-16 rounded-2xl bg-brand/15 text-brand flex items-center justify-center">
          <Icon className="w-8 h-8" aria-hidden />
        </div>
        <h2 className="text-2xl font-extrabold tracking-tight mt-5">{step.title}</h2>
        <p className="text-soft mt-2 leading-relaxed">{step.text}</p>
        {step.tip && (
          <div className="mt-4 rounded-2xl bg-raised p-3.5 text-sm font-bold">{step.tip}</div>
        )}

        <div className="flex items-center justify-between mt-8">
          <div className="flex items-center gap-1.5" aria-hidden>
            {STEPS.map((_, i) => (
              <span
                key={i}
                className={`h-2 rounded-full transition-all ${
                  i === index ? "w-6 bg-brand" : "w-2 bg-line"
                }`}
              />
            ))}
          </div>
          <div className="flex items-center gap-2">
            {!last && (
              <Button variant="ghost" onClick={onDone}>
                Skip
              </Button>
            )}
            {index > 0 && <Button onClick={() => setIndex(index - 1)}>Back</Button>}
            <Button variant="primary" onClick={() => (last ? onDone() : setIndex(index + 1))}>
              {last ? "Let's go" : "Next"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
