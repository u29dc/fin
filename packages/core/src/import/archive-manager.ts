import { mkdir, rename } from 'node:fs/promises';
import { basename, extname, join, resolve } from 'node:path';

import type { ArchiveFile } from './types';

export type ArchiveOperation = {
	sourcePath: string;
	targetPath: string;
	completed: boolean;
};

/**
 * Manages file archiving with two-phase commit support.
 * Allows preparing archive targets, committing the moves, and rolling back on failure.
 */
export class ArchiveManager {
	private operations: ArchiveOperation[] = [];
	private targetDir: string | null = null;

	/**
	 * Phase 1: Prepare archive operations without moving files.
	 * Creates the target directory and records planned moves.
	 */
	async prepareArchive(files: ArchiveFile[], archiveRootDir: string): Promise<string[]> {
		if (files.length === 0) {
			return [];
		}

		const now = new Date();
		const folderName = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
		const timestamp = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, '0')}${String(now.getDate()).padStart(2, '0')}-${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}${String(now.getSeconds()).padStart(2, '0')}`;

		this.targetDir = resolve(archiveRootDir, folderName);
		await mkdir(this.targetDir, { recursive: true });

		const targetPaths: string[] = [];
		const usedNames = new Set<string>();

		for (const [index, file] of files.entries()) {
			const sourcePath = file.path;
			const ext = extname(sourcePath).toLowerCase();
			const base = basename(sourcePath, ext);
			const providerSlug = slugify(file.provider, 16) || 'provider';
			const accountSlug = slugify(file.chartAccountId.replaceAll(':', '-'), 40) || 'account';
			const nameSlug = slugify(base, 40) || 'file';
			const order = String(index + 1).padStart(2, '0');

			let targetName = `${timestamp}_${providerSlug}_${accountSlug}_${order}_${nameSlug}${ext}`;
			let counter = 2;
			while (usedNames.has(targetName)) {
				targetName = `${timestamp}_${providerSlug}_${accountSlug}_${order}_${nameSlug}-${counter}${ext}`;
				counter += 1;
			}
			usedNames.add(targetName);

			const targetPath = join(this.targetDir, targetName);
			this.operations.push({ sourcePath, targetPath, completed: false });
			targetPaths.push(targetPath);
		}

		return targetPaths;
	}

	/**
	 * Phase 2: Commit the archive by moving all files.
	 * Call this after DB operations succeed.
	 */
	async commitArchive(): Promise<string[]> {
		const archived: string[] = [];

		for (const op of this.operations) {
			await rename(op.sourcePath, op.targetPath);
			op.completed = true;
			archived.push(op.targetPath);
		}

		return archived;
	}

	/**
	 * Rollback: Move any completed files back to their original locations.
	 * Call this if DB operations fail after archive started.
	 */
	async rollbackArchive(): Promise<void> {
		for (const op of this.operations) {
			if (op.completed) {
				try {
					await rename(op.targetPath, op.sourcePath);
					op.completed = false;
				} catch {
					// Best effort rollback - continue even if a file can't be moved back
				}
			}
		}
	}

	/**
	 * Check if any files have been archived.
	 */
	hasArchivedFiles(): boolean {
		return this.operations.some((op) => op.completed);
	}

	/**
	 * Reset the manager for reuse.
	 */
	reset(): void {
		this.operations = [];
		this.targetDir = null;
	}
}

function slugify(value: string, maxLen: number): string {
	const normalized = value
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '')
		.replace(/-{2,}/g, '-');
	if (normalized.length === 0) {
		return '';
	}
	return normalized.slice(0, maxLen);
}
