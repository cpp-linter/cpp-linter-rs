# Supported Git Servers

[git-bot-feedback]: https://crates.io/crates/git-bot-feedback
[gitlab-permissions]: https://github.com/2bndy5/git-bot-feedback/blob/main/docs/permissions/gitlab.md
[gitlab-masked-vars]: https://docs.gitlab.com/ci/variables/#mask-a-cicd-variable
[gitlab-mr-pipelines]: https://docs.gitlab.com/ci/pipelines/merge_request_pipelines/
[forgejo-runner]: https://forgejo.org/docs/latest/admin/actions/

[quick-start]: index.md#quick-start
[thread-comments]: cli.md#-t-thread-comments
[pr-review]: cli.md#-p-pr-review
[passive-reviews]: cli.md#-r-passive-reviews
[no-lgtm]: cli.md#-g-no-lgtm
[file-annotations]: cli.md#-a-file-annotations
[step-summary]: cli.md#-w-step-summary
[lines-changed-only]: cli.md#-l-lines-changed-only
[files-changed-only]: cli.md#-f-files-changed-only
[diff-base]: cli.md#-b-diff-base

cpp-linter is not limited to GitHub. It detects the git server from the environment variables that
the CI runner sets, and uses [git-bot-feedback][] to talk to the server's REST API.

| The CI runner sets | Git server | Token is read from |
|---|---|---|
| `GITHUB_ACTIONS=true` | GitHub | `GITHUB_TOKEN` |
| `GITEA_ACTIONS=true` | Gitea | `GITEA_TOKEN` |
| `FORGEJO_ACTIONS=true` | Forgejo | `GITEA_TOKEN`, or `FORGEJO_TOKEN` (set by Forgejo's runner) |
| `GITLAB_CI=true` | GitLab | `GITLAB_TOKEN` |
| none of the above | none (local) | not used |

Gitea and Forgejo runners also set `GITHUB_ACTIONS=true`, so they are checked first.

Without a known CI environment, cpp-linter runs locally. The list of changed files then comes from
`git diff` (see [`--diff-base`][diff-base]), and nothing is posted to a server.

## What works where

| Feature | GitHub | Gitea and Forgejo | GitLab |
|---|---|---|---|
| Changed files of a pull request ([`--lines-changed-only`][lines-changed-only], [`--files-changed-only`][files-changed-only]) | yes | yes | yes (merge request) |
| Changed files of a push | yes | yes | yes |
| [`--thread-comments`][thread-comments] on a pull request | yes | yes | yes (merge request) |
| [`--thread-comments`][thread-comments] on a commit (push) | yes | no | no |
| [`--pr-review`][pr-review] | yes | yes | yes |
| [`--file-annotations`][file-annotations] | yes | highlighted lines in the job's log | no |
| [`--step-summary`][step-summary] | yes | appended to the file named by `GITHUB_STEP_SUMMARY` | no |
| Output variables (`checks-failed` and others) | yes | yes | printed in the log only |
| Collapsed groups in the log | yes | yes | yes |

On every server, feedback is only posted to pull requests from a branch in the same repository.
For a pull request from a fork, cpp-linter still analyzes the changed files, but it skips the thread
comment and the review.

## Gitea

Use cpp-linter in Gitea Actions like in GitHub Actions. The differences are:

- The token must be in `GITEA_TOKEN`. `GITHUB_TOKEN` is not read on Gitea. Without a token,
  posting a thread comment or a review fails the run.
- Thread comments are only posted for pull requests, not for commits.
- Only a repository admin can dismiss a review. When the problems of an earlier review are fixed,
  cpp-linter tries to dismiss that review. With the token that Gitea creates for a job, this fails
  with `Must be repo admin` in the log, and the review that requested changes stays. To avoid a
  stale blocking review, use [`--no-lgtm=false`][no-lgtm] (cpp-linter then posts an approving
  review, which replaces the earlier one), or use [`--passive-reviews`][passive-reviews].
- Deleting a review's outdated comments needs Gitea v1.25 or newer. Older versions only resolve them.

```yaml title=".gitea/workflows/cpp-linter.yml"
name: cpp-linter
on: pull_request

jobs:
  cpp-linter:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # install cpp-linter here, see "Quick start"
      - name: Run cpp-linter
        env:
          GITEA_TOKEN: ${{ secrets.GITEA_TOKEN }}
        run: >-
          cpp-linter
          --style=file
          --tidy-checks=''
          --thread-comments=update
          --no-lgtm=false
          --pr-review=true
```

See [Quick start][quick-start] for the ways to install cpp-linter.

## Forgejo

Forgejo uses the same client as Gitea, so everything in the [Gitea](#gitea) section applies.
In addition:

- [Forgejo Runner][forgejo-runner] v7.0.0 or newer is needed. Older runners do not set
  `FORGEJO_ACTIONS`.
- The runner sets `FORGEJO_TOKEN` for each job, and cpp-linter uses it when `GITEA_TOKEN` is not set.
- A review's outdated comments are neither resolved nor deleted.

## GitLab

GitLab v17.9 or newer is needed.

- Store an access token as a [masked CI/CD variable][gitlab-masked-vars] named `GITLAB_TOKEN`.
  The job token (`CI_JOB_TOKEN`) is not used. See [GitLab token permissions][gitlab-permissions]
  for the scopes and the role that each feature needs.
  Comments and reviews are posted as the token's user, so a project or group access token
  (which has its own bot user) is a good fit.
- Run cpp-linter in a [merge request pipeline][gitlab-mr-pipelines]. Only these pipelines have
  the `CI_MERGE_REQUEST_IID` variable, which tells cpp-linter that the job belongs to a merge request.
  In other pipelines, cpp-linter checks the changes of the pipeline's commit and posts no comments.
- Thread comments are only posted for merge requests, not for commits.
- GitLab has no step summary and no output variables. The values of the output variables are
  printed in the job's log, for example `checks-failed: 5`.
- cpp-linter exits with code 0 when checks fail. Because there are no output variables, a job that
  should fail on findings has to look for the `checks-failed` line in the log.
- Turn off [`--file-annotations`][file-annotations]. GitLab has no equivalent, and each annotation
  would be printed in the job's log instead.

```yaml title=".gitlab-ci.yml"
cpp-linter:
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  script:
    # install cpp-linter here, see "Quick start"
    - >-
      cpp-linter
      --style=file
      --tidy-checks=''
      --thread-comments=update
      --pr-review=true
      --file-annotations=false
```

The token is not passed in the example because GitLab exposes CI/CD variables to the job as
environment variables.
