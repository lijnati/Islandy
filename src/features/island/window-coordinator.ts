import type { Size } from "../../services/types";

/**
 * Keeps the native frame (the window's hit/visible area) in step with the
 * animated island surface.
 *
 * Invariant: the native frame always contains the surface — the surface is
 * never clipped, yet the frame is only larger than the surface while an
 * animation is running. The rule that guarantees it:
 *
 * - Before animating to `target`, grow the frame to `union(frame, target)`
 *   (a no-op when shrinking) and only then start the animation.
 * - When the animation completes, shrink the frame to `target` — unless a
 *   newer transition has started meanwhile (generation check).
 *
 * Native calls are serialized, so they reach Rust in the order issued, and
 * are only made at transition boundaries, never per animation frame.
 */
export class WindowCoordinator {
  private frame: Size;
  private generation = 0;
  private queue: Promise<void> = Promise.resolve();
  private readonly apply: (size: Size) => Promise<void>;

  constructor(apply: (size: Size) => Promise<void>, initial: Size) {
    this.apply = apply;
    this.frame = initial;
  }

  /** The frame most recently requested from the native side. */
  get currentFrame(): Size {
    return this.frame;
  }

  /**
   * Prepares the native frame for an animation to `target`. Resolves with a
   * token once the animation may start (after any grow has been applied).
   */
  async begin(target: Size): Promise<number> {
    const token = ++this.generation;
    const needed = union(this.frame, target);
    if (!sameSize(needed, this.frame)) await this.request(needed);
    return token;
  }

  /** Called when the animation started by `begin` has finished. */
  async complete(token: number, target: Size): Promise<void> {
    if (token !== this.generation) return; // superseded by a newer transition
    if (!sameSize(target, this.frame)) await this.request(target);
  }

  /** Re-sends the current frame (e.g. after the monitor changed). */
  async resync(target: Size): Promise<void> {
    ++this.generation;
    await this.request(target);
  }

  private request(size: Size): Promise<void> {
    this.frame = size;
    const run = this.queue.then(() => this.apply(size));
    // Keep the queue alive even if a native call fails.
    this.queue = run.catch(() => undefined);
    return this.queue;
  }
}

export function union(a: Size, b: Size): Size {
  return { width: Math.max(a.width, b.width), height: Math.max(a.height, b.height) };
}

export function sameSize(a: Size, b: Size): boolean {
  return a.width === b.width && a.height === b.height;
}
