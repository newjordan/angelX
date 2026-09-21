# Brand source art

The originals behind the mark, kept out of the deployed tree. The Vercel
project's root directory is `website/`, so nothing in this folder is built or
served — these live here so the art survives without shipping bytes no visitor
loads.

| file | size | what it is |
| --- | --- | --- |
| `angelx-final.png` | 1536×1024 | the master composition the live wordmark was trimmed from |
| `logo.png` | 1536×520 | the earlier wordmark art, superseded by the trimmed mark |

## What is actually live

`website/assets/logo-mark.png` (1429×331) is the mark the site loads — the
master trimmed to the letters. It is rendered through
`h1.logo img{filter:url(#tone)}` in `website/css/style.css`, which deliberately
keeps the artwork's own chrome tint while still applying the page's shared tone
curve, so the mark is the one thing on the page that is not fully monochrome.

The harness app carries its own copies of the agent art under
`cockpit/assets/`; nothing here is compiled into the binary. Provenance for the
agent sheets is recorded in `cockpit/assets/agents/helms/manifest.json`.
