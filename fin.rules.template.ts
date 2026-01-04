/**
 * Transaction Sanitization Rules
 *
 * Copy this file to data/fin.rules.ts and customize with your own
 * merchant names, employers, contacts, and local businesses.
 *
 * These rules are merged with the generic rules in packages/core/src/sanitize/rules.ts
 * Your rules take precedence over generic rules.
 *
 * Patterns are matched case-insensitively by default using 'contains' mode.
 * Rule order matters - more specific patterns should come before general ones.
 */

import type { NameMappingConfig } from './packages/core/src/sanitize/types';

export const NAME_MAPPING_CONFIG: NameMappingConfig = {
	rules: [
		// =============================================================================
		// INCOME
		// =============================================================================
		{ patterns: ['YOUR EMPLOYER NAME'], target: 'Salary', category: 'Income' },
		{ patterns: ['DIVIDEND PAYMENT'], target: 'Dividends', category: 'Income' },

		// =============================================================================
		// GROCERIES (Major chains)
		// =============================================================================
		{ patterns: ['TESCO'], target: 'Tesco', category: 'Groceries' },
		{ patterns: ['SAINSBURY'], target: 'Sainsburys', category: 'Groceries' },
		{ patterns: ['WAITROSE'], target: 'Waitrose', category: 'Groceries' },
		{ patterns: ['LIDL'], target: 'Lidl', category: 'Groceries' },
		{ patterns: ['ALDI'], target: 'Aldi', category: 'Groceries' },

		// =============================================================================
		// SHOPPING (Major retailers)
		// =============================================================================
		{ patterns: ['AMAZON'], target: 'Amazon', category: 'Shopping' },
		{ patterns: ['EBAY'], target: 'eBay', category: 'Shopping' },

		// =============================================================================
		// LOCAL BUSINESSES
		// =============================================================================
		{ patterns: ['LOCAL COFFEE SHOP'], target: 'Coffee Shop', category: 'Food' },
		{ patterns: ['NEIGHBORHOOD GROCERY'], target: 'Local Grocery', category: 'Groceries' },

		// =============================================================================
		// PERSONAL CONTACTS
		// =============================================================================
		{ patterns: ['FRIEND NAME'], target: 'Friend Name' },
		{ patterns: ['FAMILY MEMBER'], target: 'Family Member' },

		// =============================================================================
		// INTERNAL TRANSFERS
		// =============================================================================
		{ patterns: ['YOUR BUSINESS NAME'], target: 'Business Transfer' },
		{ patterns: ['Sent money to Your Name'], target: 'Internal Transfer' },

		// =============================================================================
		// BILLS & DIRECT DEBITS
		// =============================================================================
		{ patterns: ['LOCAL COUNCIL'], target: 'Council Tax', category: 'Bills' },
		{ patterns: ['LANDLORD NAME'], target: 'Rent', category: 'Rent' },

		// =============================================================================
		// DIRECT DEBIT REFERENCES
		// Replace these with your actual direct debit reference numbers
		// =============================================================================
		// { patterns: ['12345678'], target: 'Description', category: 'Bills' },
	],
	warnOnUnmapped: true,
	fallbackToRaw: true,
};
