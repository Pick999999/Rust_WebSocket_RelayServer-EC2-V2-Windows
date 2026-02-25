const fs = require('fs');
const path = require('path');

const root = 'd:/Rust/RustAndSocket/Rust_WebSocket_RelayServer-EC2-V2-Windows/backup';

function mapCharToByte(char) {
    const code = char.charCodeAt(0);

    if (code < 256) return code;

    // Thai
    if (code >= 0x0E01 && code <= 0x0E5B) {
        return code - 0x0E00 + 0xA0;
    }

    // CP1252
    if (code === 0x20AC) return 0x80;
    if (code === 0x201A) return 0x82;
    if (code === 0x0192) return 0x83;
    if (code === 0x201E) return 0x84;
    if (code === 0x2026) return 0x85;
    if (code === 0x2020) return 0x86;
    if (code === 0x2021) return 0x87;
    if (code === 0x02C6) return 0x88;
    if (code === 0x2030) return 0x89;
    if (code === 0x0160) return 0x8A;
    if (code === 0x2039) return 0x8B;
    if (code === 0x0152) return 0x8C;
    if (code === 0x017D) return 0x8E;
    if (code === 0x2018) return 0x91;
    if (code === 0x2019) return 0x92;
    if (code === 0x201C) return 0x93;
    if (code === 0x201D) return 0x94;
    if (code === 0x2022) return 0x95;
    if (code === 0x2013) return 0x96;
    if (code === 0x2014) return 0x97;
    if (code === 0x02DC) return 0x98;
    if (code === 0x2122) return 0x99;
    if (code === 0x0161) return 0x9A;
    if (code === 0x203A) return 0x9B;
    if (code === 0x0153) return 0x9C;
    if (code === 0x017E) return 0x9E;
    if (code === 0x0178) return 0x9F;

    throw new Error(`Unknown char code: ${code}`);
}

try {
    const files = fs.readdirSync(root);
    files.forEach(file => {
        try {
            const bytes = [];
            let possibleBinary = false;
            for (let i = 0; i < file.length; i++) {
                const b = mapCharToByte(file[i]);
                bytes.push(b);
                if (b > 127) possibleBinary = true;
            }

            if (!possibleBinary) return;

            const buffer = Buffer.from(bytes);
            const decoded = buffer.toString('utf8');

            if (decoded !== file) {
                const oldPath = path.join(root, file);
                const newPath = path.join(root, decoded);
                if (fs.existsSync(newPath)) {
                    console.log(`Skipping rename of ${file} because ${decoded} already exists.`);
                } else {
                    fs.renameSync(oldPath, newPath);
                    console.log(`Renamed: ${file} -> ${decoded}`);
                }
            }
        } catch (e) {
            console.log(`Skipping ${file}: ${e.message}`);
        }
    });
} catch (e) {
    console.error(e);
}
