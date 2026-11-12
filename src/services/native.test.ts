import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const isTauri = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke, isTauri }));

const { getAppInfo, NativeError } = await import("./native");

describe("native service", () => {
  beforeEach(() => {
    invoke.mockReset();
    isTauri.mockReset().mockReturnValue(true);
  });

  it("invokes the typed command", async () => {
    const info = { name: "Islandy", version: "0.1.0" };
    invoke.mockResolvedValue(info);
    await expect(getAppInfo()).resolves.toBe(info);
    expect(invoke).toHaveBeenCalledWith("get_app_info", undefined);
  });

  it("maps structured Rust errors", async () => {
    invoke.mockRejectedValue({ kind: "monitor", message: "no monitor" });
    const error = await getAppInfo().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(NativeError);
    expect(error).toMatchObject({ kind: "monitor", command: "get_app_info" });
  });

  it("maps plain-string errors (e.g. denied permission)", async () => {
    invoke.mockRejectedValue("get_app_info not allowed");
    await expect(getAppInfo()).rejects.toMatchObject({ kind: "unknown" });
  });

  it("fails fast outside the desktop shell", async () => {
    isTauri.mockReturnValue(false);
    await expect(getAppInfo()).rejects.toMatchObject({ kind: "unavailable" });
    expect(invoke).not.toHaveBeenCalled();
  });
});
