import * as Switch from "@radix-ui/react-switch";

interface ToggleProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string; // used only for aria-label — visible label is the caller's own text
}

/** Design-consultation preview's exact toggle treatment: 34x19px track (no
 * border — the lensing rim's own host `.glass-panel` ancestor already
 * provides edge definition; this control doesn't need its own), 15px thumb,
 * translateX(17px) on check / translateX(2px) off (falls out of the same
 * 2px inset both states share). off=surface-toggle-track/on=surface-filled;
 * off-thumb=text-primary/on-thumb=near-black. */
export function Toggle({ checked, onCheckedChange, disabled, label }: ToggleProps) {
  return (
    <Switch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className="relative h-[19px] w-[34px] shrink-0 rounded-pill transition-colors duration-base ease-standard focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
      style={{
        backgroundColor: checked ? "var(--surface-filled)" : "var(--surface-toggle-track)",
      }}
    >
      <Switch.Thumb
        className="block h-[15px] w-[15px] rounded-full transition-transform duration-base ease-standard"
        style={{
          transform: checked ? "translateX(17px)" : "translateX(2px)",
          backgroundColor: checked ? "#1c1c1e" : "var(--text-primary)",
        }}
      />
    </Switch.Root>
  );
}
