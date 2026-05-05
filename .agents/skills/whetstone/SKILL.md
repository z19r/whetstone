```markdown
# whetstone Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `whetstone` Rust repository. You'll learn about file naming, import/export styles, commit message conventions, and how to write and organize tests. While no explicit workflows were detected, this guide provides best practices and command suggestions for common development tasks.

## Coding Conventions

### File Naming
- Use **snake_case** for file names.
  - Example: `my_module.rs`, `data_processor.rs`

### Import Style
- Use **relative imports** within the codebase.
  - Example:
    ```rust
    mod utils;
    use crate::utils::helper_function;
    ```

### Export Style
- Use **named exports** for modules, functions, and structs.
  - Example:
    ```rust
    pub fn process_data() { ... }
    pub struct DataModel { ... }
    ```

### Commit Messages
- Follow the **conventional commit** format.
- Use the `feat` prefix for new features.
- Average commit message length: ~62 characters.
  - Example:
    ```
    feat: add data normalization to processing pipeline
    ```

## Workflows

### Feature Development
**Trigger:** When adding a new feature  
**Command:** `/feature`

1. Create a new branch for your feature.
2. Implement the feature in a new or existing camelCase file.
3. Use relative imports for dependencies.
4. Export new functions or structs using named exports.
5. Write or update tests in a corresponding `*.test.*` file.
6. Commit changes with a `feat:` prefix and a descriptive message.
7. Open a pull request for review.

### Testing
**Trigger:** When verifying code correctness  
**Command:** `/test`

1. Locate or create a test file matching the `*.test.*` pattern.
2. Write tests for your modules or functions.
3. Run tests using the Rust testing tool:
    ```sh
    cargo test
    ```
4. Ensure all tests pass before merging changes.

## Testing Patterns

- Test files follow the `*.test.*` naming pattern.
  - Example: `data_processor.test.rs`
- Testing framework is not explicitly specified; use Rust's built-in test framework.
- Example test:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_process_data() {
            assert_eq!(process_data(), expected_result);
        }
    }
    ```

## Commands
| Command    | Purpose                                 |
|------------|-----------------------------------------|
| /feature   | Start a new feature development workflow |
| /test      | Run and verify tests                    |
```
