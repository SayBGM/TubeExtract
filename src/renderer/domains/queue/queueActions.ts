import {
  cancelJobAndGetSnapshot,
  clearTerminalJobsAndGetSnapshot,
  deleteFileAndGetSnapshot,
  openFolder,
  pauseJobAndGetSnapshot,
  resumeJobAndGetSnapshot,
} from "../../lib/desktopClient";

export async function clearCompletedQueueJobs() {
  const snapshot = await clearTerminalJobsAndGetSnapshot();
  return snapshot.items;
}

export async function pauseQueueJob(id: string) {
  const snapshot = await pauseJobAndGetSnapshot(id);
  return snapshot.items;
}

export async function resumeQueueJob(id: string) {
  const snapshot = await resumeJobAndGetSnapshot(id);
  return snapshot.items;
}

export async function cancelQueueJob(id: string) {
  const snapshot = await cancelJobAndGetSnapshot(id);
  return snapshot.items;
}

export async function deleteQueueFile(path: string) {
  const snapshot = await deleteFileAndGetSnapshot(path);
  return snapshot.items;
}

export const queueActions = {
  pauseJob: pauseQueueJob,
  resumeJob: resumeQueueJob,
  cancelJob: cancelQueueJob,
  openFolder,
  deleteFile: deleteQueueFile,
  clearCompletedQueueJobs,
};
