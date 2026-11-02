//! `tribal-grab` : pilotage du moteur en ligne de commande (utilisable depuis Raccourcis).
use grab_core::*;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

const USAGE: &str = "Tribal Grab : téléchargeur de vidéos et d'audio

Utilisation :
  tribal-grab get <url> [options]      télécharge une adresse
  tribal-grab text <texte> [options]   extrait les adresses d'un texte et les télécharge
  tribal-grab queue start              lance la file d'attente enregistrée
  tribal-grab queue list               affiche la file
  tribal-grab probe <url>              analyse une adresse (JSON)
  tribal-grab search <requête>         recherche (moteurs des réglages)
  tribal-grab sites [filtre]           sites pris en charge par yt-dlp
  tribal-grab history [filtre]         historique
  tribal-grab tools                    état de yt-dlp, ffmpeg, ffprobe, deno, node
  tribal-grab update-ytdlp             met à jour yt-dlp (publication officielle, SHA256)

Options de téléchargement :
  --dir <dossier>      dossier de destination
  --audio <m4a|mp3|original>   audio seul
  --max-res <hauteur>  résolution maximale (ex. 720)
  --subs <fr,en>       télécharge les sous-titres (langues par ordre de préférence)
  --subs-only          sous-titres seuls
  --force              retélécharge même si le fichier est dans l'historique
  --name <modèle>      modèle de nom, ex. \"{site} - {title}\"
  --data-dir <dossier> dossier de données (par défaut ~/Library/Application Support/Tribal Grab)
  --quiet              pas de barre de progression
  --verbose            affiche les messages du moteur (repli, nouvelles tentatives)";

struct Cli {
    positional: Vec<String>,
    dir: Option<String>,
    audio: Option<AudioFormat>,
    max_res: Option<u32>,
    subs: Option<Vec<String>>,
    subs_only: bool,
    force: bool,
    name: Option<String>,
    data_dir: Option<PathBuf>,
    quiet: bool,
    verbose: bool,
}

fn parse_args(args: Vec<String>) -> std::result::Result<Cli, String> {
    let mut c = Cli { positional: vec![], dir: None, audio: None, max_res: None, subs: None, subs_only: false, force: false, name: None, data_dir: None, quiet: false, verbose: false };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut val = |n: &str| it.next().ok_or_else(|| format!("{n} attend une valeur"));
        match a.as_str() {
            "--dir" => c.dir = Some(val("--dir")?),
            "--audio" => {
                c.audio = Some(match val("--audio")?.as_str() {
                    "m4a" => AudioFormat::M4a,
                    "mp3" => AudioFormat::Mp3,
                    "original" => AudioFormat::Original,
                    x => return Err(format!("format audio inconnu : {x} (m4a, mp3 ou original)")),
                })
            }
            "--max-res" => c.max_res = Some(val("--max-res")?.parse().map_err(|_| "résolution invalide".to_string())?),
            "--subs" => c.subs = Some(val("--subs")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()),
            "--subs-only" => c.subs_only = true,
            "--force" => c.force = true,
            "--name" => c.name = Some(val("--name")?),
            "--data-dir" => c.data_dir = Some(PathBuf::from(val("--data-dir")?)),
            "--quiet" => c.quiet = true,
            "--verbose" => c.verbose = true,
            "-h" | "--help" => return Err(String::new()),
            x if x.starts_with("--") => return Err(format!("option inconnue : {x}")),
            _ => c.positional.push(a),
        }
    }
    Ok(c)
}

fn default_data_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join("Library/Application Support/Tribal Grab")
}

fn options_of(c: &Cli) -> TaskOptions {
    TaskOptions {
        dest_dir: c.dir.clone(),
        audio_format: c.audio,
        max_resolution: c.max_res,
        sub_mode: if c.subs_only { Some(SubMode::Only) } else if c.subs.is_some() { Some(SubMode::IfAvailable) } else { None },
        sub_langs: c.subs.clone(),
        duplicate: c.force.then_some(DuplicatePolicy::Redownload),
        name_template: c.name.clone(),
        ..Default::default()
    }
}

/// Répond automatiquement aux questions : la ligne de commande ne pose jamais rien.
fn auto_answer(e: &Engine, t: &Task, force: bool) {
    let a = match &t.question {
        Some(Question::Quality { choices }) => choices.first().map(|c| Answer::Quality { format_id: c.id.clone() }),
        Some(Question::Subtitles { tracks }) => Some(Answer::Subtitles { langs: tracks.iter().take(1).map(|x| x.lang.clone()).collect() }),
        Some(Question::Links { links, .. }) => Some(Answer::Links { urls: links.iter().map(|l| l.url.clone()).collect(), as_playlist: true }),
        Some(Question::Duplicate { .. }) => Some(Answer::Duplicate { action: if force { DuplicateAction::Redownload } else { DuplicateAction::Skip } }),
        Some(Question::Destination { suggested }) => Some(Answer::Destination { dir: suggested.clone() }),
        None => None,
    };
    if let Some(a) = a {
        let _ = e.answer(t.id, a);
    }
}

/// Attend la fin de toutes les tâches. Code de sortie : 0 si tout est terminé, 1 sinon.
async fn wait_all(e: &Engine, c: &Cli) -> i32 {
    let mut ever = false;
    let mut events = e.subscribe();
    let mut printed = std::collections::HashSet::new();
    loop {
        tokio::time::sleep(Duration::from_millis(400)).await;
        while let Ok(ev) = events.try_recv() {
            if let (true, Event::Log { message, .. }) = (c.verbose, ev) {
                eprintln!("\r\x1b[2K[moteur] {message}");
            }
        }
        let tasks = e.tasks(SortKey::Order, false);
        for t in tasks.iter().filter(|t| t.state == TaskState::NeedsInput) {
            auto_answer(e, t, c.force);
        }
        ever |= !tasks.is_empty();
        if !c.quiet
            && let Some(t) = tasks.iter().find(|t| t.state.is_active()) {
                let p = &t.progress;
                eprint!("\r\x1b[2K{} · {} · {:.0} % · {}/s", t.title.as_deref().unwrap_or(&t.url).chars().take(50).collect::<String>(), p.stage, p.percent, util::fmt_size(p.speed));
                let _ = std::io::stderr().flush();
            }
        for t in tasks.iter().filter(|t| t.state.is_done() && printed.insert(t.id)) {
            if !c.quiet {
                eprint!("\r\x1b[2K");
            }
            match (&t.state, &t.output_path, &t.error) {
                (TaskState::Finished, Some(p), _) => println!("{p}"),
                (_, _, Some(err)) => eprintln!("{} : {err}", t.title.as_deref().unwrap_or(&t.url)),
                _ => {}
            }
        }
        let pending = tasks.iter().any(|t| !t.state.is_done() && t.state != TaskState::Paused);
        if ever && !pending {
            return if tasks.iter().all(|t| matches!(t.state, TaskState::Finished | TaskState::Cancelled)) { 0 } else { 1 };
        }
    }
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let cmd = args.remove(0);
    if cmd == "-V" || cmd == "--version" {
        println!("tribal-grab {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let cli = match parse_args(args) {
        Ok(c) => c,
        Err(m) => {
            if !m.is_empty() {
                eprintln!("Erreur : {m}\n");
            }
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if cmd == "-h" || cmd == "--help" || cmd == "help" {
        println!("{USAGE}");
        return;
    }
    let engine = match Engine::start(cli.data_dir.clone().unwrap_or_else(default_data_dir)).await {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Erreur : {e}");
            std::process::exit(1);
        }
    };
    let code = match run(&engine, &cmd, &cli).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Erreur : {e}");
            1
        }
    };
    std::process::exit(code);
}

async fn run(e: &Engine, cmd: &str, c: &Cli) -> Result<i32> {
    let arg = |n: usize, what: &str| c.positional.get(n).cloned().ok_or_else(|| Error::Other(format!("{what} manquant")));
    match cmd {
        "get" => {
            let url = arg(0, "l'adresse")?;
            e.add(AddRequest { url, options: options_of(c), ..Default::default() })?;
            Ok(wait_all(e, c).await)
        }
        "text" => {
            let text = c.positional.join(" ");
            let ids = e.add_text(&text)?;
            for id in ids {
                if let Some(t) = e.task(id) {
                    let mut o = options_of(c);
                    o.referer = t.options.referer.clone();
                    let _ = o;
                }
            }
            Ok(wait_all(e, c).await)
        }
        "queue" => match c.positional.first().map(String::as_str) {
            Some("start") => {
                e.start_queue();
                Ok(wait_all(e, c).await)
            }
            Some("list") => {
                for t in e.tasks(SortKey::Order, false) {
                    println!("{:>4}  {:?}  {:>3.0} %  {}", t.id, t.state, t.progress.percent, t.title.unwrap_or(t.url));
                }
                Ok(0)
            }
            _ => Err(Error::Other("sous-commande attendue : start ou list".into())),
        },
        "probe" => {
            let r = e.probe(&arg(0, "l'adresse")?).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(0)
        }
        "search" => {
            for r in e.search(&c.positional.join(" ")).await? {
                println!("[{}] {}  {}", r.engine, r.title, r.url);
            }
            Ok(0)
        }
        "sites" => {
            let l = e.sites(c.positional.first().map(String::as_str).unwrap_or("")).await?;
            for s in &l.sites {
                println!("{s}");
            }
            eprintln!("{} sites", l.total);
            Ok(0)
        }
        "history" => {
            for h in e.history(c.positional.first().map(String::as_str).unwrap_or(""), "date", true, 200)? {
                println!("{}  {}  {}", h.finished_at, h.title, h.path.unwrap_or_default());
            }
            Ok(0)
        }
        "tools" => {
            for t in e.tools().await {
                println!("{:<8} {:<10} {}", t.name, t.version.unwrap_or_else(|| "absent".into()), t.path.unwrap_or_default());
            }
            Ok(0)
        }
        "update-ytdlp" => {
            let v = e.update_ytdlp().await?;
            println!("yt-dlp {v} installé");
            Ok(0)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(2)
        }
    }
}
