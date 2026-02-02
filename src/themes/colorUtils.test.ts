import { describe, it, expect } from 'vitest';
import {
  hexToRgb,
  rgbToHex,
  lighten,
  darken,
  generateColorScale,
} from './colorUtils';

describe('colorUtils', () => {
  describe('hexToRgb', () => {
    it('should convert 6-digit hex to RGB', () => {
      expect(hexToRgb('#FF5733')).toEqual({ r: 255, g: 87, b: 51 });
      expect(hexToRgb('#000000')).toEqual({ r: 0, g: 0, b: 0 });
      expect(hexToRgb('#FFFFFF')).toEqual({ r: 255, g: 255, b: 255 });
    });

    it('should convert 3-digit hex to RGB', () => {
      expect(hexToRgb('#F53')).toEqual({ r: 255, g: 85, b: 51 });
      expect(hexToRgb('#000')).toEqual({ r: 0, g: 0, b: 0 });
      expect(hexToRgb('#FFF')).toEqual({ r: 255, g: 255, b: 255 });
    });

    it('should handle hex without hash', () => {
      expect(hexToRgb('FF5733')).toEqual({ r: 255, g: 87, b: 51 });
      expect(hexToRgb('F53')).toEqual({ r: 255, g: 85, b: 51 });
    });

    it('should return null for invalid hex', () => {
      expect(hexToRgb('#GGGGGG')).toBeNull();
      expect(hexToRgb('#12')).toBeNull();
      expect(hexToRgb('invalid')).toBeNull();
      expect(hexToRgb('')).toBeNull();
    });
  });

  describe('rgbToHex', () => {
    it('should convert RGB to hex', () => {
      expect(rgbToHex(255, 87, 51)).toBe('#ff5733');
      expect(rgbToHex(0, 0, 0)).toBe('#000000');
      expect(rgbToHex(255, 255, 255)).toBe('#ffffff');
    });

    it('should handle single digit hex values', () => {
      expect(rgbToHex(0, 15, 0)).toBe('#000f00');
      expect(rgbToHex(1, 2, 3)).toBe('#010203');
    });

    it('should round decimal values', () => {
      expect(rgbToHex(255.4, 87.3, 51.9)).toBe('#ff5734');
    });
  });

  describe('lighten', () => {
    it('should lighten a color by given amount', () => {
      const original = '#808080'; // Medium gray
      const lightened = lighten(original, 0.5);
      expect(lightened).not.toBe(original);
      expect(lightened.startsWith('#')).toBe(true);
    });

    it('should return white when fully lightened', () => {
      expect(lighten('#000000', 1)).toBe('#ffffff');
    });

    it('should return same color when amount is 0', () => {
      expect(lighten('#FF5733', 0)).toBe('#ff5733');
    });

    it('should return original hex if invalid', () => {
      expect(lighten('invalid', 0.5)).toBe('invalid');
    });

    it('should not exceed white (255)', () => {
      const result = lighten('#FFFFFF', 0.5);
      expect(result).toBe('#ffffff');
    });
  });

  describe('darken', () => {
    it('should darken a color by given amount', () => {
      const original = '#808080';
      const darkened = darken(original, 0.5);
      expect(darkened).not.toBe(original);
      expect(darkened.startsWith('#')).toBe(true);
    });

    it('should return black when fully darkened', () => {
      expect(darken('#FFFFFF', 1)).toBe('#000000');
    });

    it('should return same color when amount is 0', () => {
      expect(darken('#FF5733', 0)).toBe('#ff5733');
    });

    it('should return original hex if invalid', () => {
      expect(darken('invalid', 0.5)).toBe('invalid');
    });

    it('should not go below black (0)', () => {
      const result = darken('#000000', 0.5);
      expect(result).toBe('#000000');
    });
  });

  describe('generateColorScale', () => {
    it('should generate a color scale with 9 steps', () => {
      const scale = generateColorScale('#3B82F6');
      expect(scale).toHaveProperty('50');
      expect(scale).toHaveProperty('100');
      expect(scale).toHaveProperty('200');
      expect(scale).toHaveProperty('300');
      expect(scale).toHaveProperty('400');
      expect(scale).toHaveProperty('500');
      expect(scale).toHaveProperty('600');
      expect(scale).toHaveProperty('700');
      expect(scale).toHaveProperty('800');
      expect(scale).toHaveProperty('900');
    });

    it('should use base color as 500', () => {
      const baseColor = '#3B82F6';
      const scale = generateColorScale(baseColor);
      expect(scale[500]).toBe(baseColor);
    });

    it('should have lighter colors at lower indices', () => {
      const scale = generateColorScale('#3B82F6');
      // 50 should be lighter than 500
      const fifty = hexToRgb(scale[50]);
      const fiveHundred = hexToRgb(scale[500]);
      expect(fifty!.r + fifty!.g + fifty!.b).toBeGreaterThan(fiveHundred!.r + fiveHundred!.g + fiveHundred!.b);
    });

    it('should have darker colors at higher indices', () => {
      const scale = generateColorScale('#3B82F6');
      // 900 should be darker than 500
      const nineHundred = hexToRgb(scale[900]);
      const fiveHundred = hexToRgb(scale[500]);
      expect(nineHundred!.r + nineHundred!.g + nineHundred!.b).toBeLessThan(fiveHundred!.r + fiveHundred!.g + fiveHundred!.b);
    });
  });
});
