import { describe, test, expect } from "vitest";
import { formatTokens } from "./AccountsPanel";

describe("formatTokens", () => {
  test("formats millions", () => expect(formatTokens(1_800_000)).toBe("1.8M"));
  test("formats thousands", () => expect(formatTokens(212_000)).toBe("212K"));
  test("formats small", () => expect(formatTokens(500)).toBe("500"));
  test("formats zero", () => expect(formatTokens(0)).toBe("0"));
  test("rounds millions to 1dp", () => expect(formatTokens(2_500_000)).toBe("2.5M"));
  test("rounds thousands to integer", () => expect(formatTokens(1_500)).toBe("2K"));
  test("large K rounds to M boundary", () => expect(formatTokens(999_500)).toBe("1.0M"));
});
