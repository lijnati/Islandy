import { describe, expect, it, vi } from "vitest";

import type { Size } from "../../services/types";
import { WindowCoordinator } from "./window-coordinator";

const COLLAPSED: Size = { width: 220, height: 44 };
const PEEK: Size = { width: 360, height: 48 };
const EXPANDED: Size = { width: 820, height: 248 };

function setup() {
  const calls: Size[] = [];
  const apply = vi.fn((size: Size) => {
    calls.push(size);
    return Promise.resolve();
  });
  return { coordinator: new WindowCoordinator(apply, COLLAPSED), calls, apply };
}

describe("WindowCoordinator", () => {
  it("grows the native frame before the expand animation starts", async () => {
    const { coordinator, calls } = setup();
    await coordinator.begin(EXPANDED);
    expect(calls).toEqual([EXPANDED]);
  });

  it("shrinks only after the collapse animation completes", async () => {
    const { coordinator, calls } = setup();
    const t1 = await coordinator.begin(EXPANDED);
    await coordinator.complete(t1, EXPANDED);
    const t2 = await coordinator.begin(COLLAPSED);
    expect(calls).toEqual([EXPANDED]); // still large while animating
    await coordinator.complete(t2, COLLAPSED);
    expect(calls).toEqual([EXPANDED, COLLAPSED]);
  });

  it("ignores completion of an animation that was superseded", async () => {
    const { coordinator, calls } = setup();
    await coordinator.begin(EXPANDED);
    const collapse = await coordinator.begin(COLLAPSED);
    await coordinator.begin(EXPANDED); // pointer came back mid-collapse
    await coordinator.complete(collapse, COLLAPSED);
    expect(calls).toEqual([EXPANDED]);
    expect(coordinator.currentFrame).toEqual(EXPANDED);
  });

  it("uses the union when sizes are not nested (taller but narrower)", async () => {
    const { coordinator, calls } = setup();
    const tall: Size = { width: 200, height: 60 };
    const t = await coordinator.begin(tall);
    expect(calls).toEqual([{ width: 220, height: 60 }]);
    await coordinator.complete(t, tall);
    expect(calls.at(-1)).toEqual(tall);
  });

  it("collapsed ⇄ peek round trip", async () => {
    const { coordinator, calls } = setup();
    const t1 = await coordinator.begin(PEEK);
    await coordinator.complete(t1, PEEK);
    const t2 = await coordinator.begin(COLLAPSED);
    await coordinator.complete(t2, COLLAPSED);
    expect(calls).toEqual([PEEK, COLLAPSED]);
  });

  it("serializes native calls in issue order, even when one is slow", async () => {
    const order: string[] = [];
    let release: () => void = () => undefined;
    const apply = vi.fn((size: Size) => {
      if (size.width === 820) {
        return new Promise<void>((resolve) => {
          release = () => {
            order.push("820");
            resolve();
          };
        });
      }
      order.push(String(size.width));
      return Promise.resolve();
    });
    const coordinator = new WindowCoordinator(apply, COLLAPSED);
    const grow = coordinator.begin(EXPANDED);
    const shrinkDone = coordinator.resync(COLLAPSED);
    await Promise.resolve();
    expect(order).toEqual([]);
    release();
    await grow;
    await shrinkDone;
    expect(order).toEqual(["820", "220"]);
  });

  it("keeps working after a native failure", async () => {
    const apply = vi.fn().mockRejectedValueOnce(new Error("boom")).mockResolvedValue(undefined);
    const coordinator = new WindowCoordinator(apply, COLLAPSED);
    await coordinator.begin(EXPANDED).catch(() => undefined);
    const t = await coordinator.begin(COLLAPSED);
    await coordinator.complete(t, COLLAPSED);
    expect(apply).toHaveBeenCalledTimes(2);
  });

  it("makes no native call when nothing changes", async () => {
    const { coordinator, apply } = setup();
    const t = await coordinator.begin(COLLAPSED);
    await coordinator.complete(t, COLLAPSED);
    expect(apply).not.toHaveBeenCalled();
  });
});
