# angelface: the goddess in the window

The operator's words (2026-11-04): "we bring in nvidia audio2face, we use our local drama 3 outputs, we use a nice 3D model for the face, easy and rigged publicly available to start, rendered in chrome in glass, and make a layer for angelX that shows the avatar's face, talking to us, ambient animations, it will look down when we are typing sometimes in curiosity, and in general exist as a presence. we go kinda hard into sci fi on this one, surreal, like a goddess coming in over the tui."

She is a chrome head drawn by glass itself, floating over the terminal at its top right. Her irises burn ice white, her lips are rose gold, and her neck fades into the dark. She speaks in her own voice (Fish Audio Drama 3), and NVIDIA Audio2Face moves her face from the same audio.

Just her head, for now: the foundation. Her regalia, kept for later and off by default (`Drawing::regalia`), includes:

- gold and cyan rings turning about her head, with gems riding the inner one;
- a halo disc of rings, rays and a band of braille;
- a jewel on her brow;
- around her picture, an aura and rising motes.

## Calling her

- `ANGEL_GLASS_FACE=1` brings her in at launch (with the window, `ANGEL_GLASS=1`). `ANGEL_GLASS_FACE=quiet` brings her in without her voice.
- `/face` calls her in, or sends her away.
- `/face say <occasion|line>` makes her speak.
  - The occasions are `arrive`, `typing`, `think`, `done`, `error`, `idle`, `demo` and `audition`.
  - `/face lines` lists every line with its id.
- `/face mute` silences her, and `/face voice` frees her voice again.
- `/onboard`, `/face tour`, or a short message asking to be onboarded ("onboard me") starts her tour of the harness (below). `/onboard stop` ends it. Asking to be taught ("teach me ...") is the agent's to answer, never her tour.
- `/face lasers` sets off her spectacle now (below).
- `/face status` shows her frame rate, frame time, mesh traffic, size and what she is doing. Her thread logs arrivals, cues, the lines she says and a status line every ten seconds to `~/angelface/logs/face.log`.
- In a terminal session, `/face` only says that she lives in the window.

## What she does

- **She arrives.** For 3.6 seconds she forms: molten mercury settling to chrome (the `liquid` finish), her face rising to you, eyes closed until she is whole. Then she greets you.
- **She is present.**
  - She breathes and drifts.
  - She blinks, sometimes twice. Big turns of her eyes come with a blink.
  - She makes small saccades on your eyes.
  - Every few seconds she glances aside.
  - Now and then she glances at the pointer when it moves.
  - Rarely, she closes her eyes for a few seconds and listens to something far off.
  - The ponytail joints sway behind her head's turns (no hair is drawn on them yet).
- **She reads what you type, sometimes.** When a typing burst starts, she decides out of curiosity, not duty, about six times in ten.
  - If she reads, she looks down at the composer with her head tipped and tilted, brows raised, a trace of a smile, her eyes moving along the line.
  - Every few seconds she glances up at you.
  - When you stop typing she looks back up. When you send, she looks up and nods.
  - Rarely she murmurs ("Go on.", "Hm?").
- **She thinks while the agent works.** Her gaze drifts up and aside, her eyes narrow and her lips press, and the chrome quickens.
- **She answers a turn's end.**
  - A turn that ended well brings a smile and a nod, and sometimes "It is done." or "Good."
  - One that failed brings concern, and often "Something resisted."
- **She speaks** from the voice bank.
  - Audio2Face's track drives her mouth, jaw, cheeks and brows; her own mind keeps her gaze and blinks.
  - Her eyes brighten with her voice.
  - She grows a little and comes toward the middle of the window while she speaks, then settles back.
  - After four still minutes she may muse aloud.

## Her strangeness

- **The cube.** A cube sits in the top of her forehead. It turns slowly on three axes and drifts in and out of her skull: sunk in, clipping through, floating free. Its six faces go round the colours a little apart from each other, a slow hypnotic gradient.
- **The spectacle.** Every minute and a half to four, when she is not speaking or reading, she goes through it:
  1. Her eyes heat. Red light gathers in them while she scowls.
  2. Her eyes burn. Two beams, white at their heart, strike a point in the terminal (sometimes the pointer, sometimes the composer), throwing sparks.
  3. A breath.
  4. Her jaw drops and a rainbow pours out of her mouth. It arcs out over the terminal and falls into droplets.
  5. She recovers.

  Her lines wait until it is over. `/face lasers` sets it off at once.
- **She leans out of a corner.** Her neck is in the bottom right corner of the mini-viz panel and her picture is turned about 43 degrees, so her face leans up and out on the diagonal. She rises out of the corner when she comes, a little past her place and back, and her lean drifts.

## Her tour

She shows you the harness in eleven steps, surreal and a real tutorial at once, each a line in her voice, also written into the transcript. While she says a step, a limb bursts in from an edge of the window and points at what she names:

- the composer;
- the transcript (answers, receipts, approvals, `/diff`);
- the header (`/model`, `/think`);
- the world;
- the middle of the window, where the showcase opens;
- herself.

The limb is her own left arm, cut from the avatar, its hand posed pointing (index finger out along the arm, the others curled, the thumb up). It is stretched in chrome as long as the reach needs, its shoulder dissolving into nothing. About one gesture in four, and at least once a tour, it is a leg instead, the foot pointed and the toe doing the pointing. A limb overshoots as it arrives, taps at its target while she speaks, and withdraws.

What it points at is where the cockpit last drew it: `render_message_composer`, `render_transcript`, `render_header` and the mini-viz note their rectangles. The limb's picture is laid so its tip lands on the target (`draw::limb_rect`). Look at it with `cargo run -p angel-face --example tour_shot`, and at the limbs alone with `limb_shot`.

The words live in the cockpit (`TOUR` in `ui/angelface.rs`) and in the voice bank (`tour_*` in `goddess_bank.py`). Drama 3 renders each line a sentence at a time against her anchor and joins them, since it loses its way in a paragraph.

## How she is made

| Piece | Where | What |
|---|---|---|
| The avatar | `~/angelface/assets/goddess.glb` | MPFB's CC0 example avatar from met4citizen/TalkingHead: rigged (Mixamo-style), with the 52 ARKit blendshapes and Oculus visemes. |
| The model | `crates/face/src/model.rs`, `glb.rs` | Her head is cut from the avatar's body:<br>• a compact skinned head and neck, the neck band fading out through vertex alpha in its own blended group;<br>• the lips found in the skin's picture (a mouth box, then redness) as their own group;<br>• eyes rigid on the head joint, split white, iris and pupil by their picture's layout;<br>• lashes, brows, teeth and tongue skinned as authored. Clothes and hair are left out.<br>A light sculpt narrows her jaw and neck. The glb reader takes dense and sparse morph targets (Blender writes shape keys sparse). |
| Blendshapes | `Part::shaped` | Applied on the CPU. A part is sent again (`Host::upload_mesh`, written in place by `Core::upload_mesh`) only when the weights it answers to move. |
| The pose | `pose.rs` | Her head's yaw, pitch and roll are shared between chest, neck and head (15/35/50), as turns about the world's axes made into each joint's own rotation, whatever way the rig's bones point. |
| The mind | `mind.rs` | Everything above: a function of time and of the cockpit's cues (`Cue`), with its own small randomness. |
| The drawing | `draw.rs` | An `angel-gpu` scene of her head in a square picture:<br>• the skin in the mesh shader's `chrome` finish;<br>• lips with a rose base under 70% chrome;<br>• irises with `glow`.<br>Her regalia, when on: rings as thin emissive tori with a faint blended sheath, the halo disc's picture painted once in code, a glowing jewel, and an additive aura and rising motes as glass layers about her picture. |
| Speech tracks | `track.rs` | `a2f-bake`'s JSON, mapped onto her channels by name. Audio2Face's actor does not rest at zero (Claire sits frowned in silence), so a baked track of silence is taken off every line. |
| In the cockpit | `cockpit/src/ui/angelface.rs` | Her thread (as the Orrery's and the Heartglass's), 30 frames a second while she is here, and an idle wait while she is away.<br>`observe` runs once a loop pass after `App::advance`. It turns the composer's changes and the turn's start and end into cues, and does nothing while she is away.<br>Her voice plays through `pw-play` (`ffplay` as a fallback) as an owned `ServiceChild`. |
| Her voice | `~/angelface/voice/` | Drama 3 renders the bank on a GPU host (`goddess_bank.py`): three candidate timbres (seraph, oracle, sovereign), each fixed by an anchor take, and every line in seraph.<br>`qa_bank.py` transcribes each take with Whisper and checks it against its line.<br>`tighten.py` shortens Drama 3's long pauses.<br>`bake_bank.py` runs `a2f-bake --serve` over the bank and writes `index.json`. |
| Audio2Face | `~/angelface/a2f/` | NVIDIA's Audio2Face-3D SDK, built locally (CUDA 12.9, TensorRT 10.13) on the 3090.<br>`a2f-bake` runs its diffusion network (v3.0, Claire) and its own blendshape solver. Cold it takes about 0.33 s per second of audio; warm, under `--serve`, about 0.1 s. |

Ids:

- Layer group `0x4641_4345` ("FACE"), at z 60 over the text (59 to 61 with her regalia).
- Pictures from `LAYER_PICTURES + 0x4641_0000`.
- Meshes from `0x4641_0000`.

Her scene is drawn at her largest size, so she is only ever shown smaller, and smoothly.

## Looking at her

```
cargo run -p angel-face --example goddess_shot -- ~/angelface/assets/goddess.glb <outdir>
cargo run -p angel-face --example goddess_shot -- ~/angelface/assets/goddess.glb <outdir> \
    --track ~/angelface/voice/bank/a2f/demo_read.json --rest ~/angelface/voice/bank/a2f/rest.json --only none --frames 200
```

The first writes her stills, each a short script of cues played through her mind and drawn by the GPU core offscreen over a dark terminal: forming, waking, present, reading, thinking, joy, speaking. `--regalia` draws her regalia too.

The second writes a spoken line's frames. Mux them with the line's WAV to watch the lip sync.

## Not yet

- **Live speech.** She speaks only the bank's lines. Speaking an agent's reply needs Drama 3 kept loaded on a GPU host (`serve-tts`; about 0.5× realtime there, and its memory is shared with H3) and a small sidecar to send the text and bake the track.
- **Audio2Emotion** is gated on Hugging Face: a token is refused until its license is accepted on `nvidia/Audio2Emotion-v2.2`'s page. Her emotions are set per occasion instead (`bake_bank.py`).
- **Audio2Face's own gaze and saccades** (`eyes` in the track) are not used; her mind owns her eyes.
