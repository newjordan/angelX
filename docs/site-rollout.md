# Publishing omni-space.tech

The product website is https://omni-space.tech. Its pages are the static files
in `website/`: install, docs rendered from this repository, the Glass reel, and
the measurements already published in the README and release notes. Source and
release archives stay at https://github.com/newjordan/angelX.

This note is the later cutover. It does not change DNS, Vercel project
settings, or a production deployment, and it does not submit an application or
accept a hosting commitment.

## What was verified on 2026-11-06

Public responses, without changing anything:

| URL | Status | Bytes | Last-Modified |
|---|---:|---:|---|
| https://omni-space.tech/ | 200 | 8567 | Thu, 05 Nov 2026 17:48:25 GMT |
| https://www.omni-space.tech/ | 200 | 8567 | Thu, 05 Nov 2026 17:48:25 GMT |
| https://angelx.dev/ | 200 | 21434 | Thu, 05 Nov 2026 13:27:11 GMT |
| https://www.angelx.dev/ | 200 | 21434 | Thu, 05 Nov 2026 13:27:11 GMT |

8567 bytes is the company page that was live that day (the attached static
archive). It calls itself the company site and sends the product to angelx.dev.
21434 bytes is `website/index.html` from
`ac1df70f48e85f6d7f7e5928510d993cf242f439`, the previous product page. Neither
response is this tree: `main` did not contain `website/` (it was gitignored),
and later commits did not change those byte counts.

The Vercel API for team `team_kleiIeUkQvARXvZMrcgJMq8H` (`frosty40s-projects`)
returned 403 for project reads from this session, so project settings were not
re-read live. Merged PR #26
(https://github.com/newjordan/angelX/pull/26) records the linked project
`angelx` (`prj_erz49zmoqpoaYDE1u5IAJNSODxuG`) after that change:

- Root Directory is the repository root, not `website/`.
- The ignored build step exits 0. `vercel.json` at the repo root does the same.
- https://angelx.dev was left on its last good deployment. The byte count above
  matches that.

PR #24 failed earlier, while the linked project's root was still `website/` and
that directory was absent (`NOW_SANDBOX_WORKER_ROOTDIR_NOT_EXIST`). That root
was changed by PR #26. Do not point it back at `website/` as part of publishing
omni-space.tech: a root of `website/` would make the next git deploy replace
angelx.dev.

Project `omni-space` (`prj_LqqCHej5HW58qKHEbyjl2dCWRz5O`) was not readable
through the API here. The live page matches a static upload rather than this
git repo, which agrees with a CLI deploy and no Git connection. Treat that as
unconfirmed until the dashboard is checked. A merge to `main` does not update
omni-space.tech while that project has no Git link.

## Staged rollout

Do these in order. Stop if a preview is wrong. None of this is done by the
pull request.

1. Review the pull request. `node website/build.mjs --check` and
   `npm run test:website` should pass. Merging does not deploy either hostname:
   `website/` is not the linked project's root, and the ignore step still exits 0.
2. From a checkout of the merged commit:

   ```sh
   cd website
   python3 -m http.server 8080
   ```

   Check `/`, `/install.html`, `/docs/`, `/docs/models.html`, `/glass.html`,
   `/evidence.html` and `/company.html`. The canonical URL on each page is
   `https://omni-space.tech/...`. The Glass player uses the controls, does not
   autoplay, and picks the 16:9 or 9:16 file from the viewport.
3. In the Vercel dashboard, confirm project `omni-space` is still the static
   project for omni-space.tech and www.omni-space.tech, and that it is not Git
   connected. Confirm project `angelx` still has Root Directory `.` (the
   repository root) and an ignored build step.
4. Preview only, from `website/` on the merged commit. This creates a preview
   deployment and does not move the production domains:

   ```sh
   cd website
   vercel deploy --scope frosty40s-projects
   ```

   Use the existing `omni-space` project when the CLI asks. Do not pass
   `--prod`. On the preview URL, repeat the checks in step 2. Also confirm the
   preview does not redirect and does not describe another hostname as the
   product site.
5. When that preview is accepted, publish omni-space.tech. This is the
   production mutation, and it is the owner's:

   ```sh
   cd website
   vercel deploy --prod --scope frosty40s-projects
   ```

   If the domains already belong to project `omni-space`, this replaces the
   company page with this site. Do not change DNS to do that. Afterwards,
   `https://omni-space.tech/` and `https://www.omni-space.tech/` should return
   this site, with canonical `https://omni-space.tech/`.
6. Leave https://angelx.dev and https://www.angelx.dev on the deployment they
   have now. `vercel.json` must keep exiting 0. A later, separate approval can
   do one of these, not before step 5 is accepted:

   - Leave the hostname where it is.
   - Redirect the hostname to `https://omni-space.tech` in domain settings.
   - Or change project `angelx` to root `website/` and remove the ignore step,
     so it serves this same site. The canonical URL stays omni-space.tech.

## Blockers

- omni-space.tech does not build from this repository until someone runs the
  production deploy above, or connects that project to Git on purpose.
- Project settings and the production deploy were not changed here. API reads
  for the team were forbidden (403), so step 3 is still required.
- The older hostname keeps its current deployment on purpose. Removing the
  ignore step while the linked root is the repository root would publish the
  repository itself, not `website/`.
- No application is filed from this change, and the site does not state that a
  domain term or a funding programme has been accepted.
