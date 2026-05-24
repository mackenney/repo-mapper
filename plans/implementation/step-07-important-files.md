# Step 07: Important Files

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 4 — This step runs in parallel with graph construction.

### This Step
Implement the important files list per SPEC §8.

## Prerequisites
- Step 02 complete (path utilities)

## Files to Read Before Starting
- `SPEC.md` §8.1, §8.2 — important file definitions, ordering guarantee
- `src/important.rs` — current stub

## Implementation

### Task 1: Implement important files list in src/important.rs

```rust
//! Important files list (SPEC §8).

/// Check if a relative filename matches the important files list.
///
/// Per SPEC §8.1, matches exact filenames and the `.github/workflows/*.yml` pattern.
pub fn is_important(rel_fname: &str) -> bool {
    // Check exact match against the list
    if IMPORTANT_FILES.contains(&rel_fname) {
        return true;
    }

    // Check .github/workflows/*.yml pattern
    if rel_fname.starts_with(".github/workflows/") && rel_fname.ends_with(".yml") {
        return true;
    }

    false
}

/// Filter a list of relative filenames to only important ones.
pub fn filter_important_files<'a>(files: &[&'a str]) -> Vec<&'a str> {
    files.iter().copied().filter(|f| is_important(f)).collect()
}

/// Static list of important files (SPEC §8.1).
pub static IMPORTANT_FILES: &[&str] = &[
    // Version control
    ".gitignore",
    ".gitattributes",
    // Documentation
    "README",
    "README.md",
    "README.txt",
    "README.rst",
    "CONTRIBUTING",
    "CONTRIBUTING.md",
    "CONTRIBUTING.txt",
    "CONTRIBUTING.rst",
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "CHANGELOG",
    "CHANGELOG.md",
    "CHANGELOG.txt",
    "CHANGELOG.rst",
    "SECURITY",
    "SECURITY.md",
    "SECURITY.txt",
    "CODEOWNERS",
    // Package management
    "requirements.txt",
    "Pipfile",
    "Pipfile.lock",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "npm-shrinkwrap.json",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "composer.lock",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "build.sbt",
    "go.mod",
    "go.sum",
    "Cargo.toml",
    "Cargo.lock",
    "mix.exs",
    "rebar.config",
    "project.clj",
    "Podfile",
    "Cartfile",
    "dub.json",
    "dub.sdl",
    // Configuration
    ".env",
    ".env.example",
    ".editorconfig",
    "tsconfig.json",
    "jsconfig.json",
    ".babelrc",
    "babel.config.js",
    ".eslintrc",
    ".eslintignore",
    ".prettierrc",
    ".stylelintrc",
    "tslint.json",
    ".pylintrc",
    ".flake8",
    ".rubocop.yml",
    ".scalafmt.conf",
    ".dockerignore",
    ".gitpod.yml",
    "sonar-project.properties",
    "renovate.json",
    "dependabot.yml",
    ".pre-commit-config.yaml",
    "mypy.ini",
    "tox.ini",
    ".yamllint",
    "pyrightconfig.json",
    // Build
    "webpack.config.js",
    "rollup.config.js",
    "parcel.config.js",
    "gulpfile.js",
    "Gruntfile.js",
    "build.xml",
    "build.boot",
    "project.json",
    "build.cake",
    "MANIFEST.in",
    // Testing
    "pytest.ini",
    "phpunit.xml",
    "karma.conf.js",
    "jest.config.js",
    "cypress.json",
    ".nycrc",
    ".nycrc.json",
    // CI/CD
    ".travis.yml",
    ".gitlab-ci.yml",
    "Jenkinsfile",
    "azure-pipelines.yml",
    "bitbucket-pipelines.yml",
    "appveyor.yml",
    "circle.yml",
    ".circleci/config.yml",
    ".github/dependabot.yml",
    "codecov.yml",
    ".coveragerc",
    // Containers
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.override.yml",
    // Cloud/serverless
    "serverless.yml",
    "firebase.json",
    "now.json",
    "netlify.toml",
    "vercel.json",
    "app.yaml",
    "terraform.tf",
    "main.tf",
    "cloudformation.yaml",
    "cloudformation.json",
    "ansible.cfg",
    "kubernetes.yaml",
    "k8s.yaml",
    // Database
    "schema.sql",
    "liquibase.properties",
    "flyway.conf",
    // Frameworks
    "next.config.js",
    "nuxt.config.js",
    "vue.config.js",
    "angular.json",
    "gatsby-config.js",
    "gridsome.config.js",
    // API docs
    "swagger.yaml",
    "swagger.json",
    "openapi.yaml",
    "openapi.json",
    // Dev environment
    ".nvmrc",
    ".ruby-version",
    ".python-version",
    "Vagrantfile",
    // Quality
    ".codeclimate.yml",
    // Docs tooling
    "mkdocs.yml",
    "_config.yml",
    "book.toml",
    "readthedocs.yml",
    ".readthedocs.yaml",
    // Package registries
    ".npmrc",
    ".yarnrc",
    // Linting
    ".isort.cfg",
    ".markdownlint.json",
    ".markdownlint.yaml",
    // Security
    ".bandit",
    ".secrets.baseline",
    // Misc
    ".pypirc",
    ".gitkeep",
    ".npmignore",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_important_exact_match() {
        assert!(is_important("README.md"));
        assert!(is_important("Cargo.toml"));
        assert!(is_important("package.json"));
        assert!(is_important(".gitignore"));
        assert!(is_important("Dockerfile"));
    }

    #[test]
    fn is_important_github_workflows() {
        assert!(is_important(".github/workflows/ci.yml"));
        assert!(is_important(".github/workflows/release.yml"));
        assert!(is_important(".github/workflows/test.yml"));
    }

    #[test]
    fn is_not_important() {
        assert!(!is_important("src/main.rs"));
        assert!(!is_important("lib/utils.py"));
        assert!(!is_important("random.txt"));
        // Workflow files not in .github/workflows/
        assert!(!is_important("workflows/ci.yml"));
        // Non-yml files in workflows
        assert!(!is_important(".github/workflows/config.yaml"));
    }

    #[test]
    fn filter_important() {
        let files = vec![
            "README.md",
            "src/main.rs",
            "Cargo.toml",
            "lib/utils.rs",
            ".github/workflows/ci.yml",
        ];

        let important = filter_important_files(&files);
        assert_eq!(important.len(), 3);
        assert!(important.contains(&"README.md"));
        assert!(important.contains(&"Cargo.toml"));
        assert!(important.contains(&".github/workflows/ci.yml"));
        assert!(!important.contains(&"src/main.rs"));
    }

    #[test]
    fn important_list_coverage() {
        // Verify key categories are represented
        assert!(IMPORTANT_FILES.contains(&".gitignore")); // Version control
        assert!(IMPORTANT_FILES.contains(&"README.md")); // Documentation
        assert!(IMPORTANT_FILES.contains(&"package.json")); // Package management
        assert!(IMPORTANT_FILES.contains(&"tsconfig.json")); // Configuration
        assert!(IMPORTANT_FILES.contains(&"Dockerfile")); // Containers
        assert!(IMPORTANT_FILES.contains(&".travis.yml")); // CI/CD
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test important` exits 0
- [ ] `is_important` returns true for exact matches (README.md, Cargo.toml, etc.)
- [ ] `is_important` returns true for `.github/workflows/*.yml` pattern
- [ ] `is_important` returns false for regular source files
- [ ] `IMPORTANT_FILES` list covers all categories from SPEC §8.1

## Reviewer Instructions

You are reviewing Step 07. Verify:

1. Run `cargo test important::tests` — all tests must pass
2. Verify `IMPORTANT_FILES` contains entries from all SPEC §8.1 categories (version control, documentation, package management, config, build, testing, CI/CD, containers, cloud, database, frameworks, API docs, dev environment, quality, docs tooling, package registries, linting, security, misc)
3. Check `.github/workflows/*.yml` pattern matching in `is_important`
4. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/important.rs
```
