# Production releases

1. Validate changes locally (`cargo fmt --check`, `cargo clippy --release --all-targets -- -D warnings`, `cargo test`). Never commit `.env`, `target/` or test PDFs.
2. Create a pull request from `staging` targeting `master`.
3. Use the next available semantic version. Merge commit message must be exactly `release(project): bump to vX.Y.Z`.
4. Merge the PR using a merge commit, not a direct push to `master`.
5. `.github/workflows/release.yaml` extracts the version from the merge commit, creates the annotated `vX.Y.Z` tag and publishes the GitHub Release. Verify all three: workflow success, tag, release.
6. Never rewrite published history or reuse an existing release tag.
7. Deployment is performed by the user unless explicitly requested. A release is not proof of deployment. Never trigger an automatic deployment without authorization.
8. When deployment is requested, keep the server `.env`, rebuild with `docker compose up -d --build` and verify the `axum` container is healthy.
