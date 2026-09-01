import { AudioWaveform } from "lucide-react";

/** Step 0 — static copy, no backend call (matches the reference screenshots'
 * opening step, adapted to Mutter's real feature set). */
export function Welcome() {
  return (
    <div className="flex flex-col items-center text-center">
      <div
        aria-hidden="true"
        className="mb-4 flex h-11 w-11 items-center justify-center rounded-full text-text-primary"
        style={{ backgroundColor: "var(--surface-active)" }}
      >
        <AudioWaveform size={20} strokeWidth={2} />
      </div>
      <h1 className="text-lg font-semibold text-text-primary">Welcome to Mutter</h1>
      <p className="mt-2 text-sm text-text-secondary">
        Local, private speech-to-text — no audio or transcript ever leaves your Mac. Press a
        hotkey, speak, and Mutter types what you said wherever your cursor is.
      </p>
    </div>
  );
}
