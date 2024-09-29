<!-- markdownlint-disable MD024 -->

# Token Permissions

This is an exhaustive list of required permissions organized by features.

> [!IMPORTANT]
> The `GITHUB_TOKEN` environment variable should be supplied when running on a private repository.
> Otherwise the runner does not not have the privileges needed for the features mentioned here.
>
> See also [Authenticating with the `GITHUB_TOKEN`](https://docs.github.com/en/actions/reference/authentication-in-a-workflow)

[push-events]: https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#push
[pr-events]: https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request

## File Changes

When using [`--files-changed-only`](cli.md#-f-files-changed-only) or
[`--lines-changed-only`](cli.md#-l-lines-changed-only) to get the list
of file changes for a CI event, the following permissions are needed:

### Push

For [`push` events][push-events].

```yaml
    permissions:
        contents: read
```

The `contents` permission is also needed to download files if the repository is not
checked out before running cpp-linter.

### Pull Request

For [`pull_request` events][pr-events].

```yaml
    permissions:
        contents: read
        pull-requests: read
```

For pull requests, the `contents` permission is only needed to download files if
the repository is not checked out before running cpp-linter.

* Specifying `write` to the `pull-requests` permission is also sufficient as that is
  required for
  * posting [thread comments](#thread-comments) on pull requests
  * posting [pull request reviews](#pull-request-reviews)

## Thread Comments

The [`--thread-comments`](cli.md#-g-thread-comments) feature requires the following permissions:

### Push

For [`push` events][push-events].

```yaml
    permissions:
      metadata: read
      contents: write
```

* The `metadata` permission is needed to fetch existing comments.
* The `contents` permission is needed to post or update a commit comment.
  This also allows us to delete an outdated comment if needed.

### Pull Request

For [`pull_request` events][pr-events].

```yaml
    permissions:
      pull-requests: write
```

## Pull Request Reviews

The [`tidy-review`](cli.md#-d-tidy-review), [`format-review`](cli.md#-m-format-review), and [`passive-reviews`](cli.md#-r-passive-reviews) features require the following permissions:

```yaml
    permissions:
      pull-requests: write
```
