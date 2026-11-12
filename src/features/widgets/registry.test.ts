import { describe, expect, it } from "vitest";

import type { WidgetEntry } from "../../services/types";
import { DEFAULT_SETTINGS } from "../settings/defaults";
import { fitWidgets, WIDGETS } from "./registry";

const items = (...ids: [WidgetEntry["id"], boolean][]): WidgetEntry[] =>
  ids.map(([id, enabled]) => ({ id, enabled }));

describe("widget registry", () => {
  it("declares every widget with a stable id", () => {
    for (const [id, def] of Object.entries(WIDGETS)) {
      expect(def.id).toBe(id);
      expect(def.minWidth).toBeGreaterThan(150);
    }
  });

  it("shows the default layout on a full-width dashboard", () => {
    expect(fitWidgets(DEFAULT_SETTINGS.widgets.items, 820)).toEqual([
      "media",
      "calendar",
      "activity",
    ]);
  });

  it("follows the user's order and skips disabled widgets", () => {
    expect(
      fitWidgets(
        items(["audio", true], ["media", false], ["system", true], ["calendar", true]),
        820,
      ),
    ).toEqual(["audio", "system", "calendar"]);
  });

  it("drops what doesn't fit but keeps later widgets that do", () => {
    // 520 px dashboard: 488 px of room. Media (300) fits, activity (200+12) doesn't, calendar (204+12) doesn't, ...
    expect(fitWidgets(items(["media", true], ["activity", true], ["calendar", true]), 520)).toEqual(
      ["media"],
    );
    // Calendar (204) + audio (220+12) = 436 ≤ 488.
    expect(fitWidgets(items(["media", false], ["calendar", true], ["audio", true]), 520)).toEqual([
      "calendar",
      "audio",
    ]);
    expect(fitWidgets(items(["media", true]), 100)).toEqual([]);
  });
});
