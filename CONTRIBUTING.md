# Contributing

Mímir is free and open source under the [MIT licence](LICENSE). Use it, fork it,
change it and ship it. Issues and pull requests are welcome.

- **Bugs and ideas:** open an issue with what you ran, what you expected and what
  happened. `mimir doctor` and `journalctl --user -u mimir` help.
- **Pull requests:** keep each one to a single change and add a test when behaviour
  changes. Run `cargo test`, `cargo clippy` and `(cd web && npm run build)` before
  you push. The Rust code uses wide lines rather than default rustfmt output, so
  do not reformat whole files; match the style around your change.
- **Matching changes** (models, thresholds, the reranker instruction) need an A/B
  from `mimir eval CASES.tsv` on your own labelled prompts
  (`prompt<TAB>expected-skill`, `-` when no skill should match). Put the numbers in
  the changelog.
- **Security issues:** please report them privately through GitHub's security
  advisories rather than a public issue.
- **The website** is the static page in [site/](site/), published to GitHub Pages
  on every push to `main`.
