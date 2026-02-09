#!/usr/bin/env bun
/**
 * CLI entry point for fin
 *
 * Thin wrapper: initialises config, then delegates to the main command tree.
 * Import main.ts directly for side-effect-free access to the command tree
 * (used by tests and the tool registry).
 */

import { initConfig } from '@fin/core/config';
import { runMain } from 'citty';
import { main } from './main';

initConfig();

runMain(main);
