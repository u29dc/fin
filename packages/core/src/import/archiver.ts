import { mkdir, rename } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';

export async function archiveFiles(files: string[], archiveRootDir: string): Promise<string[]> {
	if (files.length === 0) {
		return [];
	}

	const now = new Date();
	const folderName = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;

	const targetDir = resolve(archiveRootDir, folderName);
	await mkdir(targetDir, { recursive: true });

	const archived: string[] = [];
	for (const filePath of files) {
		const targetPath = join(targetDir, basename(filePath));
		await rename(filePath, targetPath);
		archived.push(targetPath);
	}

	return archived;
}
