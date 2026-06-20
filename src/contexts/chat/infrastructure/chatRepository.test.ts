import { beforeEach, describe, expect, it, vi } from "vitest";

const mockIpc = vi.hoisted(() => ({
  cancelTurn: vi.fn(),
  compactNativeThread: vi.fn(),
  confirmWorkspaceThread: vi.fn(),
  getContextMaxTokens: vi.fn(),
  getActionOutput: vi.fn(),
  getNativeHistoryTokens: vi.fn(),
  getOpenCodeRuntimeCatalog: vi.fn(),
  getThreadMessagesWindow: vi.fn(),
  listCodexApps: vi.fn(),
  listCodexRemoteThreads: vi.fn(),
  listOpenCodeRemoteSessions: vi.fn(),
  listCodexSkills: vi.fn(),
  prewarmEngine: vi.fn(),
  readAttachmentPreview: vi.fn(),
  respondApproval: vi.fn(),
  savePastedImageAttachment: vi.fn(),
  searchMessages: vi.fn(),
  sendMessage: vi.fn(),
  setThreadCodexConfig: vi.fn(),
  setThreadExecutionPolicy: vi.fn(),
  setThreadOpenCodeConfig: vi.fn(),
  setThreadReasoningEffort: vi.fn(),
  startCodexReview: vi.fn(),
  steerMessage: vi.fn(),
  syncThreadFromEngine: vi.fn(),
}));

const mockListenChatTurnFinished = vi.hoisted(() => vi.fn());

vi.mock("../../../lib/ipc", () => ({
  ipc: mockIpc,
  listenChatTurnFinished: mockListenChatTurnFinished,
}));

import { chatRepository } from "./chatRepository";

describe("chatRepository", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reads attachment previews through the native chat adapter", async () => {
    const preview = { mimeType: "image/png", dataBase64: "abc" };
    mockIpc.readAttachmentPreview.mockResolvedValue(preview);

    await expect(
      chatRepository.readAttachmentPreview("C:/tmp/image.png", "image/png"),
    ).resolves.toBe(preview);

    expect(mockIpc.readAttachmentPreview).toHaveBeenCalledWith(
      "C:/tmp/image.png",
      "image/png",
    );
  });

  it("lists Codex remote threads through the native chat adapter", async () => {
    const page = { threads: [], nextCursor: null };
    mockIpc.listCodexRemoteThreads.mockResolvedValue(page);

    await expect(
      chatRepository.listCodexRemoteThreads("workspace-1", {
        cursor: null,
        limit: 20,
        searchTerm: "query",
        archived: false,
      }),
    ).resolves.toBe(page);

    expect(mockIpc.listCodexRemoteThreads).toHaveBeenCalledWith("workspace-1", {
      cursor: null,
      limit: 20,
      searchTerm: "query",
      archived: false,
    });
  });

  it("lists OpenCode remote sessions through the native chat adapter", async () => {
    const page = { sessions: [], nextCursor: null };
    mockIpc.listOpenCodeRemoteSessions.mockResolvedValue(page);

    await expect(
      chatRepository.listOpenCodeRemoteSessions("workspace-1", {
        cursor: "cursor-1",
        limit: 20,
        searchTerm: null,
        archived: true,
      }),
    ).resolves.toBe(page);

    expect(mockIpc.listOpenCodeRemoteSessions).toHaveBeenCalledWith("workspace-1", {
      cursor: "cursor-1",
      limit: 20,
      searchTerm: null,
      archived: true,
    });
  });

  it("searches workspace messages through the native chat adapter", async () => {
    const results = [{ threadId: "thread-1", messageId: "message-1", preview: "match" }];
    mockIpc.searchMessages.mockResolvedValue(results);

    await expect(chatRepository.searchMessages("workspace-1", "match")).resolves.toBe(results);

    expect(mockIpc.searchMessages).toHaveBeenCalledWith("workspace-1", "match");
  });

  it("starts Codex reviews through the native chat adapter", async () => {
    const thread = { id: "thread-1" };
    mockIpc.startCodexReview.mockResolvedValue(thread);

    await expect(
      chatRepository.startCodexReview(
        "thread-1",
        { type: "uncommittedChanges" },
        "inline",
      ),
    ).resolves.toBe(thread);

    expect(mockIpc.startCodexReview).toHaveBeenCalledWith(
      "thread-1",
      { type: "uncommittedChanges" },
      "inline",
    );
  });

  it("listens for finished chat turns through the native chat adapter", async () => {
    const unlisten = vi.fn();
    const onEvent = vi.fn();
    mockListenChatTurnFinished.mockResolvedValue(unlisten);

    await expect(chatRepository.listenChatTurnFinished(onEvent)).resolves.toBe(unlisten);

    expect(mockListenChatTurnFinished).toHaveBeenCalledWith(onEvent);
  });

  it("loads Codex references and prewarms engines through the native chat adapter", async () => {
    const skills = [{ name: "skill-1" }];
    const apps = [{ id: "app-1" }];
    mockIpc.listCodexSkills.mockResolvedValue(skills);
    mockIpc.listCodexApps.mockResolvedValue(apps);
    mockIpc.prewarmEngine.mockResolvedValue(undefined);

    await expect(chatRepository.listCodexSkills("C:/repo")).resolves.toBe(skills);
    await expect(chatRepository.listCodexApps()).resolves.toBe(apps);
    await expect(chatRepository.prewarmEngine("codex")).resolves.toBeUndefined();

    expect(mockIpc.listCodexSkills).toHaveBeenCalledWith("C:/repo");
    expect(mockIpc.listCodexApps).toHaveBeenCalledWith();
    expect(mockIpc.prewarmEngine).toHaveBeenCalledWith("codex");
  });

  it("loads OpenCode runtime catalogs through the native chat adapter", async () => {
    const catalog = { agents: [] };
    mockIpc.getOpenCodeRuntimeCatalog.mockResolvedValue(catalog);

    await expect(chatRepository.getOpenCodeRuntimeCatalog("C:/repo")).resolves.toBe(catalog);

    expect(mockIpc.getOpenCodeRuntimeCatalog).toHaveBeenCalledWith("C:/repo");
  });

  it("manages native thread context through the native chat adapter", async () => {
    mockIpc.getNativeHistoryTokens.mockResolvedValue(12);
    mockIpc.getContextMaxTokens.mockResolvedValue(200);
    mockIpc.compactNativeThread.mockResolvedValue([10, 3]);

    await expect(chatRepository.getNativeHistoryTokens("engine-thread-1")).resolves.toBe(12);
    await expect(chatRepository.getContextMaxTokens()).resolves.toBe(200);
    await expect(chatRepository.compactNativeThread("engine-thread-1")).resolves.toEqual([10, 3]);

    expect(mockIpc.getNativeHistoryTokens).toHaveBeenCalledWith("engine-thread-1");
    expect(mockIpc.getContextMaxTokens).toHaveBeenCalledWith();
    expect(mockIpc.compactNativeThread).toHaveBeenCalledWith("engine-thread-1");
  });

  it("persists thread settings through the native chat adapter", async () => {
    const updatedThread = { id: "thread-1" };
    mockIpc.setThreadCodexConfig.mockResolvedValue(updatedThread);
    mockIpc.setThreadExecutionPolicy.mockResolvedValue(updatedThread);
    mockIpc.setThreadOpenCodeConfig.mockResolvedValue(updatedThread);
    mockIpc.setThreadReasoningEffort.mockResolvedValue(undefined);
    mockIpc.confirmWorkspaceThread.mockResolvedValue(undefined);

    await expect(
      chatRepository.setThreadCodexConfig("thread-1", { serviceTier: "auto" }),
    ).resolves.toBe(updatedThread);
    await expect(
      chatRepository.setThreadExecutionPolicy("thread-1", { sandboxMode: "workspace-write" }),
    ).resolves.toBe(updatedThread);
    await expect(
      chatRepository.setThreadOpenCodeConfig("thread-1", { agent: "build" }),
    ).resolves.toBe(updatedThread);
    await expect(
      chatRepository.setThreadReasoningEffort("thread-1", "medium", "gpt-5"),
    ).resolves.toBeUndefined();
    await expect(
      chatRepository.confirmWorkspaceThread("thread-1", ["C:/repo"]),
    ).resolves.toBeUndefined();

    expect(mockIpc.setThreadCodexConfig).toHaveBeenCalledWith("thread-1", {
      serviceTier: "auto",
    });
    expect(mockIpc.setThreadExecutionPolicy).toHaveBeenCalledWith("thread-1", {
      sandboxMode: "workspace-write",
    });
    expect(mockIpc.setThreadOpenCodeConfig).toHaveBeenCalledWith("thread-1", {
      agent: "build",
    });
    expect(mockIpc.setThreadReasoningEffort).toHaveBeenCalledWith(
      "thread-1",
      "medium",
      "gpt-5",
    );
    expect(mockIpc.confirmWorkspaceThread).toHaveBeenCalledWith("thread-1", ["C:/repo"]);
  });

  it("saves pasted image attachments through the native chat adapter", async () => {
    const attachment = { kind: "image", fileName: "paste.png" };
    mockIpc.savePastedImageAttachment.mockResolvedValue(attachment);

    await expect(
      chatRepository.savePastedImageAttachment("paste.png", "image/png", "abc"),
    ).resolves.toBe(attachment);

    expect(mockIpc.savePastedImageAttachment).toHaveBeenCalledWith(
      "paste.png",
      "image/png",
      "abc",
    );
  });
});
