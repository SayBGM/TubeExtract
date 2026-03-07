#!/usr/bin/env node
/**
 * Release script for TubeExtract.
 * Usage: npm run release -- [--patch|--minor|--major|--version X.Y.Z]
 *
 * Updates version in:
 *   - package.json
 *   - src-tauri/tauri.conf.json
 *   - src-tauri/Cargo.toml
 *
 * Then creates a git commit and tag, and pushes both to the remote.
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, '..');

function run(cmd, opts = {}) {
  return execSync(cmd, { cwd: ROOT, encoding: 'utf8', ...opts }).trim();
}

function bumpVersion(current, type) {
  const parts = current.split('.').map(Number);
  if (parts.length !== 3 || parts.some(Number.isNaN)) {
    throw new Error(`Cannot parse current version: "${current}"`);
  }
  const [major, minor, patch] = parts;
  switch (type) {
    case 'major':
      return `${major + 1}.0.0`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`Unknown bump type: ${type}`);
  }
}

function validateSemver(version) {
  return /^\d+\.\d+\.\d+$/.test(version);
}

function updateJsonFile(filePath, version) {
  const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  content.version = version;
  fs.writeFileSync(filePath, JSON.stringify(content, null, 2) + '\n');
  console.log(`  Updated ${path.relative(ROOT, filePath)}: ${version}`);
}

function updateCargoToml(filePath, version) {
  let content = fs.readFileSync(filePath, 'utf8');
  // Replace only the version line inside the [package] section.
  // The regex anchors to the start of a line (^) and stops at the first
  // version = "..." occurrence, which belongs to [package].
  content = content.replace(
    /^(version\s*=\s*)"[^"]*"/m,
    `$1"${version}"`
  );
  fs.writeFileSync(filePath, content);
  console.log(`  Updated ${path.relative(ROOT, filePath)}: ${version}`);
}

// ---------------------------------------------------------------------------
// Parse CLI arguments
// ---------------------------------------------------------------------------
const args = process.argv.slice(2);
let bumpType = null;
let explicitVersion = null;

for (let i = 0; i < args.length; i++) {
  if (args[i] === '--patch') bumpType = 'patch';
  else if (args[i] === '--minor') bumpType = 'minor';
  else if (args[i] === '--major') bumpType = 'major';
  else if (args[i] === '--version' && args[i + 1]) {
    explicitVersion = args[i + 1];
    i++;
  }
}

if (!bumpType && !explicitVersion) {
  console.error('Usage: npm run release -- [--patch|--minor|--major|--version X.Y.Z]');
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Guard: require clean git working tree
// ---------------------------------------------------------------------------
const gitStatus = run('git status --porcelain');
if (gitStatus) {
  console.error('Error: Git working tree is dirty. Commit or stash changes first.');
  console.error(gitStatus);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Resolve new version
// ---------------------------------------------------------------------------
const packageJson = JSON.parse(
  fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8')
);
const currentVersion = packageJson.version;

let newVersion;
if (explicitVersion) {
  if (!validateSemver(explicitVersion)) {
    console.error(`Error: Invalid version format: "${explicitVersion}". Use X.Y.Z`);
    process.exit(1);
  }
  newVersion = explicitVersion;
} else {
  newVersion = bumpVersion(currentVersion, bumpType);
}

console.log(`\nReleasing: ${currentVersion} -> ${newVersion}\n`);

// ---------------------------------------------------------------------------
// Update version files
// ---------------------------------------------------------------------------
console.log('Updating version files...');
updateJsonFile(path.join(ROOT, 'package.json'), newVersion);
updateJsonFile(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), newVersion);
updateCargoToml(path.join(ROOT, 'src-tauri', 'Cargo.toml'), newVersion);

// ---------------------------------------------------------------------------
// Git commit and tag
// ---------------------------------------------------------------------------
console.log('\nCreating git commit and tag...');
run('git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml');
run(`git commit -m "chore(release): v${newVersion}"`);
run(`git tag v${newVersion}`);

// ---------------------------------------------------------------------------
// Push
// ---------------------------------------------------------------------------
console.log('\nPushing to remote...');
run('git push');
run('git push --tags');

console.log(`\nReleased v${newVersion} -- CI will build and publish artifacts`);
console.log(
  `   https://github.com/SayBGM/TubeExtract/releases/tag/v${newVersion}`
);
