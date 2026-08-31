import { Toggle } from "./Toggle";

interface SettingRowProps {
  title: string;
  description: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** One toggle row in the Settings panel's "Output" section — title +
 * description + toggle, matching the reference screenshots' layout. */
export function SettingRow({ title, description, checked, onCheckedChange, disabled }: SettingRowProps) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-glass-border py-3 last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-text-primary">{title}</div>
        <div className="mt-0.5 text-xs text-text-secondary">{description}</div>
      </div>
      <Toggle checked={checked} onCheckedChange={onCheckedChange} disabled={disabled} label={title} />
    </div>
  );
}
