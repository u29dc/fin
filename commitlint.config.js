export default {
	extends: ['@commitlint/config-conventional'],
	rules: {
		'type-enum': [2, 'always', ['feat', 'fix', 'refactor', 'docs', 'style', 'chore', 'test']],
		'scope-empty': [2, 'never'],
		'scope-enum': [2, 'always', ['web', 'core', 'cli', 'db', 'import', 'config', 'deps', 'docs', 'ci']],
		'subject-empty': [2, 'never'],
		'subject-case': [2, 'always', ['lower-case']],
		'header-max-length': [2, 'always', 100],
		'subject-full-stop': [2, 'never', '.'],
		'body-max-line-length': [2, 'always', 72],
	},
	helpUrl: 'https://github.com/conventional-changelog/commitlint/#what-is-commitlint',
};
