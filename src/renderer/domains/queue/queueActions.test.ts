import { describe, expect, it, vi } from "vitest";
import { clearCompletedQueueJobs, queueActions } from "./queueActions";

const {
  pauseJobMock,
  resumeJobMock,
  cancelJobMock,
  openFolderMock,
  deleteFileMock,
  clearTerminalJobsMock,
  getQueueSnapshotMock,
} = vi.hoisted(() => ({
  pauseJobMock: vi.fn(),
  resumeJobMock: vi.fn(),
  cancelJobMock: vi.fn(),
  openFolderMock: vi.fn(),
  deleteFileMock: vi.fn(),
  clearTerminalJobsMock: vi.fn(),
  getQueueSnapshotMock: vi.fn(),
}));

vi.mock("../../lib/electronClient", () => ({
  pauseJob: pauseJobMock,
  resumeJob: resumeJobMock,
  cancelJob: cancelJobMock,
  openFolder: openFolderMock,
  deleteFile: deleteFileMock,
  clearTerminalJobs: clearTerminalJobsMock,
  getQueueSnapshot: getQueueSnapshotMock,
}));

describe("queueActions", () => {
  it("forwards pause/resume/cancel/open/delete actions", async () => {
    await queueActions.pauseJob("a");
    await queueActions.resumeJob("b");
    await queueActions.cancelJob("c");
    await queueActions.openFolder("/tmp");
    await queueActions.deleteFile("/tmp/a.mp4");

    expect(pauseJobMock).toHaveBeenCalledWith("a");
    expect(resumeJobMock).toHaveBeenCalledWith("b");
    expect(cancelJobMock).toHaveBeenCalledWith("c");
    expect(openFolderMock).toHaveBeenCalledWith("/tmp");
    expect(deleteFileMock).toHaveBeenCalledWith("/tmp/a.mp4");
  });

  it("clears terminal jobs and returns refreshed queue snapshot items", async () => {
    getQueueSnapshotMock.mockResolvedValue({
      items: [{ id: "1", title: "job" }],
    });

    const items = await clearCompletedQueueJobs();

    expect(clearTerminalJobsMock).toHaveBeenCalledTimes(1);
    expect(getQueueSnapshotMock).toHaveBeenCalledTimes(1);
    expect(items).toEqual([{ id: "1", title: "job" }]);
  });
});
