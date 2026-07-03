# wine-app -- project conventions

## Code style

- **No emojis in code or comments.** Not in `.rs`, `.toml`, `.js`, Dockerfiles,
  build scripts, config, or any comment. Emojis are permitted ONLY in content
  rendered to the end user (Askama templates / static HTML shown in the
  browser). Keep code ASCII-clean. (Vendored third-party files such as
  `static/datastar.js` are not ours to modify.)
