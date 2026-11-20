import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const css = (name: string) => readFileSync(new URL(`../${name}`, import.meta.url), 'utf8');

/** Colours of "selected" controls are set in look.css (ink with on-ink text). A later file that repaints the background
 * of the same control puts the label on the wrong colour: it disappears. */
describe('selected controls', () => {
  it('are not repainted by the layers loaded after look.css', () => {
    for (const file of ['polish.css', 'wow.css']) {
      const rules = [...css(file).matchAll(/([^{}]+)\{([^{}]*)\}/g)];
      for (const [, selector, body] of rules) {
        if (/(^|[\s,])\.(tab|settings-nav-button|segmented button)\.active|aria-(selected|pressed)/.test(selector!) && /background\s*:/.test(body!)) {
          throw new Error(`${file}: "${selector!.trim()}" sets a background, which hides the selected label`);
        }
      }
    }
    expect(true).toBe(true);
  });
});
