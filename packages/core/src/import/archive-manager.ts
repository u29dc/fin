import { mkdir, rename } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';

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
	async prepareArchive(files: string[], archiveRootDir: string): Promise<string[]> {
		if (files.length === 0) {
			return [];
		}

		const now = new Date();
		const folderName = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;

		this.targetDir = resolve(archiveRootDir, folderName);
		await mkdir(this.targetDir, { recursive: true });

		const targetPaths: string[] = [];
		for (const sourcePath of files) {
			const targetPath = join(this.targetDir, basename(sourcePath));
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
