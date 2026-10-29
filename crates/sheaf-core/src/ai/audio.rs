//! Acrobat's Generate podcast, spoken: the two hosts' script read by the computer's own voices,
//! the most natural ones it has, written as one WAV file with a short pause between turns.
//! macOS: `say`'s voices (Premium and Enhanced first). Windows: the OneCore voices
//! (Windows.Media.SpeechSynthesis, Natural ones first), then the desktop SAPI voices. Linux: Piper
//! when it is installed with a voice model, else eSpeak NG's male and female variants. The voices
//! are those of the script's language; the two hosts get two different voices (of different sexes
//! when known), each changeable in the podcast's options. Nothing is sent anywhere.

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

/// What speaks a voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// macOS's `say`.
    Say,
    /// Windows's desktop voices (System.Speech).
    Sapi,
    /// Windows's OneCore voices (Windows.Media.SpeechSynthesis).
    OneCore,
    /// Piper's neural voices (a `.onnx` model).
    Piper,
    /// eSpeak NG.
    Espeak,
}

/// A voice of this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub engine: Engine,
    /// What the engine is given: `say -v`'s name, the Windows voice's name, a Piper model's path
    /// (with `#speaker` for a model of several speakers), eSpeak's `fr+m3`.
    pub id: String,
    /// As listed in the podcast options: "Ava (Premium) · en_US".
    pub label: String,
    /// Its language: "en_US", "fr-FR", "fr".
    pub lang: String,
    /// Female, male, or unknown.
    pub female: Option<bool>,
    /// 0 a novelty voice (Bubbles, Zarvox), 1 robotic (eSpeak, macOS's Eloquence voices), 2 a
    /// standard voice, 3 Enhanced / OneCore / a medium Piper model, 4 Premium / Natural / high.
    pub quality: u8,
}

/// macOS voices made for fun, never chosen for a host.
const NOVELTY: &[&str] = &["Albert", "Bad News", "Bahh", "Bells", "Boing", "Bubbles", "Cellos", "Deranged", "Good News", "Hysterical", "Jester", "Organ", "Pipe Organ", "Superstar", "Trinoids", "Whisper", "Wobble", "Zarvox", "Junior", "Ralph", "Fred", "Kathy"];
/// macOS's Eloquence voices (robotic, one per language).
const ELOQUENCE: &[&str] = &["Eddy", "Flo", "Grandma", "Grandpa", "Reed", "Rocko", "Sandy", "Shelley"];
/// The sex of macOS's, Windows's and Piper's usual voices, by first name (true: female).
const SEXES: &[(&str, bool)] = &[
    ("Allison", true), ("Ava", true), ("Samantha", true), ("Susan", true), ("Zoe", true), ("Nicky", true), ("Joelle", true), ("Victoria", true), ("Karen", true), ("Kate", true),
    ("Serena", true), ("Moira", true), ("Fiona", true), ("Tessa", true), ("Veena", true), ("Isha", true), ("Amélie", true), ("Audrey", true), ("Aurélie", true), ("Marie", true),
    ("Anna", true), ("Petra", true), ("Helena", true), ("Kyoko", true), ("Monica", true), ("Paulina", true), ("Marisol", true), ("Alice", true), ("Federica", true), ("Milena", true),
    ("Katya", true), ("Ting-Ting", true), ("Tingting", true), ("Sinji", true), ("Mei-Jia", true), ("Meijia", true), ("Yuna", true), ("Joana", true), ("Luciana", true), ("Ellen", true),
    ("Claire", true), ("Sara", true), ("Nora", true), ("Zuzana", true), ("Yelda", true), ("Laila", true), ("Lekha", true), ("Kanya", true), ("Zira", true), ("Hazel", true),
    ("Hortense", true), ("Julie", true), ("Hedda", true), ("Haruka", true), ("Ayumi", true), ("Irina", true), ("Aria", true), ("Jenny", true), ("Sonia", true), ("Denise", true),
    ("Lessac", true), ("Amy", true), ("Kathleen", true), ("Siwis", true), ("Jessica", true),
    ("Alex", false), ("Tom", false), ("Evan", false), ("Nathan", false), ("Aaron", false), ("Daniel", false), ("Oliver", false), ("Lee", false), ("Rishi", false), ("Thomas", false),
    ("Nicolas", false), ("Markus", false), ("David", false), ("Mark", false), ("George", false), ("Ryan", false), ("Joe", false), ("Pierre", false),
];

/// The sex a voice's name says, when known ("Microsoft Zira Desktop", "Ava (Premium)").
fn sex_of(name: &str) -> Option<bool> {
    name.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '_').filter(|w| !w.is_empty()).find_map(|w| SEXES.iter().find(|(n, _)| n.eq_ignore_ascii_case(w)).map(|(_, f)| *f))
}

/// The voices of a `say -v ?` listing: "Ava (Premium)      en_US    # Hello! My name is Ava.".
pub fn parse_say(listing: &str) -> Vec<Voice> {
    listing
        .lines()
        .filter_map(|l| {
            let head = l.split_once('#').map_or(l, |(h, _)| h).trim_end();
            let (name, lang) = head.rsplit_once(char::is_whitespace)?;
            let name = name.trim();
            if name.is_empty() || !lang.contains('_') {
                return None;
            }
            let base = name.split(" (").next().unwrap_or(name);
            let quality = if NOVELTY.contains(&base) {
                0
            } else if name.contains("(Premium)") {
                4
            } else if name.contains("(Enhanced)") {
                3
            } else if ELOQUENCE.contains(&base) {
                1
            } else {
                2
            };
            Some(Voice { engine: Engine::Say, id: name.to_string(), label: format!("{name} · {lang}"), lang: lang.to_string(), female: sex_of(base), quality })
        })
        .collect()
}

/// The PowerShell listing Windows's voices for [`parse_windows`]: one line per voice, "engine,
/// name, language, gender" separated by tabs, OneCore's (WinRT) then the desktop's (SAPI).
pub const WINDOWS_LISTING: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
try { $null = [Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]; \
[Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices | ForEach-Object { \"onecore`t$($_.DisplayName)`t$($_.Language)`t$($_.Gender)\" } } catch {} \
try { Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | Where-Object { $_.Enabled } | \
ForEach-Object { \"sapi`t$($_.VoiceInfo.Name)`t$($_.VoiceInfo.Culture.Name)`t$($_.VoiceInfo.Gender)\" } } catch {}";

/// The voices of [`WINDOWS_LISTING`]'s output. A voice both engines have ("Microsoft David" and
/// "Microsoft David Desktop") is kept once, as OneCore's.
pub fn parse_windows(listing: &str) -> Vec<Voice> {
    let mut out: Vec<Voice> = Vec::new();
    for l in listing.lines() {
        let f: Vec<&str> = l.trim().split('\t').collect();
        let [engine, name, lang, gender] = f[..] else { continue };
        let engine = match engine {
            "onecore" => Engine::OneCore,
            "sapi" => Engine::Sapi,
            _ => continue,
        };
        let same = name.trim_end_matches(" Desktop");
        if out.iter().any(|v| v.id.trim_end_matches(" Desktop") == same) {
            continue;
        }
        let natural = ["Natural", "Neural"].iter().any(|w| name.contains(w));
        let quality = if natural {
            4
        } else if engine == Engine::OneCore {
            3
        } else {
            2
        };
        let female = match gender {
            "Female" => Some(true),
            "Male" => Some(false),
            _ => sex_of(name),
        };
        out.push(Voice { engine, id: name.to_string(), label: format!("{} · {lang}", name.trim_start_matches("Microsoft ")), lang: lang.to_string(), female, quality });
    }
    out
}

/// eSpeak NG's voices from its `--voices` table (the language column, after a header): a male
/// and a female variant of each language.
pub fn parse_espeak(listing: &str) -> Vec<Voice> {
    listing
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().nth(1))
        .flat_map(|lang| [(false, "m3", "male"), (true, "f3", "female")].map(|(female, v, sex)| Voice { engine: Engine::Espeak, id: format!("{lang}+{v}"), label: format!("eSpeak {lang} · {sex}"), lang: lang.to_string(), female: Some(female), quality: 1 }))
        .collect()
}

/// The Piper voices in `dirs` (searched a few folders deep): each `.onnx` model with its
/// `.onnx.json` beside it, one voice per speaker (the first four of a model of several).
pub fn piper_voices(dirs: &[PathBuf]) -> Vec<Voice> {
    fn models(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() && depth < 4 {
                models(&p, depth + 1, out);
            } else if p.extension().is_some_and(|e| e == "onnx") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in dirs {
        models(d, 0, &mut files);
    }
    let mut out = Vec::new();
    for model in files {
        let Ok(cfg) = std::fs::read(format!("{}.json", model.display())) else { continue };
        let Ok(cfg) = serde_json::from_slice::<serde_json::Value>(&cfg) else { continue };
        let stem = model.file_stem().unwrap_or_default().to_string_lossy().to_string();
        // "en_US-lessac-medium": language, name, quality.
        let mut parts = stem.splitn(3, '-');
        let (lang_f, name, q) = (parts.next().unwrap_or_default(), parts.next().unwrap_or(&stem), parts.next().unwrap_or_default());
        let lang = cfg["language"]["code"].as_str().unwrap_or(lang_f).to_string();
        let q = cfg["audio"]["quality"].as_str().unwrap_or(q).to_string();
        let quality = match q.as_str() {
            "high" => 4,
            "medium" => 3,
            _ => 2,
        };
        let mut speakers: Vec<(String, u64)> = cfg["speaker_id_map"].as_object().map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_u64()?))).collect()).unwrap_or_default();
        speakers.sort_by_key(|(_, n)| *n);
        if speakers.len() < 2 {
            out.push(Voice { engine: Engine::Piper, id: model.display().to_string(), label: format!("Piper {name} · {lang} · {q}"), lang, female: sex_of(name), quality });
            continue;
        }
        for (who, n) in speakers.into_iter().take(4) {
            out.push(Voice { engine: Engine::Piper, id: format!("{}#{n}", model.display()), label: format!("Piper {name} {who} · {lang} · {q}"), lang: lang.clone(), female: sex_of(&who), quality });
        }
    }
    out
}

/// Where Piper's voice models are looked for: `$PIPER_VOICES` (folders separated as in PATH),
/// then the usual install folders.
fn piper_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PIPER_VOICES").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default();
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(d) = data {
        dirs.extend(["piper", "piper-voices", "piper-tts"].map(|n| d.join(n)));
    }
    dirs.extend(["/usr/share/piper-voices", "/usr/share/piper/voices", "/usr/local/share/piper-voices", "/usr/local/share/piper/voices"].map(PathBuf::from));
    dirs.retain(|d| d.is_dir());
    dirs
}

/// Piper's command when installed (`piper`, or `piper-tts` as some distributions name it).
fn piper_command() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).flat_map(|d| ["piper", "piper-tts"].map(|n| d.join(n))).find(|p| p.is_file())
}

/// The voices of this computer that can speak a podcast. Debug builds under gui-smoke
/// (`SHEAF_TEST_SPEECH` set) list eSpeak NG's only, so the result never depends on the machine.
pub fn system_voices() -> Vec<Voice> {
    let out = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    if cfg!(target_os = "macos") {
        return parse_say(&out("say", &["-v", "?"]));
    }
    if cfg!(windows) {
        return parse_windows(&out("powershell", &["-NoProfile", "-Command", WINDOWS_LISTING]));
    }
    let hermetic = cfg!(debug_assertions) && std::env::var_os("SHEAF_TEST_SPEECH").is_some();
    let mut v = if hermetic || piper_command().is_none() { Vec::new() } else { piper_voices(&piper_dirs()) };
    v.extend(parse_espeak(&out("espeak-ng", &["--voices"])));
    v
}

/// The main language of `text` as an ISO 639-1 code: by its script (Japanese has kana, Chinese
/// only Han, Russian Cyrillic…), and for Latin text by its commonest small words (English when
/// none is found).
pub fn guess_language(text: &str) -> &'static str {
    let mut counts = [0usize; 10];
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        let k = match c as u32 {
            0x3040..=0x30FF => 0,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => 2,
            0x0400..=0x04FF => 3,
            0x0600..=0x06FF | 0x0750..=0x077F => 4,
            0x0370..=0x03FF => 5,
            0x0590..=0x05FF => 6,
            0x0E00..=0x0E7F => 7,
            0x0900..=0x097F => 8,
            _ => 9,
        };
        counts[k] += 1;
    }
    // Japanese mixes kana and Han: a real share of kana makes it Japanese.
    if counts[0] > 0 && counts[0] * 10 >= counts[0] + counts[1] {
        return "ja";
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(i, n)| (**n, usize::MAX - i)).unwrap_or((9, &0));
    if *n > 0 && best != 9 {
        return ["ja", "zh", "ko", "ru", "ar", "el", "he", "th", "hi"][best];
    }
    const WORDS: [(&str, &[&str]); 10] = [
        ("en", &["the", "and", "is", "of", "to", "that", "this", "it", "you", "with"]),
        ("fr", &["le", "la", "les", "et", "est", "des", "une", "que", "pas", "vous", "nous", "dans"]),
        ("es", &["el", "los", "las", "y", "es", "una", "del", "por", "con", "pero", "muy"]),
        ("de", &["der", "die", "das", "und", "ist", "nicht", "ein", "eine", "mit", "auch", "wir"]),
        ("it", &["il", "che", "è", "della", "gli", "una", "non", "per", "sono", "molto"]),
        ("pt", &["os", "não", "uma", "é", "do", "da", "em", "com", "você", "muito"]),
        ("nl", &["het", "een", "en", "van", "niet", "ook", "maar", "wij", "zijn"]),
        ("pl", &["i", "w", "nie", "się", "jest", "na", "że", "jak", "to"]),
        ("tr", &["ve", "bir", "bu", "için", "ile", "değil", "çok", "da"]),
        ("sv", &["och", "att", "det", "är", "som", "inte", "jag", "med"]),
    ];
    let words: Vec<String> = text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    WORDS.iter().map(|(lang, list)| (*lang, words.iter().filter(|w| list.contains(&w.as_str())).count())).max_by_key(|(lang, n)| (*n, *lang == "en")).filter(|(_, n)| *n > 0).map_or("en", |(l, _)| l)
}

/// A voice language's main part, lower case ("en_US" → "en"; eSpeak's Mandarin "cmn" → "zh").
fn primary(lang: &str) -> String {
    let p = lang.split(['_', '-']).next().unwrap_or(lang).to_ascii_lowercase();
    match p.as_str() {
        "cmn" | "yue" => "zh".into(),
        _ => p,
    }
}

/// Whether a voice's language is the main variant of its language (fr_FR, en_US, plain "fr"),
/// preferred over regional ones when they are as good.
fn main_variant(lang: &str) -> bool {
    let p = primary(lang);
    let region = lang.split(['_', '-']).nth(1).unwrap_or_default().to_ascii_lowercase();
    region.is_empty() || region == p || matches!((p.as_str(), region.as_str()), ("en", "us") | ("zh", "cn") | ("ja", "jp") | ("ko", "kr") | ("sv", "se") | ("da", "dk") | ("el", "gr") | ("he", "il") | ("cs", "cz") | ("uk", "ua") | ("hi", "in") | ("ar", "sa"))
}

/// The voices for a podcast in language `lang` (ISO 639-1), best first: those of that language
/// (else English ones, else all), the most natural first, novelty voices only when nothing else.
pub fn rank_voices(voices: &[Voice], lang: &str) -> Vec<Voice> {
    let of = |l: &str| voices.iter().filter(|v| primary(&v.lang) == l).cloned().collect::<Vec<_>>();
    let mut pool = of(lang);
    if pool.is_empty() {
        pool = of("en");
    }
    if pool.is_empty() {
        pool = voices.to_vec();
    }
    if pool.iter().any(|v| v.quality > 0) {
        pool.retain(|v| v.quality > 0);
    }
    pool.sort_by_key(|v| (std::cmp::Reverse(v.quality), !main_variant(&v.lang)));
    pool
}

/// The two hosts' voices among `ranked` ([`rank_voices`]): the best voice, then the best other
/// voice, preferring the other sex when both are known, and the same engine; the same voice
/// twice when there is only one; none when there is none (the system's default).
pub fn host_voices(ranked: &[Voice]) -> [Option<Voice>; 2] {
    let Some(a) = ranked.first() else { return [None, None] };
    let score = |v: &Voice| v.quality as i32 * 4 + if matches!((a.female, v.female), (Some(x), Some(y)) if x != y) { 2 } else { 0 } + i32::from(v.engine == a.engine) + i32::from(main_variant(&v.lang));
    let mut b: Option<&Voice> = None;
    for v in ranked.iter().skip(1).filter(|v| v.id != a.id) {
        if b.is_none_or(|b| score(v) > score(b)) {
            b = Some(v);
        }
    }
    [Some(a.clone()), Some(b.unwrap_or(a).clone())]
}

/// Speaks `text` with `voice` (None: the system's default) into the WAV file `out`.
pub trait Synth {
    fn to_wav(&self, text: &str, voice: Option<&Voice>, out: &Path) -> Result<()>;
}

/// The computer's own voices.
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

/// PowerShell quoting: inside single quotes, a quote doubled.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

impl Synth for SystemSynth {
    fn to_wav(&self, text: &str, voice: Option<&Voice>, out: &Path) -> Result<()> {
        let default = if cfg!(target_os = "macos") {
            Engine::Say
        } else if cfg!(windows) {
            Engine::Sapi
        } else {
            Engine::Espeak
        };
        let id = voice.map(|v| v.id.as_str());
        let utf8 = "[Console]::InputEncoding = [Text.Encoding]::UTF8; ";
        match voice.map_or(default, |v| v.engine) {
            Engine::Say => {
                let mut c = std::process::Command::new("say");
                c.args(["--file-format=WAVE", "--data-format=LEI16@22050", "-f", "-", "-o"]).arg(out);
                if let Some(v) = id {
                    c.args(["-v", v]);
                }
                run(&mut c, text, "say")
            }
            Engine::Sapi => {
                let script = format!(
                    "{utf8}Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; {}$f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(22050, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono); $s.SetOutputToWaveFile({}, $f); $s.Speak([Console]::In.ReadToEnd()); $s.Dispose()",
                    id.map(|v| format!("$s.SelectVoice({}); ", ps_quote(v))).unwrap_or_default(),
                    ps_quote(&out.display().to_string())
                );
                run(std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]), text, "Windows speech")
            }
            Engine::OneCore => {
                // WinRT from Windows PowerShell: the asynchronous synthesis awaited through AsTask.
                let script = format!(
                    "{utf8}Add-Type -AssemblyName System.Runtime.WindowsRuntime; \
                     $null = [Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]; \
                     $s = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer; \
                     $v = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices | Where-Object {{ $_.DisplayName -eq {} }} | Select-Object -First 1; if ($v) {{ $s.Voice = $v }}; \
                     $as = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }} | Select-Object -First 1; \
                     $t = $as.MakeGenericMethod([Windows.Media.SpeechSynthesis.SpeechSynthesisStream]).Invoke($null, @($s.SynthesizeTextToStreamAsync([Console]::In.ReadToEnd()))); \
                     $r = [System.IO.WindowsRuntimeStreamExtensions]::AsStreamForRead($t.Result); $f = [System.IO.File]::Create({}); $r.CopyTo($f); $f.Close(); $r.Close()",
                    ps_quote(id.unwrap_or_default()),
                    ps_quote(&out.display().to_string())
                );
                run(std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]), text, "Windows speech")
            }
            Engine::Piper => {
                let bin = piper_command().ok_or_else(|| OpdfError::Unsupported("Piper is not installed: it speaks the podcast".into()))?;
                let (model, speaker) = match id.and_then(|v| v.rsplit_once('#')) {
                    Some((m, n)) => (m, Some(n)),
                    None => (id.unwrap_or_default(), None),
                };
                let mut c = std::process::Command::new(bin);
                c.args(["-m", model, "-f"]).arg(out);
                if let Some(n) = speaker {
                    c.args(["-s", n]);
                }
                // One line: Piper speaks each line of its input as a sentence of its own.
                run(&mut c, &text.replace('\n', " "), "Piper")
            }
            Engine::Espeak => {
                let mut c = std::process::Command::new("espeak-ng");
                c.args(["--stdin", "-w"]).arg(out);
                if let Some(v) = id {
                    c.args(["-v", v]);
                }
                run(&mut c, text, "eSpeak NG")
            }
        }
    }
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


/// 16-bit PCM samples `data` of format `from` in format `to` (fmt chunks): channels mixed down or
/// repeated, the rate changed by linear interpolation. None for other sample formats.
fn convert(data: &[u8], from: &[u8], to: &[u8]) -> Option<Vec<u8>> {
    let info = |f: &[u8]| -> Option<(usize, u32)> {
        let u16_at = |i: usize| f.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let rate = f.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))?;
        let (tag, channels, bits) = (u16_at(0)?, u16_at(2)?, u16_at(14)?);
        (matches!(tag, 1 | 0xFFFE) && bits == 16 && channels > 0 && rate > 0).then_some((channels as usize, rate))
    };
    let ((fc, fr), (tc, tr)) = (info(from)?, info(to)?);
    let frames: Vec<f32> = data.chunks_exact(2 * fc).map(|f| f.as_chunks::<2>().0.iter().map(|s| f32::from(i16::from_le_bytes(*s))).sum::<f32>() / fc as f32).collect();
    let n = (frames.len() as u64 * u64::from(tr) / u64::from(fr)) as usize;
    let at = |k: usize| frames.get(k.min(frames.len().saturating_sub(1))).copied().unwrap_or(0.0);
    let mut out = Vec::with_capacity(n * 2 * tc);
    for i in 0..n {
        let pos = i as f64 * f64::from(fr) / f64::from(tr);
        let k = pos.floor() as usize;
        let s = at(k) + (at(k + 1) - at(k)) * (pos - k as f64) as f32;
        let s = s.round().clamp(-32768.0, 32767.0) as i16;
        for _ in 0..tc {
            out.extend_from_slice(&s.to_le_bytes());
        }
    }
    Some(out)
}

/// The script spoken into the WAV file `out`: each turn by its host's voice (the first speaker
/// named takes `voices[0]`, the second `voices[1]`, then they alternate by name), 0.4 s of
/// silence between turns. Voices that speak in different formats (Piper's 16 kHz beside
/// eSpeak's 22 kHz) are brought to the first turn's. `progress(turn, turns)` false stops. Never
/// overwrites `out`.
pub fn podcast_wav(script: &str, synth: &dyn Synth, voices: &[Option<Voice>; 2], out: &Path, progress: &mut dyn FnMut(usize, usize) -> bool) -> Result<PathBuf> {
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
        synth.to_wav(&t.text, voices[who].as_ref(), &part)?;
        let mut w = read_wav(&std::fs::read(&part)?)?;
        match &fmt {
            None => fmt = Some(w.fmt.clone()),
            Some(f) if f[..f.len().min(16)] != w.fmt[..w.fmt.len().min(16)] => {
                w.data = convert(&w.data, &w.fmt, f).ok_or_else(|| OpdfError::Engine("the two voices speak in audio formats that cannot be joined".into()))?;
            }
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

    /// `say -v ?` on macOS 14 with Ava Premium and Evan Enhanced downloaded.
    const SAY: &str = "Albert              en_US    # Hello! My name is Albert.
Alice               it_IT    # Ciao! Mi chiamo Alice.
Amélie              fr_CA    # Bonjour! Je m’appelle Amélie.
Ava (Premium)       en_US    # Hello! My name is Ava.
Bad News            en_US    # Hello! My name is Bad News.
Daniel              en_GB    # Hello! My name is Daniel.
Eddy (English (US)) en_US    # Hello! My name is Eddy.
Eddy (French (France)) fr_FR    # Bonjour ! Je m’appelle Eddy.
Evan (Enhanced)     en_US    # Hello! My name is Evan.
Fred                en_US    # Hello! My name is Fred.
Kyoko               ja_JP    # こんにちは! 私の名前はKyokoです。
Samantha            en_US    # Hello! My name is Samantha.
Thomas              fr_FR    # Bonjour, je m’appelle Thomas.
Zoe (Premium)       en_US    # Hello! My name is Zoe.
";

    #[test]
    fn macos_hosts_get_the_premium_and_enhanced_voices_of_two_sexes() {
        let v = parse_say(SAY);
        assert_eq!(v.len(), 14);
        let ava = v.iter().find(|v| v.id == "Ava (Premium)").unwrap();
        assert_eq!((ava.lang.as_str(), ava.quality, ava.female), ("en_US", 4, Some(true)));
        assert_eq!(v.iter().find(|v| v.id == "Eddy (French (France))").map(|v| (v.lang.as_str(), v.quality)), Some(("fr_FR", 1)));
        // English: Ava (Premium) first; Zoe is Premium too but a woman, Evan (Enhanced) a man.
        let en = rank_voices(&v, "en");
        assert_eq!(en.iter().map(|v| v.id.as_str()).take(3).collect::<Vec<_>>(), ["Ava (Premium)", "Zoe (Premium)", "Evan (Enhanced)"]);
        assert!(!en.iter().any(|v| v.id == "Bad News" || v.id == "Albert"), "novelty voices are never hosts");
        let [a, b] = host_voices(&en);
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Ava (Premium)".to_string(), "Zoe (Premium)".to_string()));
        // French: Thomas (fr_FR) before Amélie (fr_CA), the robotic Eddy last.
        let fr = rank_voices(&v, "fr");
        assert_eq!(fr.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["Thomas", "Amélie", "Eddy (French (France))"]);
        let [a, b] = host_voices(&fr);
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Thomas".to_string(), "Amélie".to_string()));
        // One Japanese voice: both hosts speak with it; a language with none falls back to English.
        let [a, b] = host_voices(&rank_voices(&v, "ja"));
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Kyoko".to_string(), "Kyoko".to_string()));
        assert_eq!(rank_voices(&v, "ko")[0].id, "Ava (Premium)");
        assert_eq!(host_voices(&[]), [None, None]);
    }

    /// Two voices of the same quality: the second host is of the other sex.
    #[test]
    fn the_second_host_is_of_the_other_sex_when_as_good() {
        let v = parse_say("Samantha            en_US    # Hello!\nSusan               en_US    # Hello!\nDaniel              en_GB    # Hello!\n");
        let [a, b] = host_voices(&rank_voices(&v, "en"));
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Samantha".to_string(), "Daniel".to_string()));
    }

    /// [`WINDOWS_LISTING`]'s output on Windows 11: OneCore voices, a Natural one, the desktop's.
    #[test]
    fn windows_hosts_get_onecore_and_natural_voices_first() {
        let listing = "onecore\tMicrosoft David\ten-US\tMale\r\nonecore\tMicrosoft Zira\ten-US\tFemale\r\nonecore\tMicrosoft Mark\ten-US\tMale\r\nonecore\tMicrosoft Hortense\tfr-FR\tFemale\r\n\
onecore\tMicrosoft Aria Online (Natural)\ten-US\tFemale\r\nsapi\tMicrosoft David Desktop\ten-US\tMale\r\nsapi\tMicrosoft Hazel Desktop\ten-GB\tFemale\r\nsapi\tMicrosoft Paul Desktop\tfr-FR\tMale\r\n";
        let v = parse_windows(listing);
        assert_eq!(v.len(), 7, "David listed once: {v:?}");
        assert_eq!(v.iter().find(|v| v.id == "Microsoft David").map(|v| v.engine), Some(Engine::OneCore));
        assert_eq!(v.iter().find(|v| v.id == "Microsoft Paul Desktop").map(|v| (v.engine, v.quality, v.label.as_str())), Some((Engine::Sapi, 2, "Paul Desktop · fr-FR")));
        let en = rank_voices(&v, "en");
        assert_eq!(en[0].id, "Microsoft Aria Online (Natural)");
        let [a, b] = host_voices(&en);
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Microsoft Aria Online (Natural)".to_string(), "Microsoft David".to_string()));
        // French: Hortense (OneCore) before Paul (desktop), of the other sex.
        let [a, b] = host_voices(&rank_voices(&v, "fr"));
        assert_eq!((a.unwrap().id, b.unwrap().id), ("Microsoft Hortense".to_string(), "Microsoft Paul Desktop".to_string()));
    }

    /// Linux: Piper's models (one voice per speaker of a multi-speaker model) before eSpeak NG.
    #[test]
    fn linux_hosts_get_piper_voices_before_espeak() {
        let d = tempfile::tempdir().unwrap();
        let v = d.path().join("en/en_US");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::write(v.join("en_US-lessac-medium.onnx"), b"").unwrap();
        std::fs::write(v.join("en_US-lessac-medium.onnx.json"), r#"{"language": {"code": "en_US"}, "audio": {"quality": "medium", "sample_rate": 22050}, "num_speakers": 1}"#).unwrap();
        std::fs::write(v.join("fr_FR-upmc-medium.onnx"), b"").unwrap();
        std::fs::write(v.join("fr_FR-upmc-medium.onnx.json"), r#"{"language": {"code": "fr_FR"}, "audio": {"quality": "medium"}, "speaker_id_map": {"pierre": 1, "jessica": 0}}"#).unwrap();
        // A model without its .json is not a voice.
        std::fs::write(v.join("de_DE-thorsten-high.onnx"), b"").unwrap();
        let mut voices = piper_voices(&[d.path().to_path_buf()]);
        assert_eq!(voices.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(), ["Piper lessac · en_US · medium", "Piper upmc jessica · fr_FR · medium", "Piper upmc pierre · fr_FR · medium"]);
        voices.extend(parse_espeak("Pty Language       Age/Gender VoiceName          File                 Other Languages\n 5  en-gb           --/M      English_(Great_Britain) gmw/en\n 2  en-us           --/M      English_(America)  gmw/en-US\n 5  fr              --/M      French             roa/fr\n"));
        let [a, b] = host_voices(&rank_voices(&voices, "fr"));
        let (a, b) = (a.unwrap(), b.unwrap());
        assert!(a.id.ends_with("fr_FR-upmc-medium.onnx#0") && b.id.ends_with("fr_FR-upmc-medium.onnx#1"), "{a:?} {b:?}");
        // English: one Piper voice (lessac, a woman); the second host is eSpeak's US English
        // variant of the other sex.
        let [a, b] = host_voices(&rank_voices(&voices, "en"));
        assert_eq!((a.unwrap().label, b.unwrap().id), ("Piper lessac · en_US · medium".to_string(), "en-us+m3".to_string()));
        // Without Piper: eSpeak NG's male and female variants, as before.
        let espeak = parse_espeak("Pty Language       Age/Gender VoiceName          File                 Other Languages\n 5  fr              --/M      French             roa/fr\n");
        let [a, b] = host_voices(&rank_voices(&espeak, "fr"));
        assert_eq!((a.unwrap().id, b.unwrap().id), ("fr+m3".to_string(), "fr+f3".to_string()));
    }

    #[test]
    fn the_language_of_a_script_is_guessed() {
        assert_eq!(guess_language("Alex: Welcome to the show. Sam: This is about the report."), "en");
        assert_eq!(guess_language("Alex : Bienvenue dans l'émission. Sam : Nous parlons des résultats et de la suite."), "fr");
        assert_eq!(guess_language("Alex: Willkommen. Sam: Das ist nicht der Bericht, und wir sind hier."), "de");
        assert_eq!(guess_language("Alex: ようこそ。今日は年次報告について話します。"), "ja");
        assert_eq!(guess_language("Alex: 欢迎收听。今天我们讨论年度报告。"), "zh");
        assert_eq!(guess_language("Алекс: Добро пожаловать."), "ru");
        assert_eq!(guess_language("12 34"), "en");
    }

    #[test]
    fn voices_of_two_formats_are_joined_in_the_first() {
        // 16 kHz stereo to 8 kHz mono: every other frame, both channels averaged.
        let fmt = |ch: u16, rate: u32| [1u16.to_le_bytes(), ch.to_le_bytes()].concat().into_iter().chain(rate.to_le_bytes()).chain((rate * 2 * ch as u32).to_le_bytes()).chain((2 * ch).to_le_bytes()).chain(16u16.to_le_bytes()).collect::<Vec<u8>>();
        let data: Vec<u8> = [100i16, 300, 200, 400, 500, 700, 600, 800].iter().flat_map(|s| s.to_le_bytes()).collect();
        let out = convert(&data, &fmt(2, 16000), &fmt(1, 8000)).unwrap();
        assert_eq!(out.chunks(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect::<Vec<_>>(), [200, 600]);
        // 8-bit audio is not joined.
        let mut eight = fmt(1, 8000);
        eight[14] = 8;
        assert!(convert(&data, &eight, &fmt(1, 8000)).is_none());
    }
}
