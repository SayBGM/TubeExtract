import { describe, expect, it, vi } from "vitest";
import { clearCompletedQueueJobs, queueActions } from "./queueActions";

const {
  pauseJobAndGetSnapshotMock,
  resumeJobAndGetSnapshotMock,
  cancelJobAndGetSnapshotMock,
  openFolderMock,
  deleteFileAndGetSnapshotMock,
  clearTerminalJobsAndGetSnapshotMock,
} = vi.hoisted(() => ({
  pauseJobAndGetSnapshotMock: vi.fn(),
  resumeJobAndGetSnapshotMock: vi.fn(),
  cancelJobAndGetSnapshotMock: vi.fn(),
  openFolderMock: vi.fn(),
  deleteFileAndGetSnapshotMock: vi.fn(),
  clearTerminalJobsAndGetSnapshotMock: vi.fn(),
}));

vi.mock("../../lib/desktopClient", () => ({
  pauseJobAndGetSnapshot: pauseJobAndGetSnapshotMock,
  resumeJobAndGetSnapshot: resumeJobAndGetSnapshotMock,
  cancelJobAndGetSnapshot: cancelJobAndGetSnapshotMock,
  openFolder: openFolderMock,
  deleteFileAndGetSnapshot: deleteFileAndGetSnapshotMock,
  clearTerminalJobsAndGetSnapshot: clearTerminalJobsAndGetSnapshotMock,
}));

describe("queueActions", () => {
  it("forwards pause/resume/cancel/open/delete actions", async () => {
    pauseJobAndGetSnapshotMock.mockResolvedValue({ items: [] });
    resumeJobAndGetSnapshotMock.mockResolvedValue({ items: [] });
    cancelJobAndGetSnapshotMock.mockResolvedValue({ items: [] });
    deleteFileAndGetSnapshotMock.mockResolvedValue({ items: [] });

    await queueActions.pauseJob("a");
    await queueActions.resumeJob("b");
    await queueActions.cancelJob("c");
    await queueActions.openFolder("/tmp");
    await queueActions.deleteFile("/tmp/a.mp4");

    expect(pauseJobAndGetSnapshotMock).toHaveBeenCalledWith("a");
    expect(resumeJobAndGetSnapshotMock).toHaveBeenCalledWith("b");
    expect(cancelJobAndGetSnapshotMock).toHaveBeenCalledWith("c");
    expect(openFolderMock).toHaveBeenCalledWith("/tmp");
    expect(deleteFileAndGetSnapshotMock).toHaveBeenCalledWith("/tmp/a.mp4");
  });

  it("clears terminal jobs and returns queue snapshot items from the command response", async () => {
    clearTerminalJobsAndGetSnapshotMock.mockResolvedValue({
      items: [{ id: "1", title: "job" }],
    });

    const items = await clearCompletedQueueJobs();

    expect(clearTerminalJobsAndGetSnapshotMock).toHaveBeenCalledTimes(1);
    expect(items).toEqual([{ id: "1", title: "job" }]);
  });
});
