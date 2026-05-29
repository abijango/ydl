import { cn } from "@/lib/utils";
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes } from "react";

export function Button({
  className,
  variant = "primary",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "ghost" | "outline" }) {
  const base =
    "inline-flex items-center justify-center gap-2 rounded-xl text-sm font-semibold tracking-tight transition-all duration-150 disabled:opacity-40 disabled:pointer-events-none active:scale-[0.98] select-none";
  const variants = {
    primary:
      "bg-[var(--color-accent)] text-[var(--color-accent-ink)] px-5 py-2.5 shadow-[0_1px_2px_rgba(0,0,0,0.10)] hover:bg-[var(--color-accent-strong)] hover:shadow-[0_4px_14px_-3px_rgba(132,204,22,0.5)]",
    ghost: "text-[var(--color-muted)] px-3 py-2 hover:text-[var(--color-ink)] hover:bg-[var(--color-hover)]",
    outline:
      "border border-[var(--color-line-strong)] text-[var(--color-ink)] px-4 py-2 hover:bg-[var(--color-hover)] hover:border-[var(--color-faint)]",
  };
  return <button className={cn(base, variants[variant], className)} {...props} />;
}

export function IconButton({
  className,
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode }) {
  return (
    <button
      className={cn(
        "grid place-items-center h-9 w-9 rounded-lg text-[var(--color-muted)] transition-colors hover:text-[var(--color-ink)] hover:bg-[var(--color-hover)] active:scale-95",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

export function TextField({
  label,
  hint,
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label?: string; hint?: string }) {
  return (
    <label className="block">
      {label && (
        <span className="mb-1.5 block text-xs font-medium uppercase tracking-[0.14em] text-[var(--color-faint)]">
          {label}
        </span>
      )}
      <input
        className={cn(
          "w-full rounded-lg bg-[var(--color-panel-2)] border border-[var(--color-line)] px-3.5 py-2.5 text-sm text-[var(--color-ink)] placeholder:text-[var(--color-faint)] outline-none transition-colors focus:border-[var(--color-accent)]/60 focus:bg-[var(--color-panel)] select-text",
          className,
        )}
        {...props}
      />
      {hint && <span className="mt-1.5 block text-xs text-[var(--color-faint)]">{hint}</span>}
    </label>
  );
}

export function SelectField({
  label,
  children,
  className,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement> & { label?: string }) {
  return (
    <label className="block">
      {label && (
        <span className="mb-1.5 block text-xs font-medium uppercase tracking-[0.14em] text-[var(--color-faint)]">
          {label}
        </span>
      )}
      <select
        className={cn(
          "w-full rounded-lg bg-[var(--color-panel-2)] border border-[var(--color-line)] px-3.5 py-2.5 text-sm text-[var(--color-ink)] outline-none transition-colors focus:border-[var(--color-accent)]/60",
          className,
        )}
        {...props}
      >
        {children}
      </select>
    </label>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="group inline-flex items-center gap-2.5"
    >
      <span
        className={cn(
          "relative h-[22px] w-[38px] rounded-full transition-colors duration-200",
          checked ? "bg-[var(--color-accent)]" : "bg-[var(--color-track)]",
        )}
      >
        <span
          className={cn(
            "absolute top-[3px] h-4 w-4 rounded-full bg-white shadow-sm transition-all duration-200",
            checked ? "left-[19px]" : "left-[3px]",
          )}
        />
      </span>
      {label && (
        <span className="text-sm text-[var(--color-muted)] group-hover:text-[var(--color-ink)] transition-colors">
          {label}
        </span>
      )}
    </button>
  );
}
