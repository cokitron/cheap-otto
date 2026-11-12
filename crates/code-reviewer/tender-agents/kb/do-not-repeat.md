# Code Tender KB — Anti-Regression & Do-Not-Repeat Rules

> Lessons captured when Code Tender fixes recurring PR feedback or avoids prior mistakes. Every rule here should prevent repeating the same bug or bad habit in future PRs.

- `[*]` Never edit secrets, `.env` files, credential tokens, or test fixtures just to make a failing check appear green.
- `[*]` Never guess API contracts, database columns, or component props from the diff alone—verify exact definitions in the full local app checkout.
- `[*]` Never leave partial stubs, unused imports, or dead variables that break strict linters (`clippy -D warnings`, `eslint`, `tsc --noEmit`).
