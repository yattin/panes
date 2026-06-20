import { beforeEach, describe, expect, it, vi } from "vitest";

const updaterMock = vi.hoisted(() => ({
  checkForAvailableUpdate: vi.fn(),
  relaunchAfterUpdate: vi.fn(),
}));

import { configureUpdateGateway } from "../contexts/software-update/application/updateGateway";
import { useUpdateStore } from "./updateStore";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushPromises() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("updateStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureUpdateGateway(updaterMock);
    useUpdateStore.setState({
      status: "idle",
      version: null,
      error: null,
      snoozed: false,
    });
  });

  it("does not start a second update check while one is already running", async () => {
    const updateCheck = deferred<null>();
    updaterMock.checkForAvailableUpdate.mockReturnValueOnce(updateCheck.promise);

    const firstCheck = useUpdateStore.getState().checkForUpdate();
    await flushPromises();

    const secondCheck = useUpdateStore.getState().checkForUpdate();
    await secondCheck;

    expect(updaterMock.checkForAvailableUpdate).toHaveBeenCalledTimes(1);
    expect(useUpdateStore.getState().status).toBe("checking");

    updateCheck.resolve(null);
    await firstCheck;

    expect(useUpdateStore.getState().status).toBe("idle");
  });
});
