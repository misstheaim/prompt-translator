import { useId, type InputHTMLAttributes, type KeyboardEvent, type ReactNode } from "react";

import { cx } from "../../lib/cx";

/** A titled group of fields. */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="rounded-lg border border-border bg-surface p-4">
      <h2 className="text-sm font-semibold text-text">{title}</h2>
      {description && <p className="mt-0.5 text-xs text-muted">{description}</p>}
      <div className="mt-3 flex flex-col gap-3">{children}</div>
    </section>
  );
}

interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange" | "value"> {
  label: string;
  value: string | number;
  onChange: (value: string) => void;
  hint?: string;
  error?: string;
  /** Extra controls next to the input (a Browse button, ...). */
  trailing?: ReactNode;
}

/** Labelled text/number input with a hint and an inline error. */
export function TextField({
  label,
  value,
  onChange,
  hint,
  error,
  trailing,
  className,
  id,
  ...rest
}: FieldProps) {
  const autoId = useId();
  const inputId = id ?? autoId;
  const noteId = `${inputId}-note`;
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={inputId} className="text-xs font-medium text-text">
        {label}
      </label>
      <div className="flex items-center gap-2">
        <input
          id={inputId}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={error ? true : undefined}
          aria-describedby={error || hint ? noteId : undefined}
          className={cx(
            "h-8 min-w-0 flex-1 rounded-md border bg-surface px-2 text-sm text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50",
            error ? "border-danger" : "border-border",
            className,
          )}
          {...rest}
        />
        {trailing}
      </div>
      {(error || hint) && (
        <p id={noteId} className={cx("text-xs", error ? "text-danger" : "text-muted")}>
          {error ?? hint}
        </p>
      )}
    </div>
  );
}

/** Checkbox row with a label and an optional description. */
export function Toggle({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className={cx("flex items-start gap-2", disabled && "opacity-50")}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 size-4 accent-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed"
      />
      <label htmlFor={id} className="flex flex-col">
        <span className="text-sm text-text">{label}</span>
        {description && <span className="text-xs text-muted">{description}</span>}
      </label>
    </div>
  );
}

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph"]);

/** The key part of a global-shortcut string ("Space", "A", "F5"), or null for unsupported keys. */
function keyName(e: KeyboardEvent): string | null {
  const code = e.code;
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  const named: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Insert: "Insert",
    Delete: "Delete",
    Pause: "Pause",
  };
  return named[code] ?? null;
}

/** Read-only input that records the pressed key combination as e.g. `Ctrl+Alt+Space` or `Shift+Pause`. */
export function HotkeyField({
  label,
  value,
  onChange,
  hint,
  error,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  error?: string;
  disabled?: boolean;
}) {
  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) return; // keep keyboard navigation
    e.preventDefault();
    if (MODIFIER_KEYS.has(e.key)) return;
    const key = keyName(e);
    if (!key) return;
    const mods = [
      e.ctrlKey && "Ctrl",
      e.altKey && "Alt",
      e.shiftKey && "Shift",
      e.metaKey && "Super",
    ].filter(Boolean) as string[];
    if (mods.length === 0) return; // a global hotkey needs a modifier
    onChange([...mods, key].join("+"));
  }

  return (
    <TextField
      label={label}
      value={value}
      onChange={() => undefined}
      onKeyDown={onKeyDown}
      readOnly
      disabled={disabled}
      hint={hint ?? "Click the field and press the new combination (at least one modifier)."}
      error={error}
    />
  );
}
