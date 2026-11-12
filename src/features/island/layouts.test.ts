import { describe, expect, it } from "vitest";

import { designLayouts } from "../../design/tokens";
import { fitToViewport, layoutFor } from "./layouts";

describe("layouts", () => {
  it("keeps design targets in a large viewport", () => {
    expect(fitToViewport({ width: 1920, height: 1080 })).toEqual(designLayouts);
  });

  it("narrows the dashboard in a small viewport", () => {
    const l = fitToViewport({ width: 640, height: 400 });
    expect(l.expanded).toEqual({ width: 616, height: 248 });
    expect(l.collapsed).toEqual(designLayouts.collapsed);
  });

  it("maps modes to layouts", () => {
    expect(layoutFor("collapsed", designLayouts)).toBe(designLayouts.collapsed);
    expect(layoutFor("peek", designLayouts)).toBe(designLayouts.peek);
    expect(layoutFor("expanded", designLayouts)).toBe(designLayouts.expanded);
    expect(layoutFor("focused", designLayouts)).toBe(designLayouts.expanded);
  });

  it("widens only the collapsed pill for ambient activity", () => {
    expect(layoutFor("collapsed", designLayouts, true)).toBe(designLayouts.ambient);
    expect(layoutFor("peek", designLayouts, true)).toBe(designLayouts.peek);
    expect(layoutFor("expanded", designLayouts, true)).toBe(designLayouts.expanded);
    // The ambient pill fits inside the canvas every other layout defines.
    expect(designLayouts.ambient.width).toBeLessThan(designLayouts.peek.width);
    expect(designLayouts.ambient.height).toBe(designLayouts.collapsed.height);
  });
});
