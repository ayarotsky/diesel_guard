export default {
    extends: [
        '@commitlint/config-conventional',
    ],
    rules: {
        "header-max-length": [2, "always", 80],
        "subject-case": [2, "always", ["sentence-case"]],
        "type-enum": [
            2,
            "always",
            [
                "chore",
                "ci",
                "docs",
                "feat",
                "fix",
                "perf",
                "refactor",
                "revert",
                "style",
                "test",
                "build",
                "deps",
            ],
        ],
    },
}
