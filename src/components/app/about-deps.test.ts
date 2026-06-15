import { describe, expect, it } from "vitest";
import { DEPENDENCIES } from "@/components/app/about-deps";

describe("about-deps", () => {
  it("每项依赖都有必填字段", () => {
    for (const dep of DEPENDENCIES) {
      expect(dep.name).toBeTruthy();
      expect(dep.kind).toMatch(/^(frontend|runtime)$/);
      expect(dep.license).toBeTruthy();
      expect(dep.url).toMatch(/^https?:\/\//);
    }
  });

  it("没有重复的依赖名", () => {
    const names = DEPENDENCIES.map((d) => d.name);
    const unique = new Set(names);
    expect(unique.size).toBe(names.length);
  });
});
