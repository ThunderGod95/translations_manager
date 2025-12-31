import { readdir, mkdir, copyFile } from "node:fs/promises";
import { join } from "node:path";

const TARGET_DIR =
    "C:\\Users\\tarun\\Translations\\TheMirrorLegacy\\translations";
const BACKUP_ROOT = "./.backups";

const NAV_START = "<!-- NAV START -->";
const NAV_END = "<!-- NAV END -->";
const TOC_LINK = "[TOC](./)";

function timestamp() {
    return new Date().toISOString().replace(/[:.]/g, "-");
}

async function backupFiles(files) {
    const backupDir = join(BACKUP_ROOT, timestamp());

    await mkdir(backupDir, { recursive: true });

    for (const file of files) {
        const src = join(TARGET_DIR, file);
        const dest = join(backupDir, file);
        await copyFile(src, dest);
    }

    console.log(`Backup created at: ${backupDir}`);
}

async function main() {
    const allFiles = await readdir(TARGET_DIR);

    const chapterFiles = allFiles
        .filter((file) => /^\d+\.md$/.test(file))
        .map((file) => ({
            name: file,
            num: Number(file.replace(".md", "")),
        }))
        .sort((a, b) => a.num - b.num);

    if (chapterFiles.length === 0) {
        console.log("No chapter files found. Exiting.");
        return;
    }

    try {
        await backupFiles(chapterFiles.map((c) => c.name));
    } catch (err) {
        console.error("Backup failed. Aborting without modifying files.");
        console.error(err);
        process.exit(1);
    }

    const existingChapters = new Set(chapterFiles.map((c) => c.num));
    console.log(`Found ${chapterFiles.length} chapter files.`);

    for (const { name, num } of chapterFiles) {
        const filePath = join(TARGET_DIR, name);

        let fileContent;
        try {
            fileContent = await Bun.file(filePath).text();
        } catch (err) {
            console.error(`Failed to read ${name}:`, err);
            continue;
        }

        const prevLink = existingChapters.has(num - 1)
            ? `[Previous Chapter](./${num - 1}.md)`
            : `Previous Chapter`;

        const nextLink = existingChapters.has(num + 1)
            ? `[Next Chapter](./${num + 1}.md)`
            : `Next Chapter`;

        const navBlock = [
            NAV_START,
            `${prevLink} | ${TOC_LINK} | ${nextLink}`,
            NAV_END,
        ].join("\n");

        const navRegex = new RegExp(
            `${NAV_START}[\\s\\S]*?${NAV_END}\\n*`,
            "g",
        );

        let updatedContent;

        const matches = fileContent.match(navRegex) ?? [];

        if (matches.length === 0) {
            updatedContent =
                `${navBlock}\n\n` +
                fileContent.replace(/^\n+/, "") +
                `\n\n${navBlock}\n`;
        } else {
            let replaced = fileContent.replace(navRegex, `${navBlock}\n\n`);
            const count = (replaced.match(new RegExp(NAV_START, "g")) ?? [])
                .length;

            if (count === 1) {
                replaced = replaced.replace(/\s*$/, `\n\n${navBlock}\n`);
            }

            updatedContent = replaced;
        }

        if (updatedContent !== fileContent) {
            await Bun.write(filePath, updatedContent);
            console.log(`Updated: ${name}`);
        } else {
            console.log(`Unchanged: ${name}`);
        }
    }

    console.log("All files processed!");
}

main().catch((err) => {
    console.error("Fatal error:", err);
    process.exit(1);
});
