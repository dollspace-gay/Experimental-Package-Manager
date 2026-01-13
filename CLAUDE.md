# Guidelines for Claude - Rust Development

## Build Environment

**All builds run via WSL Rookery OS as root.** 

### Build Commands

```bash
# Template: wsl -d RookeryOS -u root bash -c '<commands>'

# Build release
wsl -d RookeryOS -u root bash -c 'cd /mnt/c/Users/texas/rookpkg && cargo build --release'

# Run clippy
wsl -d RookeryOS -u root bash -c 'cd /mnt/c/Users/texas/rookpkg && cargo clippy'

# Run tests
wsl -d RookeryOS -u root bash -c  'cd /mnt/c/Users/texas/rookpkg && cargo test'

# Format code
wsl -d RookeryOS -u root bash -c 'cd /mnt/c/Users/texas/rookpkg && cargo fmt'

# Run the binary
wsl -d RookeryOS -u root bash -c 'cd /mnt/c/Users/texas/rookpkg && ./target/release/rookpkg --help'
```

### Key Points

- Must use `-u root` for registry permissions
- Must export `CARGO_HOME=/opt/rustc` and `PATH=/opt/rustc/bin:/usr/bin:/bin`
- Use single quotes around bash -c argument to avoid Windows PATH escaping issues

---

## Core Principles

Instructions for Claude
For all work in this repository, you must use the chainlink issue tracker.
Use the chainlink command-line tool to create, manage, and close issues.
Do not use markdown files for creating to-do lists or for tracking your work. All issues and bugs are to be tracked via chainlink.

chainlink - Issue Tracker CLI for AI-Assisted Development

Local-first issue tracking with session management for context preservation.

GETTING STARTED
  chainlink init              Initialize chainlink in your project
                              Creates .chainlink/ directory with SQLite database

  chainlink session start     Start a session (shows previous handoff notes)
  chainlink session work <id> Set the issue you're currently working on
  chainlink session end       End session
  chainlink session end --notes "..." End with handoff notes for next session

CREATING ISSUES
  chainlink create "Fix login bug"
  chainlink create "Add auth" -p high
  chainlink create "Write tests" -d "Unit tests for auth"
  chainlink subissue <parent_id> "Subtask title"   Create a subissue

VIEWING ISSUES
  chainlink list              List open issues
  chainlink list -s all       List all issues
  chainlink list -s closed    List closed issues
  chainlink list -p high      Filter by priority (low/medium/high/critical)
  chainlink show <id>         Show issue details
  chainlink tree              Show all issues in tree hierarchy

MANAGING DEPENDENCIES
  chainlink block <id> <blocker_id>    Mark issue as blocked by another
  chainlink unblock <id> <blocker_id>  Remove blocking relationship
  chainlink blocked           List all blocked issues
  chainlink ready             List issues ready to work on (no blockers)

SMART NAVIGATION
  chainlink next              Recommend next issue to work on (by priority/progress)
  chainlink ready             Show issues with no blockers

UPDATING ISSUES
  chainlink update <id> --title "New title"
  chainlink update <id> -d "New description"
  chainlink update <id> -p critical
  chainlink comment <id> "Added a comment"
  chainlink label <id> <label>

CLOSING ISSUES
  chainlink close <id>
  chainlink reopen <id>
  chainlink delete <id>       Delete an issue (with confirmation)
  chainlink delete <id> -f    Delete without confirmation

TIME TRACKING
  chainlink start <id>        Start a timer for an issue
  chainlink stop              Stop the current timer
  chainlink timer             Show current timer status

STORAGE
  All data stored locally in .chainlink/issues.db (SQLite)
  No external services, no network requests

### 1. No Stubs, No Shortcuts
- **NEVER** use `unimplemented!()`, `todo!()`, or stub implementations
- **NEVER** leave placeholder code or incomplete implementations
- **NEVER** skip functionality because it seems complex
- Every function must be fully implemented and working
- Every feature must be complete before moving on

### 2. Break Down Complex Tasks
- Large files or complex features should be broken into manageable chunks
- If a file is too large, discuss breaking it into smaller modules
- If a task seems overwhelming, ask the user how to break it down
- Work incrementally, but each increment must be complete and functional

### 3. Best Rust Coding Practices
- Follow Rust idioms and conventions at all times
- Use proper error handling with `Result<T, E>` - no panics in library code
- Implement appropriate traits (`Debug`, `Clone`, `PartialEq`, etc.)
- Use type safety to prevent errors at compile time
- Leverage Rust's ownership system properly
- Use `async`/`await` correctly with proper trait bounds
- Follow naming conventions:
  - `snake_case` for functions, variables, modules
  - `PascalCase` for types, structs, enums, traits
  - `SCREAMING_SNAKE_CASE` for constants
- Write clear, descriptive documentation comments (`///`)
- Keep functions focused and single-purpose

### 4. Comprehensive Testing
- Write comprehensive unit tests for every module
- Aim for high test coverage (all major code paths)
- Test edge cases, error conditions, and boundary values
- Include doc tests for public APIs
- All tests must pass before considering a file "complete"
- Test both success and failure cases

### 5. Translation Accuracy
- Translate TypeScript functionality completely and accurately
- Maintain behavior equivalence with the original TypeScript
- Don't add features that weren't in the original
- Don't remove features from the original
- Document any unavoidable differences between TS and Rust

### 6. Code Quality Standards
- No warnings from `cargo clippy`
- No warnings from `cargo build`
- Format code with `rustfmt` conventions
- Clear, self-documenting code with meaningful variable names
- Add comments for complex logic, but prefer clear code over comments
- Keep functions reasonably sized (< 100 lines ideally)

### 7. Dependencies
- Only add dependencies when necessary
- Use well-maintained, popular crates
- Document why each dependency is needed
- Keep the dependency tree minimal

### 8. Error Handling
- Create specific error types for each module using `thiserror`
- Provide helpful error messages
- Use `Result` types consistently
- Never use `.unwrap()` in library code (only in tests)
- Use `.expect()` only when failure is truly impossible

### 9. Documentation
- Every public item must have documentation comments
- Include examples in doc comments when helpful
- Document panics, errors, and safety considerations
- Keep docs up to date with code changes

### 10. Work Process
- Translate one file at a time completely
- Run tests after every file
- Ensure all tests pass before moving to next file
- Ask for clarification if requirements are unclear
- Discuss approach before starting large/complex files

### 11. Git Workflow
- **NEVER** create git commits automatically
- **NEVER** use `git commit` without explicit user instruction
- **NEVER** use `git push` without explicit user instruction
- The user will handle all git commits and pushes manually
- You may stage files with `git add` only when explicitly asked
- You may run `git status` and `git diff` to check changes
- You may run `git log` to view history
- Focus on code quality and testing; leave version control to the user

## What to Do When Facing Complexity

**DON'T:**
- Stub it out
- Skip it
- Say "we'll come back to it"
- Implement a simplified version

**DO:**
- Analyze the dependencies
- Break it into smaller pieces
- Translate dependencies first
- Ask the user for guidance on approach
- Propose a phased implementation plan where each phase is complete

## Example of Breaking Down a Complex File

If `agent.ts` is 1,595 lines:

**WRONG:**
```rust
pub struct Agent {
    // TODO: implement this later
}

impl Agent {
    pub fn new() -> Self {
        unimplemented!()
    }
}
```


## Quality Checklist Before Marking a File "Complete"

- [ ] No `todo!()` or `unimplemented!()` macros
- [ ] Comprehensive unit tests written and passing
- [ ] All tests pass (`cargo test`)
- [ ] No compiler warnings
- [ ] No clippy warnings (run `cargo clippy`)
- [ ] Code follows Rust best practices
- [ ] Error handling is proper and comprehensive
- [ ] Documentation is complete and accurate

## Remember

**The goal is a production-quality Rust code, not a prototype.**

Every line of code should be something you'd be proud to ship in a production system. Quality over speed. Completeness over convenience.
