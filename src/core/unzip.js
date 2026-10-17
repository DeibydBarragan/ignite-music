'use strict';

const fsp = require('node:fs/promises');
const path = require('node:path');
const zlib = require('node:zlib');
const { promisify } = require('node:util');

const inflateRaw = promisify(zlib.inflateRaw);

const DEFAULT_MAX_ENTRIES = 10_000;
const DEFAULT_MAX_BYTES = 1024 * 1024 * 1024;

/**
 * Minimal async ZIP extractor (stored and deflate entries, Unix permissions and
 * symlinks) with zip-bomb limits. Used to install yt-dlp without depending on
 * unzip/tar/ditto being present on the system.
 */
async function extractZip(buffer, destination, { maxEntries = DEFAULT_MAX_ENTRIES, maxBytes = DEFAULT_MAX_BYTES } = {}) {
    const eocd = buffer.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
    if (eocd < 0) throw new Error('Not a ZIP file');
    const entries = buffer.readUInt16LE(eocd + 10);
    if (entries > maxEntries) throw new Error(`ZIP has too many entries (${entries} > ${maxEntries})`);
    let offset = buffer.readUInt32LE(eocd + 16);
    const root = path.resolve(destination);
    await fsp.mkdir(root, { recursive: true });
    const symlinks = [];
    let totalBytes = 0;

    for (let i = 0; i < entries; i++) {
        if (buffer.readUInt32LE(offset) !== 0x02014b50) throw new Error('Corrupt ZIP central directory');
        const method = buffer.readUInt16LE(offset + 10);
        const compressedSize = buffer.readUInt32LE(offset + 20);
        const nameLength = buffer.readUInt16LE(offset + 28);
        const extraLength = buffer.readUInt16LE(offset + 30);
        const commentLength = buffer.readUInt16LE(offset + 32);
        const externalAttributes = buffer.readUInt32LE(offset + 38);
        const localOffset = buffer.readUInt32LE(offset + 42);
        const name = buffer.toString('utf8', offset + 46, offset + 46 + nameLength);
        offset += 46 + nameLength + extraLength + commentLength;

        const target = path.resolve(root, name);
        if (target !== root && !target.startsWith(root + path.sep)) throw new Error(`Unsafe path in ZIP: ${name}`);

        const mode = externalAttributes >>> 16;
        const type = mode & 0o170000;
        if (name.endsWith('/') || type === 0o040000) {
            await fsp.mkdir(target, { recursive: true });
            continue;
        }

        if (buffer.readUInt32LE(localOffset) !== 0x04034b50) throw new Error(`Corrupt ZIP entry: ${name}`);
        const dataStart = localOffset + 30 + buffer.readUInt16LE(localOffset + 26) + buffer.readUInt16LE(localOffset + 28);
        const raw = buffer.subarray(dataStart, dataStart + compressedSize);
        let data;
        if (method === 0) data = raw;
        else if (method === 8) data = await inflateRaw(raw, { maxOutputLength: Math.max(1, maxBytes - totalBytes) });
        else throw new Error(`Unsupported ZIP compression method ${method} for ${name}`);
        totalBytes += data.length;
        if (totalBytes > maxBytes) throw new Error(`ZIP expands beyond ${maxBytes} bytes`);

        await fsp.mkdir(path.dirname(target), { recursive: true });
        if (type === 0o120000) {
            symlinks.push([data.toString('utf8'), target]);
            continue;
        }
        await fsp.writeFile(target, data);
        if (process.platform !== 'win32' && mode & 0o777) await fsp.chmod(target, mode & 0o777);
    }

    for (const [linkTarget, linkPath] of symlinks) {
        const resolved = path.resolve(path.dirname(linkPath), linkTarget);
        if (!resolved.startsWith(root + path.sep)) throw new Error(`Unsafe symlink in ZIP: ${linkPath}`);
        await fsp.rm(linkPath, { force: true });
        await fsp.symlink(linkTarget, linkPath);
    }
    return entries;
}

module.exports = { extractZip };
