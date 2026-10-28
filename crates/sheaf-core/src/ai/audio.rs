//! Acrobat's Generate podcast, spoken: the two hosts' script read by the computer's own voices
//! (the engine Read aloud uses: eSpeak NG on Linux, `say` on macOS, SAPI on Windows), one voice
//! per host when the system has two, written as one WAV file with a short pause between turns.
//! Nothing is sent anywhere: the voices run on this computer.

use crate::{OpdfError, Result};
use std::path::{Path, PathBuf};

/// A turn of the script: who speaks (as written, "Alex") and what.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub speaker: String,
    pub text: String,
}

/// The turns of a script written as "Name: words" lines; a line without a name continues the
/// previous turn (or is the first host's when nothing came before). Markdown emphasis is dropped.
pub fn turns(script: &str) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::new();
    for line in script.lines().map(|l| l.trim().replace("**", "")).filter(|l| !l.is_empty()) {
        let named = line.split_once(':').filter(|(n, rest)| !n.is_empty() && n.len() <= 24 && n.split_whitespace().count() <= 3 && !rest.trim().is_empty() && n.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '.'));
        match named {
            Some((n, rest)) => out.push(Turn { speaker: n.trim().to_string(), text: rest.trim().to_string() }),
            None => match out.last_mut() {
                Some(t) => {
                    t.text.push(' ');
                    t.text.push_str(&line);
                }
                None => out.push(Turn { speaker: "Host".into(), text: line }),
            },
        }
    }
    out
}

/// Speaks `text` with `voice` (None: the system's default) into the WAV file `out`.
pub trait Synth {
    fn to_wav(&self, text: &str, voice: Option<&str>, out: &Path) -> Result<()>;
}

/// The computer's own voices (as Read aloud uses them).
pub struct SystemSynth;

fn run(cmd: &mut std::process::Command, input: &str, what: &str) -> Result<()> {
    use std::io::Write;
    let mut child = cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().map_err(|_| OpdfError::Unsupported(format!("{what} is not installed: it speaks the podcast")))?;
    if let Some(mut i) = child.stdin.take() {
        let _ = i.write_all(input.as_bytes());
    }
    let o = child.wait_with_output()?;
    if !o.status.success() {
        return Err(OpdfError::Engine(format!("{what}: {}", String::from_utf8_lossy(&o.stderr).trim())));
    }
    Ok(())
}

impl Synth for SystemSynth {
    fn to_wav(&self, text: &str, voice: Option<&str>, out: &Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let mut c = std::process::Command::new("say");
            c.args(["--file-format=WAVE", "--data-format=LEI16@22050", "-f", "-", "-o"]).arg(out);
            if let Some(v) = voice {
                c.args(["-v", v]);
            }
            run(&mut c, text, "say")
        }
        #[cfg(windows)]
        {
            let script = format!(
                "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; {}$f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(22050, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono); $s.SetOutputToWaveFile('{}', $f); $s.Speak([Console]::In.ReadToEnd()); $s.Dispose()",
                voice.map(|v| format!("$s.SelectVoice('{}'); ", v.replace('\'', "''"))).unwrap_or_default(),
                out.display().to_string().replace('\'', "''")
            );
            run(std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]), text, "Windows speech")
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let mut c = std::process::Command::new("espeak-ng");
            c.args(["--stdin", "-w"]).arg(out);
            if let Some(v) = voice {
                c.args(["-v", v]);
            }
            run(&mut c, text, "eSpeak NG")
        }
    }
}

/// Two voices for the two hosts. Linux: eSpeak NG's male and female variants of `base` (the
/// Read aloud voice, else English). macOS and Windows: two different installed voices of
/// `installed` (the first ones whose name says English when there are), else the default for both.
pub fn host_voices(base: Option<&str>, installed: &[String]) -> [Option<String>; 2] {
    if cfg!(any(target_os = "macos", windows)) {
        let english: Vec<&String> = installed.iter().filter(|v| v.to_lowercase().contains("en") || ["Alex", "Samantha", "Daniel", "Karen"].contains(&v.as_str())).collect();
        let pool: Vec<&String> = if english.len() >= 2 { english } else { installed.iter().collect() };
        return match pool.as_slice() {
            [a, b, ..] => [Some((*a).clone()), Some((*b).clone())],
            [a] => [Some((*a).clone()), Some((*a).clone())],
            [] => [None, None],
        };
    }
    let base = base.map(|b| b.split('+').next().unwrap_or(b)).filter(|b| !b.is_empty()).unwrap_or("en-us");
    [Some(format!("{base}+m3")), Some(format!("{base}+f3"))]
}

/// A WAV file's format chunk and samples.
struct Wav {
    fmt: Vec<u8>,
    data: Vec<u8>,
}

fn read_wav(bytes: &[u8]) -> Result<Wav> {
    let bad = || OpdfError::Engine("the voice wrote an unreadable WAV file".into());
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad());
    }
    let (mut fmt, mut data, mut at) = (None, None, 12);
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let mut len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().map_err(|_| bad())?) as usize;
        let start = at + 8;
        // eSpeak NG streams to a file and may leave the data size at its placeholder.
        if id == b"data" && (len == 0 || len == 0x7fff_ffff || start + len > bytes.len()) {
            len = bytes.len() - start;
        }
        let end = (start + len).min(bytes.len());
        match id {
            b"fmt " => fmt = Some(bytes[start..end].to_vec()),
            b"data" => data = Some(bytes[start..end].to_vec()),
            _ => {}
        }
        at = end + (len & 1);
    }
    Ok(Wav { fmt: fmt.ok_or_else(bad)?, data: data.ok_or_else(bad)? })
}

fn write_wav(fmt: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + fmt.len() + 28);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((4 + 8 + fmt.len() + 8 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    out.extend_from_slice(fmt);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// The script spoken into the WAV file `out`: each turn by its host's voice (the first speaker
/// named takes `voices[0]`, the second `voices[1]`, then they alternate by name), 0.4 s of
/// silence between turns. `progress(turn, turns)` false stops. Never overwrites `out`.
pub fn podcast_wav(script: &str, synth: &dyn Synth, voices: &[Option<String>; 2], out: &Path, progress: &mut dyn FnMut(usize, usize) -> bool) -> Result<PathBuf> {
    let turns = turns(script);
    if turns.is_empty() {
        return Err(OpdfError::Unsupported("the script has nothing to say".into()));
    }
    let dir = tempfile::tempdir()?;
    let mut speakers: Vec<String> = Vec::new();
    let (mut fmt, mut data): (Option<Vec<u8>>, Vec<u8>) = (None, Vec::new());
    for (i, t) in turns.iter().enumerate() {
        if !progress(i + 1, turns.len()) {
            return Err(OpdfError::Cancelled);
        }
        if !speakers.contains(&t.speaker) {
            speakers.push(t.speaker.clone());
        }
        let who = speakers.iter().position(|s| s == &t.speaker).unwrap_or(0) % 2;
        let part = dir.path().join(format!("{i}.wav"));
        synth.to_wav(&t.text, voices[who].as_deref(), &part)?;
        let w = read_wav(&std::fs::read(&part)?)?;
        match &fmt {
            None => fmt = Some(w.fmt.clone()),
            Some(f) if f[..f.len().min(16)] != w.fmt[..w.fmt.len().min(16)] => return Err(OpdfError::Engine("the two voices speak in different audio formats".into())),
            _ => {}
        }
        if !data.is_empty() {
            // Silence: 0.4 s at the format's byte rate (bytes 8..12 of fmt), block-aligned.
            let f = fmt.as_deref().unwrap_or_default();
            let rate = f.get(8..12).map_or(44_100, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) as usize;
            let align = f.get(12..14).map_or(2, |b| u16::from_le_bytes([b[0], b[1]]) as usize).max(1);
            data.extend(std::iter::repeat_n(0u8, rate * 2 / 5 / align * align));
        }
        data.extend_from_slice(&w.data);
    }
    let bytes = write_wav(&fmt.unwrap_or_default(), &data);
    crate::paths::atomic_write(out, |p| Ok(std::fs::write(p, &bytes)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_becomes_turns() {
        let t = turns("**Alex:** Welcome!\nSam: Thanks.\nIt is great.\n\nAlex: Bye. Time: 5 pm");
        assert_eq!(t.iter().map(|t| (t.speaker.as_str(), t.text.as_str())).collect::<Vec<_>>(), [("Alex", "Welcome!"), ("Sam", "Thanks. It is great."), ("Alex", "Bye. Time: 5 pm")]);
        assert_eq!(turns("Just words")[0].speaker, "Host");
    }

    #[test]
    fn linux_hosts_get_two_espeak_variants() {
        if cfg!(any(target_os = "macos", windows)) {
            assert_eq!(host_voices(None, &["Alex".into(), "Samantha".into()]), [Some("Alex".into()), Some("Samantha".into())]);
        } else {
            assert_eq!(host_voices(Some("fr"), &[]), [Some("fr+m3".into()), Some("fr+f3".into())]);
            assert_eq!(host_voices(None, &[]), [Some("en-us+m3".into()), Some("en-us+f3".into())]);
        }
    }
}
