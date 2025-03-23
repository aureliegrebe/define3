extern crate define3;
extern crate getopts;
extern crate regex;
extern crate rusqlite;

use define3::parse_wikitext::parse_wikitext;
use define3::PageContent;
use define3::{Module, Template, Word};

use getopts::Options;
use regex::Regex;
use rusqlite::{Connection, Transaction};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    // TODO: figure out list of languages automatically
    let languages: HashSet<&str> = [
        "Alemannic German",
        "Chinese",
        "English",
        "Esperanto",
        "French",
        "German",
        "Japanese",
        "Korean",
        "Lojban",
    ]
    .iter()
    .cloned()
    .collect();

    // TODO: figure out POS list automatically
    let parts_of_speech: HashSet<&str> = [
        "Adjective",
        "Adverb",
        "Brivla",
        "Cmavo",
        "Conjunction",
        "Definitions",
        "Gismu",
        "Hanja",
        "Hanzi",
        "Infix",
        "Initialism",
        "Interjection",
        "Kanji",
        "Noun",
        "Phrase",
        "Proper noun",
        "Rafsi",
        "Romanization",
        "Verb",
    ]
    .iter()
    .cloned()
    .collect();

    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help text");
    let matches = opts.parse(&args[1..]).unwrap();
    if matches.opt_present("h") || matches.free.len() != 1 {
        let brief = format!(
            "Usage: {} PATH_TO_enwiktionary-YYYYMMDD-pages-meta-current.xml [options]",
            args[0]
        );
        print!("{}", opts.usage(&brief));
        return;
    }
    let xml_path = matches.free[0].clone();

    let mut sqlite_path = dirs::data_dir().unwrap();
    sqlite_path.push("define3");
    std::fs::create_dir_all(&sqlite_path).unwrap();
    sqlite_path.push("define3.sqlite3");

    let mut conn = Connection::open(&sqlite_path).unwrap();
    let tx = Transaction::new(&mut conn, rusqlite::TransactionBehavior::Exclusive).unwrap();

    println!("Saving data to {:?}", sqlite_path);

    let mut count: u64 = 0;

    let mut templates: HashMap<String, String> = HashMap::new();
    let mut modules: HashMap<String, String> = HashMap::new();

    println!("Pass 1: Collecting templates and modules");

    tx.execute("DROP TABLE IF EXISTS templates", []).unwrap();
    tx.execute(
        "CREATE TABLE templates (
             name           text not null,
             content        text not null
         )",
        [],
    )
    .unwrap();

    tx.execute("DROP TABLE IF EXISTS modules", []).unwrap();
    tx.execute(
        "CREATE TABLE modules (
             name           text not null,
             content        text not null
         )",
        [],
    )
    .unwrap();

    let re_noinclude = Regex::new(r"<noinclude>(?P<text>(?s:.)*?)</noinclude>").unwrap();
    let re_includeonly = Regex::new(r"<includeonly>(?P<text>(?s:.)*?)</includeonly>").unwrap();
    let re_html_comment = Regex::new(r"<!--(?s:.)*?-->").unwrap();
    // TODO: combine link REs into one
    let re_display_link = Regex::new(r"\[\[[^\]]*?\|(?P<text>.*?)\]\]").unwrap();
    let re_link = Regex::new(r"\[\[(?P<text>.*?)\]\]").unwrap();
    // This technically doesn't work if some jerk decided to format a single quote.
    let re_bold = Regex::new(r"'''(?P<text>[^']*?)'''").unwrap();
    let re_italic = Regex::new(r"''(?P<text>[^']*?)''").unwrap();

    define3::parse_xml::for_pages(&xml_path, |page| {
        if page.title.starts_with("Template:") {
            let content = page.content;
            let content = re_noinclude.replace_all(&content, "");
            let content = re_html_comment.replace_all(&content, "");
            let content = content.into_owned();
            let content = match re_includeonly.captures(&content) {
                None => content.clone(),
                Some(captures) => captures.name("text").unwrap().as_str().to_owned(),
            };
            let title = &page.title[9..];
            tx.execute(
                "insert into templates (name, content) values (?1, ?2)",
                &[&title, &content.as_str()],
            )
            .unwrap();
            templates.insert(title.to_owned(), content);
        } else if page.title.starts_with("Module:") {
            let title = &page.title[7..];
            tx.execute(
                "insert into modules (name, content) values (?1, ?2)",
                [&title, &page.content.as_str()],
            )
            .unwrap();

            println!("Saved module: {}", page.title);
            let path = format!("modules/{}.lua", page.title);
            let path = Path::new(&path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut file = File::create(path).unwrap();
            file.write_all(page.content.as_bytes()).unwrap();
            modules.insert(title.to_owned(), page.content);
        }
    });

    println!("Pass 2: Collecting words");

    tx.execute("DROP TABLE IF EXISTS words", []).unwrap();
    tx.execute(
        "CREATE TABLE words (
             name           text not null,
             language       text not null,
             part_of_speech text not null,
             gender         text,
             definition     text not null
         )",
        [],
    )
    .unwrap();

    define3::parse_xml::for_pages(&xml_path, |page| {
        let page_content = match page.title.split(':').next() {
            Some("Template") => Box::new(PageContent::Template(Template {
                name: page.title,
                content: page.content,
            })),
            Some("Module") => Box::new(PageContent::Module(Module {
                name: page.title,
                src: page.content,
            })),
            _ => {
                let meanings = parse_wikitext(page.content, &languages, &parts_of_speech);
                Box::new(PageContent::Word(Word {
                    name: page.title,
                    meanings: meanings,
                }))
            }
        };
        match *page_content {
            PageContent::Word(word) => {
                count += 1;
                if count % 1000000 == 0 {
                    println!("{}: {}", count, word.name);
                }
                for meaning in &word.meanings {
                    let defn = &meaning.definition;
                    //let defn = re_link.replace_all(&defn, "\x1b[0;36m$x\x1b[0m");
                    let defn = re_display_link.replace_all(&defn, "$text");
                    let defn = re_link.replace_all(&defn, "$text");
                    let defn = re_html_comment.replace_all(&defn, "");
                    let defn = re_bold.replace_all(&defn, "$text");
                    let defn = re_italic.replace_all(&defn, "$text");
                    tx.execute(
                        "insert into words (name, language, part_of_speech, gender, definition)
                 values (?1, ?2, ?3, ?4, ?5)",
                        &[
                            &word.name,
                            &meaning.language,
                            &meaning.part_of_speech,
                            meaning.gender.as_ref().unwrap_or(&"".to_string()),
                            &defn.into_owned(),
                        ],
                    )
                    .unwrap();
                }
            }
            _ => (),
        }
    });

    tx.execute_batch(
        "create index words_name_idx on words(name);
         create index words_language_idx on words(language);
         create index words_part_of_speech_idx on words(part_of_speech);",
    )
    .unwrap();

    tx.commit().unwrap();
}
