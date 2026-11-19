# Contributing to EH3D-PHOENIX-3DP

Thank you for looking. A printer heats what it is told to heat, so a wrong temperature or a wrong move is worse than a refused job.

1. **Never panic and never guess.** What cannot be understood is an error that names the value and the reason.
2. **A limit is a refusal, never a trim.** A temperature or a move that is not allowed is an error and nothing is written for it.
3. **A change comes with a test**, and a test with damaged input when the change reads a file. Tests use a fixed seed.
4. **Clean-room.** The program is written from public protocol documentation, manuals and facts. Do not copy code, texts of an interface, icons, logos or profile files from any other slicer; the names and the art are our own.
5. **Before a pull request** run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `python -I tools/ci_validate.py`: all must pass.
6. **The README exists in eight languages** (English, Spanish, German, French, Italian, Japanese, Chinese and European Portuguese): a change to the text is made in all eight files.
7. Commits are written in English.
