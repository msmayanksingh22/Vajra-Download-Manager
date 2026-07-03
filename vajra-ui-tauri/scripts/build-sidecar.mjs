import { execSync } from 'child_process';
import { existsSync, mkdirSync, copyFileSync } from 'fs';
import { join } from 'path';

// Get the target triple from Tauri
const target = process.env.TARGET;
if (!target) {
  console.log('No TARGET env var set by Tauri. Building for host.');
} else {
  console.log(`Building vajrad sidecar for target: ${target}`);
}

const buildCmd = target
  ? `cargo build --release --bin vajrad --target ${target}`
  : `cargo build --release --bin vajrad`;

try {
  // Build vajrad
  // We run this from vajra-ui-tauri context, so root is ..
  execSync(buildCmd, { stdio: 'inherit', cwd: join(process.cwd(), '..') });
} catch (err) {
  console.error('Failed to build vajrad sidecar:', err);
  process.exit(1);
}

// Find the compiled binary
const extension = target?.includes('windows') || process.platform === 'win32' ? '.exe' : '';
const targetDir = target ? `target/${target}/release` : `target/release`;
const sourceBin = join(process.cwd(), '..', targetDir, `vajrad${extension}`);

// Destination directory is src-tauri
const destDir = join(process.cwd(), 'src-tauri');
if (!existsSync(destDir)) {
  mkdirSync(destDir, { recursive: true });
}

// Tauri expects the binary to be named vajrad-<target><extension>
let finalTarget = target;
if (!finalTarget) {
  finalTarget = execSync('rustc -vV')
    .toString()
    .match(/host: (.+)/)[1]
    .trim();
}

const destBin = join(destDir, `vajrad-${finalTarget}${extension}`);
console.log(`Copying sidecar: ${sourceBin} -> ${destBin}`);
copyFileSync(sourceBin, destBin);
console.log('Sidecar build complete.');
