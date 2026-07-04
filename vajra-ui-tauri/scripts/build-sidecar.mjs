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
  ? `cargo build --release --bin vajrad --bin vajra-cli --target ${target}`
  : `cargo build --release --bin vajrad --bin vajra-cli`;

try {
  // Build vajrad and vajra-cli
  execSync(target ? `cargo build --release --bin vajrad --bin vajra-cli --target ${target}` : `cargo build --release --bin vajrad --bin vajra-cli`, { stdio: 'inherit', cwd: join(process.cwd(), '..') });
} catch (err) {
  console.error('Failed to build sidecars:', err);
  process.exit(1);
}

// Find the compiled binary
const extension = target?.includes('windows') || process.platform === 'win32' ? '.exe' : '';
const targetDir = target ? `target/${target}/release` : `target/release`;

// Destination directory is src-tauri/bin
const destDir = join(process.cwd(), 'src-tauri', 'bin');
if (!existsSync(destDir)) {
  mkdirSync(destDir, { recursive: true });
}

// Tauri expects the binary to be named <name>-<target><extension>
let finalTarget = target;
if (!finalTarget) {
  finalTarget = execSync('rustc -vV')
    .toString()
    .match(/host: (.+)/)[1]
    .trim();
}

const binaries = ['vajrad', 'vajra-cli'];
for (const bin of binaries) {
  const sourceBin = join(process.cwd(), '..', targetDir, `${bin}${extension}`);
  const destBin = join(destDir, `${bin}-${finalTarget}${extension}`);
  console.log(`Copying sidecar: ${sourceBin} -> ${destBin}`);
  copyFileSync(sourceBin, destBin);
}
console.log('Sidecar build complete.');

