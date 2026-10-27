# Issue tracker: GitHub

Issues, specs, and tickets live in this repository's GitHub Issues. Use the `gh` CLI. Infer the repository from `git remote -v` when working inside the checkout. Outside it, pass `-R <owner>/<repo>` to `gh issue` and `gh pr` commands. For `gh api`, put the owner and repository in the endpoint path or set `GH_REPO`.

## Conventions

- Create an issue: `gh issue create --title "..." --body-file <body-file>`.
- Read an issue and its discussion: `gh issue view <number> --comments`. Use `--json number,title,body,labels,comments,assignees,state` when structured output is needed.
- List issues: `gh issue list --state open --json number,title,body,labels,comments,assignees`, with appropriate `--label` and `--state` filters. Use `--limit` or API pagination when the complete issue set is needed.
- Comment: `gh issue comment <number> --body-file <body-file>`.
- Add or remove labels: `gh issue edit <number> --add-label "..."` or `--remove-label "..."`.
- Close: `gh issue close <number>`. Add a separate comment when an explanation is needed.

Keep multiline issue and comment bodies in a temporary UTF-8 file and pass it with `--body-file`.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub shares a number space across issues and pull requests. When a reference is ambiguous, resolve it with `gh pr view <number>` and fall back to `gh issue view <number>`.

## Skill operations

When a skill says "publish to the issue tracker", create a GitHub issue. When it says "fetch the relevant ticket", read the issue and its comments.

## Wayfinding operations

- Map: one issue labelled `wayfinder:map`, with Notes, Decisions-so-far, and Fog sections.
- Child ticket: an issue linked to the map through GitHub sub-issues, labelled `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`. If sub-issues are unavailable, use a task list in the map and a `Part of #<map>` line in each child.
- Blocking: use native issue dependencies. Add an edge with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`. The blocker ID is the numeric database ID from `gh api repos/<owner>/<repo>/issues/<number> --jq .id`, not its issue number or node ID. If dependencies are unavailable, put `Blocked by: #<number>, #<number>` at the top of the child body.
- Frontier: examine the map's open children in map order. Exclude assigned tickets and tickets with open blockers. Query the blocked-by endpoint or resolve every fallback reference; a ticket is unblocked when all blockers are closed.
- Claim: `gh issue edit <number> --add-assignee @me`.
- Resolve: comment with the answer, close the child, and append a short description and link to the map's Decisions-so-far section.

GitHub documents [sub-issue operations](https://docs.github.com/en/rest/issues/sub-issues) and [issue dependency operations](https://docs.github.com/en/rest/issues/issue-dependencies).
