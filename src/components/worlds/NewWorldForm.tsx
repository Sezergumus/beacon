import React, { useState } from "react";
import { Loader2, Skull } from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import { Button, Disclosure, TextInput, Toggle } from "../ui";

/** Name a new world and, if wanted, pick its seed and hardcore mode (fixed once it's made). */
export const NewWorldForm: React.FC<{
  placeholder: string;
  /** Shown left of the name box. */
  label?: string;
  autoFocus?: boolean;
  /** Without this there is no Cancel button (e.g. when it's the only thing you can do). */
  onCancel?: () => void;
  onCreated?: () => void;
  className?: string;
}> = ({ placeholder, label, autoFocus, onCancel, onCreated, className }) => {
  const createWorld = useServerStore((s) => s.createWorld);
  const [name, setName] = useState("");
  const [seed, setSeed] = useState("");
  const [hardcore, setHardcore] = useState(false);
  const [saving, setSaving] = useState(false);

  const submit = async () => {
    if (!name.trim() || saving) return;
    setSaving(true);
    const id = await createWorld(name, { seed: seed.trim() || undefined, hardcore });
    setSaving(false);
    if (id) {
      setName("");
      setSeed("");
      setHardcore(false);
      onCreated?.();
    }
  };

  return (
    <form
      className={`flex flex-col gap-4 ${className ?? ""}`}
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <div className="flex items-center gap-3">
        {label && <span className="font-bold text-soft">{label}</span>}
        <TextInput
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={placeholder}
          maxLength={40}
          autoFocus={autoFocus}
          aria-label="New world name"
          className="max-w-md"
        />
        <Button
          type="submit"
          variant="primary"
          disabled={saving || !name.trim()}
          icon={saving ? <Loader2 className="w-4 h-4 animate-spin" /> : undefined}
        >
          Create
        </Button>
        {onCancel && (
          <Button type="button" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>

      <Disclosure label="More options">
        <div className="flex flex-col gap-4 max-w-xl">
          <label className="flex flex-col gap-1.5">
            <span className="font-bold">Seed</span>
            <TextInput
              value={seed}
              onChange={(e) => setSeed(e.target.value)}
              placeholder="Leave empty for a random world"
              maxLength={64}
              className="font-mono text-sm"
            />
            <span className="text-sm text-soft">
              Any word or number. The same seed makes the same world.
            </span>
          </label>
          <div className="flex items-center justify-between gap-6">
            <div className="min-w-0">
              <div className="font-bold flex items-center gap-1.5">
                <Skull className="w-4 h-4 text-danger" aria-hidden /> Hardcore
              </div>
              <div className="text-sm text-soft mt-0.5">
                One life: players turn into spectators when they die. Always on hard difficulty.
                Can't be changed after the world is made.
              </div>
            </div>
            <Toggle label="Hardcore" checked={hardcore} onChange={setHardcore} />
          </div>
        </div>
      </Disclosure>
    </form>
  );
};
