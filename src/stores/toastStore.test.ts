import { beforeEach, describe, expect, it } from "vitest";
import { toast, useToastStore } from "./toastStore";

describe("toastStore", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("applies default durations and keeps only the newest toasts", () => {
    toast.success("saved");
    toast.info("loaded");
    toast.warning("slow");
    toast.error("failed");
    toast.info("retrying");
    toast.success("done");

    expect(useToastStore.getState().toasts).toEqual([
      expect.objectContaining({ message: "loaded", duration: 4000 }),
      expect.objectContaining({ message: "slow", duration: 6000 }),
      expect.objectContaining({ message: "failed", duration: 8000 }),
      expect.objectContaining({ message: "retrying", duration: 4000 }),
      expect.objectContaining({ message: "done", duration: 4000 }),
    ]);
  });
});
