import sharp from 'sharp';
import fs from 'fs';
import path from 'path';

const logoPath = 'logo.png';
const extDir = 'vajra-extension/public';
const extDistDir = 'vajra-extension/dist';
const publicDir = 'vajra-ui-tauri/public';

async function generate() {
    if (!fs.existsSync(extDir)) fs.mkdirSync(extDir, { recursive: true });

    // Extension icons in public folder
    await sharp(logoPath).resize(32, 32).png().toFile(path.join(extDir, 'icon32.png'));
    await sharp(logoPath).resize(64, 64).png().toFile(path.join(extDir, 'icon64.png'));
    await sharp(logoPath).resize(128, 128).png().toFile(path.join(extDir, 'icon128.png'));
    fs.copyFileSync(logoPath, path.join(extDir, 'logo.png'));

    // If dist exists, copy to dist as well for instant refresh
    if (fs.existsSync(extDistDir)) {
        await sharp(logoPath).resize(32, 32).png().toFile(path.join(extDistDir, 'icon32.png'));
        await sharp(logoPath).resize(64, 64).png().toFile(path.join(extDistDir, 'icon64.png'));
        await sharp(logoPath).resize(128, 128).png().toFile(path.join(extDistDir, 'icon128.png'));
        fs.copyFileSync(logoPath, path.join(extDistDir, 'logo.png'));
    }

    console.log('Extension icons generated.');

    // Copy to tauri public
    fs.copyFileSync(logoPath, path.join(publicDir, 'logo.png'));
    console.log('Public logo generated.');
}

generate().catch(console.error);
