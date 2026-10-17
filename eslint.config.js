'use strict';

const globals = require('globals');

module.exports = [
    {
        files: ['**/*.js'],
        ignores: ['node_modules/**', 'data/**', 'bin/**', 'src/public/**'],
        languageOptions: {
            ecmaVersion: 2023,
            sourceType: 'commonjs',
            globals: { ...globals.node },
        },
        rules: {
            'no-unused-vars': ['error', { argsIgnorePattern: '^_', caughtErrors: 'none' }],
            'no-undef': 'error',
            eqeqeq: ['error', 'smart'],
            'no-var': 'error',
            'prefer-const': 'error',
        },
    },
];
