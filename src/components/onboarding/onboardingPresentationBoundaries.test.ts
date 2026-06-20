import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("onboarding presentation boundaries", () => {
  it("does not import onboarding or engine infrastructure directly", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/onboarding/OnboardingWizard.tsx"),
      "utf8",
    );
    expect(source).not.toContain("contexts/onboarding/infrastructure");
    expect(source).not.toContain("contexts/engines/infrastructure");
  });

  it("does not import Tauri native APIs directly", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/onboarding/OnboardingWizard.tsx"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/plugin-dialog");
    expect(source).not.toContain("@tauri-apps/plugin-shell");
  });

  it("opens harness websites through an application boundary", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/onboarding/HarnessPanel.tsx"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/plugin-shell");
  });

  it("reads app information through an application boundary", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/onboarding/UpdateDialog.tsx"),
      "utf8",
    );

    expect(source).not.toContain("@tauri-apps/api/app");
  });
});
