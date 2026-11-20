import { describe, expect, it } from 'vitest';
import { bucketPeaks, playedBars } from './waveform';

describe('waveform', () => {
  it('takes the peak of each bucket and scales to 1', () => {
    expect(bucketPeaks([[0, 0.1, -0.5, 0.2, 0, 0, 0.25, -0.25]], 4)).toEqual([0.2, 1, 0, 0.5]);
  });
  it('downmixes channels and keeps silence flat', () => {
    expect(bucketPeaks([[1, 1], [-1, -1]], 2)).toEqual([0, 0]);
    expect(bucketPeaks([[0, 0, 0, 0]], 2)).toEqual([0, 0]);
    expect(bucketPeaks([], 4)).toEqual([]);
  });
  it('has the requested number of bars even for short audio', () => {
    expect(bucketPeaks([[0.5]], 48)).toHaveLength(48);
  });
  it('counts played bars', () => {
    expect(playedBars(0, 10, 48)).toBe(0);
    expect(playedBars(5, 10, 48)).toBe(24);
    expect(playedBars(99, 10, 48)).toBe(48);
    expect(playedBars(1, 0, 48)).toBe(0);
  });
});
