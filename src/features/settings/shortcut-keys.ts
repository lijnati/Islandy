/**
 * Turns a key press into an accelerator string such as `Ctrl+Alt+Space`
 * (the syntax Rust validates; `Win` is the Windows key). Pure, so the
 * recorder's behaviour is unit-tested.
 */
export interface KeyLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "OSLeft",
  "OSRight",
]);

/** `KeyA` → `A`, `Digit1` → `1`, `Space`, `F5`, `ArrowUp`, … */
function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  const named = new Set([
    "Space",
    "Enter",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Backquote",
    "Minus",
    "Equal",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Semicolon",
    "Quote",
    "Comma",
    "Period",
    "Slash",
  ]);
  return named.has(code) ? code : null;
}

export type RecordResult =
  | { kind: "pending"; preview: string }
  | { kind: "done"; accelerator: string }
  | { kind: "cancel" }
  | { kind: "invalid"; reason: string };

export function recordKey(e: KeyLike): RecordResult {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Win");
  if (MODIFIER_CODES.has(e.code)) {
    return { kind: "pending", preview: mods.length ? `${mods.join(" + ")} + …` : "…" };
  }
  if (e.code === "Escape" && mods.length === 0) return { kind: "cancel" };
  const key = keyName(e.code);
  if (!key) return { kind: "invalid", reason: "That key can't be used in a shortcut." };
  if (!e.ctrlKey && !e.altKey && !e.metaKey) {
    return { kind: "invalid", reason: "Use Ctrl, Alt or Win with another key." };
  }
  return { kind: "done", accelerator: [...mods, key].join("+") };
}

/** `Ctrl+Alt+Space` → `Ctrl + Alt + Space`. */
export const displayShortcut = (accelerator: string) => accelerator.split("+").join(" + ");
