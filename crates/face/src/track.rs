//! A speech track: Audio2Face's blendshape weights for a line she speaks,
//! as `a2f-bake` writes them (`{ "fps", "names", "frames": [[w..]..] }`,
//! names in its solver's order), read onto her channels and sampled at any
//! time between its frames.

use crate::model::Goddess;

#[derive(Clone, Debug)]
pub struct Track {
    /// The shapes its columns are, in its order.
    pub names: Vec<String>,
    pub fps: f32,
    /// Seconds.
    pub duration: f32,
    /// For each column of a frame, the channel it drives (`None`: a shape
    /// she does not have).
    columns: Vec<Option<usize>>,
    frames: Vec<Vec<f32>>,
    /// The columns that open her mouth, for how loud she is.
    jaw: Option<usize>,
    open: Vec<usize>,
}

/// How much of a track's weights reach her face, per shape family: the
/// solver's eyes stay hers (her gaze, her blinks win where they are
/// stronger), its mouth and brows are taken whole.
fn reach(name: &str) -> f32 {
    if name.starts_with("eyeLook") {
        0.0
    } else if name.starts_with("eyeBlink") {
        0.6
    } else {
        1.0
    }
}

impl Track {
    /// Read a track for `goddess` from its JSON text.
    pub fn from_json(text: &str, goddess: &Goddess) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("a speech track: {e}"))?;
        let fps = v["fps"]
            .as_f64()
            .filter(|f| f.is_finite() && *f > 0.0 && *f <= 240.0)
            .ok_or("a speech track with no frame rate")? as f32;
        let names: Vec<&str> = v["names"]
            .as_array()
            .ok_or("a speech track with no names")?
            .iter()
            .map(|n| n.as_str().unwrap_or(""))
            .collect();
        let frames: Vec<Vec<f32>> = v["frames"]
            .as_array()
            .ok_or("a speech track with no frames")?
            .iter()
            .map(|f| {
                let row: Vec<f32> = f.as_array().map_or_else(Vec::new, |r| {
                    r.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()
                });
                if row.len() != names.len() || row.iter().any(|x| !x.is_finite()) {
                    return Err("a speech track frame that does not match its names".to_string());
                }
                Ok(row
                    .iter()
                    .zip(&names)
                    .map(|(w, n)| w.clamp(0.0, 1.0) * reach(n))
                    .collect())
            })
            .collect::<Result<_, _>>()?;
        if frames.is_empty() {
            return Err("a speech track with no frames".into());
        }
        let columns = names.iter().map(|n| goddess.channel(n)).collect();
        let col = |n: &str| names.iter().position(|x| *x == n);
        let open = [
            "mouthFunnel",
            "mouthLowerDownLeft",
            "mouthLowerDownRight",
            "mouthUpperUpLeft",
            "mouthUpperUpRight",
        ]
        .iter()
        .filter_map(|n| col(n))
        .collect();
        let duration = v["duration"]
            .as_f64()
            .map_or(frames.len() as f32 / fps, |d| d as f32);
        let names = names.iter().map(|n| n.to_string()).collect();
        Ok(Self {
            names,
            fps,
            duration,
            columns,
            jaw: col("jawOpen"),
            open,
            frames,
        })
    }

    /// Take away a resting face: `rest`'s mean frame (a track of silence),
    /// shape by shape, from every frame of this one. Audio2Face's actor does
    /// not rest at zero (Claire's mouth sits frowned and rolled in
    /// silence); her own face is her rest.
    pub fn without_rest(mut self, rest: &Track) -> Self {
        let n = rest.frames.len().max(1) as f32;
        for (c, name) in self.names.iter().enumerate() {
            let Some(r) = rest.names.iter().position(|x| x == name) else {
                continue;
            };
            let mean = rest.frames.iter().map(|f| f[r]).sum::<f32>() / n;
            for f in &mut self.frames {
                f[c] = (f[c] - mean).max(0.0);
            }
        }
        self
    }

    /// Her weights `t` seconds into the line (a weight per channel, zero
    /// where the track says nothing), and how loud she is then, 0..1.
    /// `None` past the end.
    pub fn sample(&self, t: f32, channels: usize) -> Option<(Vec<f32>, f32)> {
        if !(t >= 0.0 && t < self.duration) {
            return None;
        }
        let x = t * self.fps;
        let i = (x.floor() as usize).min(self.frames.len() - 1);
        let j = (i + 1).min(self.frames.len() - 1);
        let k = x - i as f32;
        let (a, b) = (&self.frames[i], &self.frames[j]);
        let mut out = vec![0.0; channels];
        for (c, column) in self.columns.iter().enumerate() {
            if let Some(ch) = column
                && let Some(slot) = out.get_mut(*ch)
            {
                *slot = a[c] + (b[c] - a[c]) * k;
            }
        }
        let at = |c: usize| a[c] + (b[c] - a[c]) * k;
        let jaw = self.jaw.map_or(0.0, at);
        let open = self.open.iter().map(|&c| at(c)).sum::<f32>() / self.open.len().max(1) as f32;
        let loud = ((jaw * 2.2) + open * 1.2).clamp(0.0, 1.0);
        Some((out, loud))
    }
}
