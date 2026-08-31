import * as Switch from "@radix-ui/react-switch";

interface ToggleProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string; // used only for aria-label — visible label is the caller's own text
}

/** DESIGN.md's toggle-switch spec: 36x20px track, radius-pill,
 * 1px solid glass-border, off=surface-toggle-track/on=surface-filled;
 * 14px thumb, translateX(16px) on check, off=text-primary/on near-black. */
export function Toggle({ checked, onCheckedChange, disabled, label }: ToggleProps) {
  return (
    <Switch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className="relative h-5 w-9 shrink-0 rounded-pill border border-glass-border transition-colors duration-base ease-standard focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
      style={{
        backgroundColor: checked ? "var(--surface-filled)" : "var(--surface-toggle-track)",
      }}
    >
      <Switch.Thumb
        className="block h-3.5 w-3.5 rounded-full transition-transform duration-base ease-standard"
        style={{
          transform: checked ? "translateX(17px)" : "translateX(2px)",
          backgroundColor: checked ? "#1c1c1e" : "var(--text-primary)",
        }}
      />
    </Switch.Root>
  );
}
