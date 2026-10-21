import { describe, expect, it } from "vitest";
import { formatBytes, megabytes } from "./storage";

describe("formatBytes", () => {
  it("counts megabytes below a gigabyte and gigabytes from there", () => {
    expect(formatBytes(0, "en")).toBe("0 MB");
    expect(formatBytes(312.4 * 1024 * 1024, "en")).toBe("312 MB");
    expect(formatBytes(1024 * 1024 * 1024, "en")).toBe("1 GB");
    expect(formatBytes(1.55 * 1024 * 1024 * 1024, "en")).toBe("1.6 GB");
  });

  it("turns a limit back into whole megabytes", () => {
    expect(megabytes(512 * 1024 * 1024)).toBe(512);
  });
});
