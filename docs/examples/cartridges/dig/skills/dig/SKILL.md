---
name: dig
description: Find files by what they show or say (pictures, PDFs, documents, recordings, video, code) with DigUp's local index, for a /dig description.
---

# DigUp in angelX

DigUp is plugged in as a cartridge. Its CLI runs on the cartridge's model and
index:

    ${ANGEL_CARTRIDGE_DIR}/bin/digup search <words> [--kind K,K] [-n 10] [--json]

- It ranks files by meaning (EmbeddingGemma 2: text, images, audio and video
  share one space) and by the exact words in names and text, so describe the
  thing ("the knight sheet with the lance first", "the clip where the bridge
  burns") or give an exact string (a file name, an error code).
- `--kind` narrows to `screenshot`, `image`, `pdf`, `doc`, `audio`, `video`.
  For something to look at, start with `--kind image,screenshot` (or
  `video`): a text file that describes a picture, like its generation prompt,
  outranks the picture itself.
- Pictures under about 200 px (native-size sprites), silent or very short
  sounds, and animated WebP aren't in the index; search their source sheets
  or the folder by name instead.
- A hit names the file and, in brackets, where in it: a PDF's page, a
  recording's or video's second, a document's passage. `--json` adds `loc`
  (page or seconds), `loc_end` and `excerpt`.
- `"code: <what it does>"` searches the code index, of folders indexed with
  `--code`; a hit names the line.
- Only indexed folders are searched. `bin/digup status` shows what is in the index.
  If the folder the operator means isn't there, say so; `digup estimate
  <folder>` gives the first pass's time, and `digup index <folder>` adds it
  (more folders only add; `--code` for the code index). A big first pass runs
  in the background. Running `digup index` on a folder again catches the index
  up with what changed there since.

The operator's task describes something to find. Search with their words
first, then rephrase or narrow by kind if the top hits miss. Look at picture,
frame and page hits yourself before naming one, and answer with the paths and
places that match, best first, saying which you checked.
