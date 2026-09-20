# Excalibur startup

User-supplied source: `excalibur-tui-long-blade-5s (1).mp4`, 1360×768,
24 fps, 120 frames / five seconds.

Source SHA-256: `eea408f34983b524329f6ec133c1fb96d7fd311e0c2d8c8b0b720349d29566c4`.

`rise.png` is a 10-column × 6-row atlas of 60 frames at 680×384, 12 fps, stored
as white ink on a transparent background: the ink plane is uniformly white and the
alpha plane carries the frame luminance, so the saved source shows the same
white-on-transparent animation the terminal receives. It is embedded in the
ordinary cockpit and decoded off the input thread. The atlas is **never
displayed**: playback converts it into Dotmax Braille dots, using the same
physical dot pitch as the world, or ordinary Braille cells on terminals without
fine-dot transport. No runtime video decoder is needed.

Reproduce from the supplied video:

```sh
ffmpeg -i 'excalibur-tui-long-blade-5s (1).mp4' \
  -vf 'fps=12,scale=680:384:flags=lanczos,format=gray,tile=10x6' \
  -frames:v 1 /tmp/rise-gray.png
make-alpha-mask.sh /tmp/rise-gray.png
```

The ffmpeg step tiles the clip into the luminance atlas; `make-alpha-mask.sh`
rewrites it as the shipped mask and fails unless the alpha plane is byte-identical
to that luminance and the ink plane is uniformly white.

The sword rises once, then holds. While it holds, the water becomes ambient:
the lower third crossfades from the settled frame's water into a loop of the
late-clip shimmer — frames 50-59 at 12 fps, then four ticks dissolving the last
frame back into the first, so the cycle closes on a crossfade rather than the
three-times-normal step a hard wrap leaves. The loop may only touch the pixels the
settled frame already shows as water: the blade, the hand, the hilt and the solid
core of their reflection are closed to it, so the ambient flow can never jitter the
hand or the hilt. Under reduced motion the static water dots fade to black instead
of sitting frozen. First composer text or
paste fades the whole intro for 600 ms; submitting or restoring a
conversation removes it immediately.
Deleting the draft or starting a new conversation does not replay the intro.
Reduced motion holds the raised sword with a shorter fade; motion off holds it
and dismisses immediately. Comp mode and `ANGEL_BACKDROP=off` skip the intro.

The shell and mini-viz share one playback clock, fade and bounded encoding worker,
which crops, scales and levels the settled and late-clip canvases once per dot
geometry and re-uses them, so an ambient tick costs a copy and a band blend rather
than two Lanczos resamples of the atlas.
A fixed side crop frames the whole blade and waterline in both panes; Lanczos
sampling and fixed black/white levels retain the tip and steel shading. Nothing is
painted behind the ink: the fine-dot transport uploads only the dots, and the
Braille fallback leaves empty cells alone, so both intro panes keep the surface
behind the animation. Other world canvas colors remain unchanged.
