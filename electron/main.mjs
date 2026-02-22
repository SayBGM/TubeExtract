import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from "electron";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { pipeline } from "node:stream/promises";
import https from "node:https";
import checkDiskSpace from "check-disk-space";
import ffmpegStatic from "ffmpeg-static";

const sleep = promisify(setTimeout);

const QUEUE_FILE = "queue_state.json";
const SETTINGS_FILE = "settings.json";
const UPDATE_REPO = "";
const MANAGED_BIN_DIR = "bin";
const TEMP_DOWNLOADS_DIR = "tmp-downloads";
const COMMON_BINARY_DIRS = [
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/usr/bin",
  "/bin",
  "C:\\Program Files\\yt-dlp",
  "C:\\Program Files\\ffmpeg\\bin",
];
const MAX_LOG_LINES_PER_JOB = 120;
const ANALYZE_TIMEOUT_MS = 15_000;
const RETRY_DELAY_TABLE_MS = [2000, 5000, 10000, 15000];
const YTDLP_DOWNLOAD_URL_WINDOWS = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_DOWNLOAD_URL_MACOS = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";
const MAX_BASE_FILENAME_LENGTH = 160;

let mainWindow = null;
let dependencyInstallPromise = null;
const state = {
  queue: {
    items: [],
    activeJobId: null,
  },
  settings: {
    downloadDir: "",
    maxRetries: 3,
    language: "ko",
  },
  activeChild: null,
};

function appDataDir() {
  const dir = app.getPath("userData");
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function queueFilePath() {
  return path.join(appDataDir(), QUEUE_FILE);
}

function managedBinDirPath() {
  return path.join(appDataDir(), MANAGED_BIN_DIR);
}

function tempDownloadsRootDir() {
  return path.join(appDataDir(), TEMP_DOWNLOADS_DIR);
}

function tempJobDirPath(jobId) {
  return path.join(tempDownloadsRootDir(), String(jobId));
}

function managedExecutablePath(binaryName) {
  const executableName = process.platform === "win32" ? `${binaryName}.exe` : binaryName;
  return path.join(managedBinDirPath(), executableName);
}

function settingsFilePath() {
  return path.join(appDataDir(), SETTINGS_FILE);
}

function queueSnapshot() {
  return { items: [...state.queue.items] };
}

function emitQueueUpdated() {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  mainWindow.webContents.send("queue-updated", queueSnapshot());
}

function persistQueue() {
  fs.writeFileSync(queueFilePath(), JSON.stringify(state.queue.items, null, 2), "utf-8");
}

function persistSettings() {
  fs.writeFileSync(settingsFilePath(), JSON.stringify(state.settings, null, 2), "utf-8");
}

function loadSettings() {
  const file = settingsFilePath();
  if (!fs.existsSync(file)) return;
  const parsed = JSON.parse(fs.readFileSync(file, "utf-8"));
  state.settings = { ...state.settings, ...parsed };
}

function loadQueue() {
  const file = queueFilePath();
  if (!fs.existsSync(file)) return;
  const parsed = JSON.parse(fs.readFileSync(file, "utf-8"));
  state.queue.items = (Array.isArray(parsed) ? parsed : []).map((item) => ({
    ...item,
    status: item.status === "downloading" ? "queued" : item.status,
    downloadLog: Array.isArray(item.downloadLog) ? item.downloadLog : [],
    thumbnailUrl: typeof item.thumbnailUrl === "string" ? item.thumbnailUrl : undefined,
  }));
}

function getDefaultSettings() {
  return {
    downloadDir: app.getPath("downloads"),
    maxRetries: 3,
    language: "ko",
  };
}

function resolveExecutable(binaryName) {
  const managedPath = managedExecutablePath(binaryName);
  if (fs.existsSync(managedPath)) {
    return managedPath;
  }

  const isWindows = process.platform === "win32";
  const withExt = isWindows ? `${binaryName}.exe` : binaryName;
  const pathVar = process.env.PATH ?? "";
  for (const entry of pathVar.split(path.delimiter)) {
    const candidate = path.join(entry, withExt);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  for (const baseDir of COMMON_BINARY_DIRS) {
    const candidate = path.join(baseDir, withExt);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return withExt;
}

function downloadFile(url, destinationPath) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      if (response.statusCode && response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        downloadFile(response.headers.location, destinationPath).then(resolve).catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`파일 다운로드 실패: ${response.statusCode ?? "unknown"}`));
        return;
      }

      const fileStream = fs.createWriteStream(destinationPath);
      pipeline(response, fileStream).then(resolve).catch(reject);
    });

    request.on("error", reject);
  });
}

async function ensureYtDlp() {
  const target = managedExecutablePath("yt-dlp");
  if (fs.existsSync(target)) return;

  const downloadUrl = process.platform === "win32" ? YTDLP_DOWNLOAD_URL_WINDOWS : YTDLP_DOWNLOAD_URL_MACOS;
  await downloadFile(downloadUrl, target);
  if (process.platform !== "win32") {
    fs.chmodSync(target, 0o755);
  }
}

async function ensureFfmpeg() {
  const target = managedExecutablePath("ffmpeg");
  if (fs.existsSync(target)) return;

  if (!ffmpegStatic) {
    throw new Error("ffmpeg-static 바이너리를 찾을 수 없습니다.");
  }

  fs.copyFileSync(ffmpegStatic, target);
  if (process.platform !== "win32") {
    fs.chmodSync(target, 0o755);
  }
}

async function ensureDependencies() {
  if (dependencyInstallPromise) {
    return dependencyInstallPromise;
  }

  dependencyInstallPromise = (async () => {
    fs.mkdirSync(managedBinDirPath(), { recursive: true });
    await ensureYtDlp();
    await ensureFfmpeg();
  })();

  try {
    await dependencyInstallPromise;
  } finally {
    dependencyInstallPromise = null;
  }
}

function normalizeYouTubeVideoUrl(rawUrl) {
  const input = String(rawUrl ?? "").trim();
  if (!input) return input;

  try {
    const parsed = new URL(input);
    const host = parsed.hostname.toLowerCase();

    if (host.includes("youtube.com")) {
      const videoId = parsed.searchParams.get("v");
      if (videoId) {
        return `https://www.youtube.com/watch?v=${videoId}`;
      }
      const pathParts = parsed.pathname.split("/").filter(Boolean);
      if (pathParts.length >= 2 && (pathParts[0] === "shorts" || pathParts[0] === "live")) {
        return `https://www.youtube.com/watch?v=${pathParts[1]}`;
      }
    }

    if (host === "youtu.be") {
      const videoId = parsed.pathname.split("/").filter(Boolean)[0];
      if (videoId) {
        return `https://www.youtube.com/watch?v=${videoId}`;
      }
    }
  } catch {
    return input;
  }

  return input;
}

function isCurrentlyLiveStream(parsedMetadata) {
  if (!parsedMetadata || typeof parsedMetadata !== "object") return false;
  if (parsedMetadata.is_live === true) return true;
  const liveStatus = String(parsedMetadata.live_status ?? "").toLowerCase();
  return liveStatus === "is_live";
}

function expectedOutputExtension(mode) {
  return mode === "audio" ? "mp3" : "mp4";
}

function sanitizeFileName(rawName) {
  const normalized = String(rawName ?? "download")
    .replace(/[\\/:*?"<>|]/g, "_")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[. ]+$/g, "");

  if (!normalized) return "download";
  return normalized.slice(0, MAX_BASE_FILENAME_LENGTH);
}

function createUniqueOutputPath(downloadDir, rawTitle, mode) {
  const baseName = sanitizeFileName(rawTitle);
  const extension = expectedOutputExtension(mode);
  const hasConflictingQueuedPath = (candidatePath) =>
    state.queue.items.some((item) => item.outputPath === candidatePath && item.status !== "failed" && item.status !== "canceled");

  let suffix = 0;
  while (true) {
    const suffixLabel = suffix === 0 ? "" : ` (${suffix})`;
    const candidateFileName = `${baseName}${suffixLabel}.${extension}`;
    const candidatePath = path.join(downloadDir, candidateFileName);
    const existsOnDisk = fs.existsSync(candidatePath);
    if (!existsOnDisk && !hasConflictingQueuedPath(candidatePath)) {
      return candidatePath;
    }
    suffix += 1;
  }
}

async function removeDirectorySafe(targetDir) {
  if (!targetDir) return;
  await fs.promises.rm(targetDir, { recursive: true, force: true });
}

async function moveFileWithFallback(sourcePath, destinationPath) {
  try {
    await fs.promises.rename(sourcePath, destinationPath);
  } catch (error) {
    const isCrossDevice = error && typeof error === "object" && "code" in error && error.code === "EXDEV";
    if (!isCrossDevice) {
      throw error;
    }
    await fs.promises.copyFile(sourcePath, destinationPath);
    await fs.promises.unlink(sourcePath);
  }
}

async function resolveDownloadedFilePath(tempDir, expectedExt) {
  const prioritizedFile = path.join(tempDir, `media.${expectedExt}`);
  if (fs.existsSync(prioritizedFile)) {
    return prioritizedFile;
  }

  const entries = await fs.promises.readdir(tempDir, { withFileTypes: true });
  const candidates = [];
  for (const entry of entries) {
    if (!entry.isFile()) continue;
    if (!entry.name.toLowerCase().endsWith(`.${expectedExt.toLowerCase()}`)) continue;
    const candidatePath = path.join(tempDir, entry.name);
    const stat = await fs.promises.stat(candidatePath);
    candidates.push({ path: candidatePath, mtimeMs: stat.mtimeMs });
  }

  if (candidates.length === 0) {
    throw new Error("완성 파일을 임시 폴더에서 찾지 못했습니다.");
  }

  candidates.sort((a, b) => b.mtimeMs - a.mtimeMs);
  return candidates[0].path;
}

async function cleanupTemporaryDownloads() {
  await removeDirectorySafe(tempDownloadsRootDir());
}

function runCommandCapture(command, args, timeoutMs) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let timeoutId;

    if (typeof timeoutMs === "number" && timeoutMs > 0) {
      timeoutId = setTimeout(() => {
        timedOut = true;
        child.kill("SIGKILL");
      }, timeoutMs);
    }

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (timeoutId) clearTimeout(timeoutId);
      resolve({ code: code ?? 1, stdout, stderr, timedOut });
    });
  });
}

function videoExtensionPriority(ext) {
  if (ext === "mp4") return 2;
  if (ext === "webm") return 1;
  return 0;
}

function selectFormatExpression(job) {
  if (job.mode === "audio") return job.qualityId;
  if (job.qualityId.includes("+")) return job.qualityId;
  return `${job.qualityId}+bestaudio/best`;
}

function parseProgressPercent(line) {
  const idx = line.indexOf("%");
  if (idx < 0) return undefined;
  const start = line.slice(0, idx).search(/[0-9.]+$/);
  if (start < 0) return undefined;
  const value = Number.parseFloat(line.slice(start, idx).trim());
  return Number.isFinite(value) ? value : undefined;
}

function parseSpeed(line) {
  const at = line.indexOf(" at ");
  const eta = line.indexOf(" ETA");
  if (at < 0 || eta < 0) return undefined;
  return line.slice(at + 4, eta).trim();
}

function parseEta(line) {
  const eta = line.indexOf(" ETA ");
  if (eta < 0) return undefined;
  return line.slice(eta + 5).trim();
}

function readLines(stream, onLine) {
  let buffer = "";
  stream.on("data", (chunk) => {
    buffer += chunk.toString();
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() ?? "";
    for (const line of lines) onLine(line);
  });
  stream.on("end", () => {
    if (buffer.trim()) onLine(buffer.trim());
  });
}

function handleDownloadOutputLine(jobId, line, lastError) {
  const normalizedLine = line.trim();
  if (!normalizedLine) return;

  const item = state.queue.items.find((job) => job.id === jobId);
  if (!item) return;

  const isDuplicateLine = item.downloadLog.at(-1) === normalizedLine;
  if (!isDuplicateLine) {
    item.downloadLog.push(normalizedLine);
    if (item.downloadLog.length > MAX_LOG_LINES_PER_JOB) {
      item.downloadLog.splice(0, item.downloadLog.length - MAX_LOG_LINES_PER_JOB);
    }
  }

  if (normalizedLine.includes("ERROR:") || normalizedLine.includes("HTTP Error")) {
    lastError.value = normalizedLine;
  }

  if (normalizedLine.includes("Destination:")) {
    const [, destination] = normalizedLine.split("Destination:");
    const destinationPath = destination?.trim();
    if (destinationPath && !destinationPath.startsWith(tempDownloadsRootDir())) {
      item.outputPath = destinationPath;
    }
  }

  const progress = parseProgressPercent(normalizedLine);
  if (progress !== undefined) {
    item.progressPercent = progress;
    item.status = "downloading";
  }
  item.speedText = parseSpeed(normalizedLine) ?? item.speedText;
  item.etaText = parseEta(normalizedLine) ?? item.etaText;

  emitQueueUpdated();
}

async function runDownloadOnce(job) {
  await ensureDependencies();
  fs.mkdirSync(state.settings.downloadDir, { recursive: true });
  const finalOutputPath = createUniqueOutputPath(state.settings.downloadDir, job.title, job.mode);
  const tempDir = tempJobDirPath(job.id);
  await fs.promises.mkdir(tempDir, { recursive: true });
  const outputTemplate = path.join(tempDir, "media.%(ext)s");
  const expectedExt = expectedOutputExtension(job.mode);
  const ytDlp = resolveExecutable("yt-dlp");
  const args = [
    "--no-playlist",
    "--newline",
    "--progress",
    "-f",
    selectFormatExpression(job),
    "-o",
    outputTemplate,
    job.url,
  ];
  if (job.mode === "audio") {
    args.push("-x", "--audio-format", "mp3");
  } else {
    args.push("--merge-output-format", "mp4", "--recode-video", "mp4");
  }

  const child = spawn(ytDlp, args, { stdio: ["ignore", "pipe", "pipe"] });
  state.activeChild = child;
  job.outputPath = finalOutputPath;
  job.tempDir = tempDir;
  const lastError = { value: undefined };

  if (child.stdout) {
    readLines(child.stdout, (line) => handleDownloadOutputLine(job.id, line, lastError));
  }
  if (child.stderr) {
    readLines(child.stderr, (line) => handleDownloadOutputLine(job.id, line, lastError));
  }

  const statusOk = await new Promise((resolve) => {
    child.on("close", (code) => resolve(code === 0));
    child.on("error", () => resolve(false));
  });

  let finalStatusOk = statusOk;
  const item = state.queue.items.find((queuedJob) => queuedJob.id === job.id);
  if (item && item.status !== "paused" && item.status !== "canceled") {
    if (statusOk) {
      try {
        const completedPath = await resolveDownloadedFilePath(tempDir, expectedExt);
        await moveFileWithFallback(completedPath, finalOutputPath);
        item.status = "completed";
        item.progressPercent = 100;
        item.errorMessage = undefined;
      } catch (error) {
        item.status = "failed";
        item.errorMessage =
          error instanceof Error ? error.message : "완성 파일 이동에 실패했습니다.";
        finalStatusOk = false;
      }
    } else {
      item.status = "failed";
      item.errorMessage = lastError.value ?? "다운로드 프로세스 종료 실패";
    }
  }

  state.activeChild = null;
  await removeDirectorySafe(tempDir);
  if (item && "tempDir" in item) {
    delete item.tempDir;
  }
  emitQueueUpdated();
  return finalStatusOk;
}

function retryDelayMs(attempt) {
  const idx = Math.min(attempt, RETRY_DELAY_TABLE_MS.length - 1);
  return RETRY_DELAY_TABLE_MS[idx];
}

async function runDownloadLoop(jobId) {
  const maxRetries = state.settings.maxRetries;
  for (let attempt = 0; attempt <= maxRetries; attempt += 1) {
    const job = state.queue.items.find((item) => item.id === jobId);
    if (!job) break;
    if (job.status === "canceled" || job.status === "paused") break;

    const ok = await runDownloadOnce(job);
    if (ok) break;

    const latestJob = state.queue.items.find((item) => item.id === jobId);
    if (!latestJob || latestJob.status === "canceled" || latestJob.status === "paused") {
      break;
    }

    if (attempt < maxRetries) {
      latestJob.retryCount = attempt + 1;
      latestJob.status = "queued";
      emitQueueUpdated();
      await sleep(retryDelayMs(attempt + 1));
      continue;
    }

    if (latestJob.status !== "canceled" && latestJob.status !== "paused") {
      latestJob.status = "failed";
      latestJob.errorMessage = latestJob.errorMessage ?? "다운로드 실패";
    }
  }

  state.queue.activeJobId = null;
  persistQueue();
  emitQueueUpdated();
  tryStartNextJob();
}

function tryStartNextJob() {
  if (state.queue.activeJobId) return;
  const next = state.queue.items.find((item) => item.status === "queued");
  if (!next) return;

  state.queue.activeJobId = next.id;
  next.status = "downloading";
  emitQueueUpdated();
  persistQueue();
  void runDownloadLoop(next.id);
}

async function analyzeUrl(url) {
  await ensureDependencies();
  const normalizedUrl = normalizeYouTubeVideoUrl(url);
  if (!normalizedUrl?.trim()) throw new Error("URL is empty");
  const ytDlp = resolveExecutable("yt-dlp");
  const output = await runCommandCapture(
    ytDlp,
    ["--no-playlist", "-J", "--no-warnings", normalizedUrl.trim()],
    ANALYZE_TIMEOUT_MS,
  );
  if (output.timedOut) {
    throw new Error("분석 시간이 15초를 초과했습니다. 네트워크 상태를 확인하고 다시 시도해 주세요.");
  }
  if (output.code !== 0) {
    throw new Error(output.stderr.trim() || "URL 분석에 실패했습니다.");
  }

  const parsed = JSON.parse(output.stdout);
  if (isCurrentlyLiveStream(parsed)) {
    throw new Error("현재 라이브 스트리밍 중인 영상은 다운로드할 수 없습니다.");
  }
  const formats = Array.isArray(parsed.formats) ? parsed.formats : [];
  const videoCandidates = [];
  const audioCandidates = [];

  for (const format of formats) {
    const formatId = String(format.format_id ?? "");
    const ext = String(format.ext ?? "unknown");
    const height = Number(format.height ?? 0);
    const fps = Number(format.fps ?? 0);
    const tbr = Number(format.tbr ?? 0);
    const abr = Number(format.abr ?? 0);
    const vcodec = String(format.vcodec ?? "none");
    const acodec = String(format.acodec ?? "none");

    if (vcodec !== "none" && height > 0) {
      const qualityRank = height * 1_000_000 + fps * 1_000 + tbr;
      videoCandidates.push({
        extPriority: videoExtensionPriority(ext),
        qualityRank,
        height,
        option: { id: formatId, label: `${height}p`, ext, type: "video" },
      });
    }
    if (acodec !== "none" && vcodec === "none") {
      const abrValue = Math.max(0, Math.floor(abr));
      const qualityRank = abrValue * 1_000 + tbr;
      const label = `${abrValue}kbps`;
      audioCandidates.push({
        qualityRank,
        option: { id: formatId, label, ext, type: "audio" },
      });
    }
  }

  videoCandidates.sort((a, b) => b.extPriority - a.extPriority || b.qualityRank - a.qualityRank);
  audioCandidates.sort((a, b) => b.qualityRank - a.qualityRank);

  // Keep only one option per resolution so users can focus on quality.
  const seenHeights = new Set();
  const videoOptions = [];
  for (const candidate of videoCandidates) {
    if (seenHeights.has(candidate.height)) continue;
    seenHeights.add(candidate.height);
    videoOptions.push(candidate.option);
  }
  const audioOptions = audioCandidates.map((entry) => entry.option);
  if (videoOptions.length === 0) {
    videoOptions.push({
      id: "bestvideo+bestaudio",
      label: "Best Video",
      ext: "mp4",
      type: "video",
    });
  }
  if (audioOptions.length === 0) {
    audioOptions.push({
      id: "bestaudio",
      label: "Best Audio",
      ext: "m4a",
      type: "audio",
    });
  }

  return {
    sourceUrl: normalizedUrl,
    title: String(parsed.title ?? "Unknown Title"),
    channel: String(parsed.uploader ?? "Unknown Channel"),
    durationSec: Number(parsed.duration ?? 0),
    thumbnailUrl: String(parsed.thumbnail ?? ""),
    videoOptions,
    audioOptions,
  };
}

function checkDuplicate(input) {
  const normalizedUrl = normalizeYouTubeVideoUrl(input.url);
  const duplicate = state.queue.items.find(
    (item) =>
      item.url === normalizedUrl &&
      item.mode === input.mode &&
      item.qualityId === input.qualityId &&
      item.status !== "failed" &&
      item.status !== "canceled",
  );
  return {
    isDuplicate: Boolean(duplicate),
    existingOutputPath: duplicate?.outputPath,
  };
}

function enqueueJob(input) {
  const normalizedUrl = normalizeYouTubeVideoUrl(input.url);
  if (!input.forceDuplicate) {
    const duplicate = checkDuplicate({ ...input, url: normalizedUrl });
    if (duplicate.isDuplicate) {
      throw new Error("중복 다운로드가 감지되었습니다.");
    }
  }
  const id = crypto.randomUUID();
  state.queue.items.push({
    id,
    title: input.title ?? input.url,
    thumbnailUrl: input.thumbnailUrl,
    url: normalizedUrl,
    mode: input.mode,
    qualityId: input.qualityId,
    status: "queued",
    progressPercent: 0,
    speedText: undefined,
    etaText: undefined,
    outputPath: undefined,
    errorMessage: undefined,
    retryCount: 0,
    downloadLog: [],
  });
  persistQueue();
  emitQueueUpdated();
  tryStartNextJob();
  return { jobId: id };
}

function pauseJob(id) {
  const item = state.queue.items.find((job) => job.id === id);
  if (!item) return;
  item.status = "paused";
  if (state.activeChild) {
    state.activeChild.kill();
  }
  persistQueue();
  emitQueueUpdated();
}

function resumeJob(id) {
  const item = state.queue.items.find((job) => job.id === id);
  if (!item) return;
  item.status = "queued";
  item.errorMessage = undefined;
  persistQueue();
  emitQueueUpdated();
  tryStartNextJob();
}

function cancelJob(id) {
  const item = state.queue.items.find((job) => job.id === id);
  if (!item) return;
  item.status = "canceled";
  item.errorMessage = "사용자 취소";
  if (state.activeChild && state.queue.activeJobId === id) {
    state.activeChild.kill();
  }
  persistQueue();
  emitQueueUpdated();
}

function clearTerminalJobs() {
  state.queue.items = state.queue.items.filter((item) => item.status !== "completed");
  persistQueue();
  emitQueueUpdated();
}

function getQueueSnapshot() {
  return queueSnapshot();
}

function getSettings() {
  return { ...state.settings };
}

async function pickDownloadDir() {
  const result = await dialog.showOpenDialog(mainWindow, {
    title: "다운로드 폴더 선택",
    properties: ["openDirectory", "createDirectory"],
    defaultPath: state.settings.downloadDir || app.getPath("downloads"),
  });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return result.filePaths[0];
}

function setSettings(nextSettings) {
  state.settings = { ...state.settings, ...nextSettings };
  persistSettings();
}

async function runDiagnostics() {
  try {
    await ensureDependencies();
  } catch {
    // Diagnostics should still run even if installation failed.
  }
  const ytDlp = resolveExecutable("yt-dlp");
  const ffmpeg = resolveExecutable("ffmpeg");
  const ytDlpCheck = await runCommandCapture(ytDlp, ["--version"]).catch(() => ({ code: 1 }));
  const ffmpegCheck = await runCommandCapture(ffmpeg, ["-version"]).catch(() => ({ code: 1 }));
  let writable = false;
  try {
    fs.mkdirSync(state.settings.downloadDir, { recursive: true });
    const testFile = path.join(state.settings.downloadDir, "tubeextract_write_test.tmp");
    fs.writeFileSync(testFile, "ok", "utf-8");
    fs.unlinkSync(testFile);
    writable = true;
  } catch {
    writable = false;
  }
  return {
    ytDlpAvailable: ytDlpCheck.code === 0,
    ffmpegAvailable: ffmpegCheck.code === 0,
    downloadPathWritable: writable,
    message: `yt-dlp: ${ytDlpCheck.code === 0 ? "OK" : "FAIL"} (${ytDlp}), ffmpeg: ${ffmpegCheck.code === 0 ? "OK" : "FAIL"} (${ffmpeg}), download-dir writable: ${writable ? "OK" : "FAIL"}`,
  };
}

function calculateDirectorySize(dirPath) {
  try {
    const entries = fs.readdirSync(dirPath, { withFileTypes: true });
    let total = 0;
    for (const entry of entries) {
      const fullPath = path.join(dirPath, entry.name);
      if (entry.isDirectory()) total += calculateDirectorySize(fullPath);
      if (entry.isFile()) total += fs.statSync(fullPath).size;
    }
    return total;
  } catch {
    return 0;
  }
}

async function getStorageStats() {
  const disk = await checkDiskSpace(state.settings.downloadDir);
  const totalBytes = disk.size;
  const availableBytes = disk.free;
  const usedBytes = Math.max(0, totalBytes - availableBytes);
  const usedPercent = totalBytes === 0 ? 0 : (usedBytes / totalBytes) * 100;
  const downloadDirBytes = calculateDirectorySize(state.settings.downloadDir);
  return {
    totalBytes,
    availableBytes,
    usedBytes,
    usedPercent,
    downloadDirBytes,
  };
}

async function checkUpdate() {
  if (!UPDATE_REPO) {
    return { hasUpdate: false, latestVersion: undefined, url: undefined };
  }
  return { hasUpdate: false, latestVersion: undefined, url: undefined };
}

async function deleteFile(filePath) {
  if (!filePath) return;

  state.queue.items = state.queue.items.filter((item) => item.outputPath !== filePath);
  persistQueue();
  emitQueueUpdated();
}

async function openFolder(targetPath) {
  if (!targetPath?.trim()) return;

  const normalizedPath = path.resolve(targetPath);
  const openParentDirectory = async () => {
    const parentDirectory = path.dirname(normalizedPath);
    const openResult = await shell.openPath(parentDirectory);
    if (openResult) {
      throw new Error(openResult);
    }
  };

  try {
    const stats = await fs.promises.stat(normalizedPath);
    // Reveal behavior: keep the target selected in Finder/Explorer when possible.
    if (stats.isDirectory() || stats.isFile()) {
      shell.showItemInFolder(normalizedPath);
      return;
    }

    const openResult = await shell.openPath(normalizedPath);
    if (openResult) {
      throw new Error(openResult);
    }
  } catch (error) {
    const isNotFound = error && typeof error === "object" && "code" in error && error.code === "ENOENT";
    if (isNotFound) {
      await openParentDirectory();
      return;
    }
    throw error;
  }
}

async function openExternalUrl(rawUrl) {
  const input = String(rawUrl ?? "").trim();
  if (!input) return;

  let parsed;
  try {
    parsed = new URL(input);
  } catch {
    throw new Error("유효한 URL이 아닙니다.");
  }

  const protocol = parsed.protocol.toLowerCase();
  if (protocol !== "http:" && protocol !== "https:") {
    throw new Error("http/https URL만 열 수 있습니다.");
  }

  await shell.openExternal(parsed.toString());
}

function registerIpcHandlers() {
  ipcMain.handle("analyze_url", async (_event, args) => analyzeUrl(args.url));
  ipcMain.handle("check_duplicate", (_event, args) => checkDuplicate(args.input));
  ipcMain.handle("enqueue_job", (_event, args) => enqueueJob(args.input));
  ipcMain.handle("pause_job", (_event, args) => pauseJob(args.id));
  ipcMain.handle("resume_job", (_event, args) => resumeJob(args.id));
  ipcMain.handle("cancel_job", (_event, args) => cancelJob(args.id));
  ipcMain.handle("clear_terminal_jobs", () => clearTerminalJobs());
  ipcMain.handle("get_queue_snapshot", () => getQueueSnapshot());
  ipcMain.handle("get_settings", () => getSettings());
  ipcMain.handle("pick_download_dir", () => pickDownloadDir());
  ipcMain.handle("set_settings", (_event, args) => setSettings(args.settings));
  ipcMain.handle("run_diagnostics", () => runDiagnostics());
  ipcMain.handle("check_update", () => checkUpdate());
  ipcMain.handle("get_storage_stats", () => getStorageStats());
  ipcMain.handle("delete_file", async (_event, args) => deleteFile(args.path));
  ipcMain.handle("open_folder", async (_event, args) => openFolder(args.path));
  ipcMain.handle("open_external_url", async (_event, args) => openExternalUrl(args.url));
}

function createWindow() {
  const __filename = fileURLToPath(import.meta.url);
  const __dirname = path.dirname(__filename);

  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1100,
    minHeight: 700,
    backgroundColor: "#09090b",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      sandbox: false,
      nodeIntegration: false,
    },
  });

  const devUrl = process.env.ELECTRON_RENDERER_URL ?? "http://localhost:1420";
  const isDevMode = !app.isPackaged || process.env.ELECTRON_DEV === "true";
  if (isDevMode) {
    void mainWindow.loadURL(devUrl);
  } else {
    void mainWindow.loadFile(path.join(__dirname, "..", "dist", "index.html"));
  }

  // Enable DevTools toggle shortcuts in both development and packaged builds.
  mainWindow.webContents.on("before-input-event", (event, input) => {
    const isF12 = input.key === "F12" && input.type === "keyDown";
    const isDevToolsShortcut =
      input.type === "keyDown" &&
      input.key.toLowerCase() === "i" &&
      input.shift &&
      input.control;
    const isMacDevToolsShortcut =
      input.type === "keyDown" &&
      input.key.toLowerCase() === "i" &&
      input.shift &&
      input.meta;

    if (isF12 || isDevToolsShortcut || isMacDevToolsShortcut) {
      event.preventDefault();
      if (mainWindow.webContents.isDevToolsOpened()) {
        mainWindow.webContents.closeDevTools();
      } else {
        mainWindow.webContents.openDevTools({ mode: "detach" });
      }
    }
  });

  // Enable right-click context menu including DevTools access.
  mainWindow.webContents.on("context-menu", (_event, params) => {
    const menu = Menu.buildFromTemplate([
      { label: "뒤로", enabled: mainWindow.webContents.canGoBack(), click: () => mainWindow.webContents.goBack() },
      { label: "앞으로", enabled: mainWindow.webContents.canGoForward(), click: () => mainWindow.webContents.goForward() },
      { type: "separator" },
      { role: "reload", label: "새로고침" },
      { role: "forceReload", label: "강력 새로고침" },
      { type: "separator" },
      { role: "copy", label: "복사" },
      { role: "paste", label: "붙여넣기" },
      { role: "selectAll", label: "전체 선택" },
      { type: "separator" },
      {
        label: "개발자 도구 토글",
        click: () => {
          if (mainWindow.webContents.isDevToolsOpened()) {
            mainWindow.webContents.closeDevTools();
          } else {
            mainWindow.webContents.openDevTools({ mode: "detach" });
          }
        },
      },
      {
        label: "요소 검사",
        click: () => {
          if (!mainWindow.webContents.isDevToolsOpened()) {
            mainWindow.webContents.openDevTools({ mode: "detach" });
          }
          mainWindow.webContents.inspectElement(params.x, params.y);
        },
      },
    ]);
    menu.popup({ window: mainWindow });
  });
}

app.whenReady().then(() => {
  state.settings = getDefaultSettings();
  loadSettings();
  loadQueue();
  void cleanupTemporaryDownloads().catch(console.error);
  registerIpcHandlers();
  createWindow();
  void ensureDependencies().catch(console.error);
  emitQueueUpdated();
  tryStartNextJob();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
