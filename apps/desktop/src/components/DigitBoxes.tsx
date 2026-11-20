import { useEffect, useRef } from 'react';

/**
 * Six one-digit boxes: typing moves on, backspace moves back, pasting fills them, and `onComplete` fires once the
 * sixth digit is in (no button to press). Plain inputs, so keyboard, paste and screen readers all work.
 */
export function DigitBoxes({
  value,
  onChange,
  onComplete,
  disabled,
  label,
}: {
  value: string;
  onChange: (digits: string) => void;
  onComplete: (digits: string) => void;
  disabled?: boolean;
  label: string;
}) {
  const refs = useRef<(HTMLInputElement | null)[]>([]);
  useEffect(() => {
    if (!disabled) refs.current[Math.min(value.length, 5)]?.focus();
  }, [value.length, disabled]);

  const set = (digits: string) => {
    const clean = digits.replace(/\D/g, '').slice(0, 6);
    onChange(clean);
    if (clean.length === 6) onComplete(clean);
  };

  return (
    <div className="digit-boxes" role="group" aria-label={label}>
      {Array.from({ length: 6 }, (_, i) => (
        <input
          key={i}
          ref={(el) => {
            refs.current[i] = el;
          }}
          className="digit-box"
          inputMode="numeric"
          autoComplete="one-time-code"
          maxLength={1}
          disabled={disabled}
          aria-label={`Digit ${i + 1} of 6`}
          value={value[i] ?? ''}
          onChange={(e) => {
            const d = e.target.value.replace(/\D/g, '');
            if (d) set(value.slice(0, i) + d + value.slice(i + 1));
          }}
          onKeyDown={(e) => {
            if (e.key === 'Backspace') {
              e.preventDefault();
              set(value.slice(0, value[i] ? i : Math.max(i - 1, 0)));
            }
          }}
          onPaste={(e) => {
            e.preventDefault();
            set(e.clipboardData.getData('text'));
          }}
          onFocus={(e) => e.target.select()}
        />
      ))}
    </div>
  );
}
