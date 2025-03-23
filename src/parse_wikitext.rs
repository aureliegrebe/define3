use std::collections::HashSet;

use Meaning;

#[derive(Debug, PartialEq)]
pub enum WikiContext {
    Heading1(String),
    Heading2(String),
    Heading3(String),
    Heading4(String),
    Heading5(String),
    Heading6(String),
}

use parse_wikitext::WikiContext::*;

impl WikiContext {
    pub fn precedence(&self) -> u32 {
        match self {
            &Heading1(_) => 1,
            &Heading2(_) => 2,
            &Heading3(_) => 3,
            &Heading4(_) => 4,
            &Heading5(_) => 5,
            &Heading6(_) => 6,
        }
    }

    pub fn text(&self) -> &String {
        match self {
            &Heading1(ref x) => &x,
            &Heading2(ref x) => &x,
            &Heading3(ref x) => &x,
            &Heading4(ref x) => &x,
            &Heading5(ref x) => &x,
            &Heading6(ref x) => &x,
        }
    }
}

pub struct ContextStack {
    contexts: Vec<WikiContext>,
    pub language: Option<String>,
    pub part_of_speech: Option<String>,
    pub gender: Option<String>,
}

impl ContextStack {
    pub fn apply(
        &mut self,
        context: WikiContext,
        languages: &HashSet<&str>,
        parts_of_speech: &HashSet<&str>,
    ) {
        let new_prec = context.precedence();
        // leave only lower-precedence contexts in the stack
        let contexts = &mut self.contexts;
        while contexts
            .last()
            .map_or(false, |c| c.precedence() >= new_prec)
        {
            // TODO: Check if this check is even necessary
            match contexts.pop() {
                None => (),
                Some(context) => {
                    if self.language.as_ref().unwrap_or(&String::from("")) == context.text() {
                        self.language = None;
                    }
                    if self.part_of_speech.as_ref().unwrap_or(&String::from("")) == context.text() {
                        self.part_of_speech = None;
                    }
                }
            }
        }
        if languages.contains(context.text().as_str()) {
            self.language = Some(context.text().clone());
        }
        if parts_of_speech.contains(context.text().as_str()) {
            self.part_of_speech = Some(context.text().clone());
        }
        if context.text().starts_with("{{") && context.text().ends_with("}}") {
            let (lang, pos, gender) = parse_template(
                &context
                    .text()
                    .get(2..context.text().len() - 2)
                    .unwrap_or(""),
            );
            match lang {
                Some(s) => self.language = Some(s),
                None => (),
            };
            match pos {
                Some(s) => self.part_of_speech = Some(s),
                None => (),
            };
            match gender {
                Some(s) => self.gender = Some(s),
                None => (),
            };
        }
        contexts.push(context);
    }

    pub fn new() -> ContextStack {
        ContextStack {
            contexts: Vec::new(),
            language: None,
            part_of_speech: None,
            gender: None,
        }
    }
}

pub fn parse_wikitext(
    text: String,
    languages: &HashSet<&str>,
    parts_of_speech: &HashSet<&str>,
) -> Vec<Meaning> {
    let mut result: Vec<Meaning> = Vec::new();
    let mut context_stack: ContextStack = ContextStack::new();

    let stack_apply = |context_stack: &mut ContextStack,
                       wiki_context: &dyn Fn(String) -> WikiContext,
                       line: &str,
                       slice: &Option<&str>| {
        slice.map_or_else(
            || {
                println!("Could not parse line: {}", line);
            },
            |slice| {
                context_stack.apply(wiki_context(slice.to_owned()), languages, parts_of_speech);
            },
        );
    };

    for line in text.lines() {
        if line.starts_with("======") && line.len() > 12 {
            stack_apply(
                &mut context_stack,
                &|x| Heading6(x),
                line,
                &line.get(6..line.len() - 6),
            );
        } else if line.starts_with("=====") && line.len() > 10 {
            stack_apply(
                &mut context_stack,
                &|x| Heading5(x),
                line,
                &line.get(5..line.len() - 5),
            );
        } else if line.starts_with("====") && line.len() > 8 {
            stack_apply(
                &mut context_stack,
                &|x| Heading4(x),
                line,
                &line.get(4..line.len() - 4),
            );
        } else if line.starts_with("===") && line.len() > 6 {
            stack_apply(
                &mut context_stack,
                &|x| Heading3(x),
                line,
                &line.get(3..line.len() - 3),
            );
        } else if line.starts_with("==") && line.len() > 4 {
            stack_apply(
                &mut context_stack,
                &|x| Heading2(x),
                line,
                &line.get(2..line.len() - 2),
            );
        } else if line.starts_with("=") && line.len() > 2 {
            stack_apply(
                &mut context_stack,
                &|x| Heading1(x),
                line,
                &line.get(1..line.len() - 1),
            );
        } else if line.starts_with("{{") && line.ends_with("}}") {
            stack_apply(
                &mut context_stack,
                &|x| Heading6(x),
                line,
                &line.get(0..line.len()),
            );
        } else if line.starts_with("# ") {
            context_stack.language.as_ref().and_then(|language| {
                context_stack.part_of_speech.as_ref().map(|part_of_speech| {
                    result.push(Meaning {
                        language: language.clone(),
                        part_of_speech: part_of_speech.clone(),
                        gender: context_stack.gender.clone(),
                        definition: String::from(&line[2..]),
                    })
                })
            });
        }
    }
    result
}

fn parse_template(line: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut tokens = line.rsplit('|').collect::<Vec<&str>>();
    let mut gender: Option<String> = None;
    let (lang, pos) = match tokens.pop() {
        None => (None, None),
        Some(s) => match s {
            // English
            "en-adj" => (Some("English".to_string()), Some("Adjective".to_string())),
            "en-adv" => (Some("English".to_string()), Some("Adverb".to_string())),
            "en-con" => (Some("English".to_string()), Some("Conjuction".to_string())),
            "en-det" => (Some("English".to_string()), Some("Determiner".to_string())),
            "en-interj" => (
                Some("English".to_string()),
                Some("Interjection".to_string()),
            ),
            "en-noun" => (Some("English".to_string()), Some("Noun".to_string())),
            "en-part" => (Some("English".to_string()), Some("Particle".to_string())),
            "en-prefix" => (Some("English".to_string()), Some("Prefix".to_string())),
            "en-prep" => (Some("English".to_string()), Some("Preposition".to_string())),
            "en-prep phrase" => (
                Some("English".to_string()),
                Some("Prepositional Phrase".to_string()),
            ),
            "en-pron" => (Some("English".to_string()), Some("Pronoun".to_string())),
            "en-proper noun" => (Some("English".to_string()), Some("Proper Noun".to_string())),
            "en-proverb" => (Some("English".to_string()), Some("Proverb".to_string())),
            "en-suffix" => (Some("English".to_string()), Some("Suffix".to_string())),
            "en-symbol" => (Some("English".to_string()), Some("Symbol".to_string())),
            "en-verb" => (Some("English".to_string()), Some("Verb".to_string())),

            // French
            "fr-adjective" => (Some("French".to_string()), Some("Adjective".to_string())),
            "fr-adverb" => (Some("French".to_string()), Some("Adverb".to_string())),
            "fr-card-adj" => (
                Some("French".to_string()),
                Some("Cardinal Adjective".to_string()),
            ),
            "fr-card-inv" => (Some("French".to_string()), Some("card-inv".to_string())),
            "fr-card-noun" => {
                gender = Some("".to_string());
                (
                    Some("French".to_string()),
                    Some("Cardinal Noun".to_string()),
                )
            }
            "fr-conjunction" => (Some("French".to_string()), Some("Conjuction".to_string())),
            "fr-det" => (Some("French".to_string()), Some("Determiner".to_string())),
            "fr-diacretical mark" => (
                Some("French".to_string()),
                Some("Diacretical Mark".to_string()),
            ),
            "fr-interj" => (Some("French".to_string()), Some("Interjection".to_string())),
            "fr-letter" => (Some("French".to_string()), Some("Letter".to_string())),
            "fr-noun" => {
                gender = Some("".to_string());
                (Some("French".to_string()), Some("Noun".to_string()))
            }
            "fr-past participle" => (
                Some("French".to_string()),
                Some("Past Participle".to_string()),
            ),
            "fr-phrase" => (Some("French".to_string()), Some("Phrase".to_string())),
            "fr-prefix" => (Some("French".to_string()), Some("Prefix".to_string())),
            "fr-postposition" => (Some("French".to_string()), Some("Postposition".to_string())),
            "fr-preposition" => (Some("French".to_string()), Some("Preposition".to_string())),
            "fr-pronoun" => (Some("French".to_string()), Some("Pronoun".to_string())),
            "fr-proper noun" => {
                gender = Some("".to_string());
                (Some("French".to_string()), Some("Proper Noun".to_string()))
            }
            "fr-punctuation mark" => (
                Some("French".to_string()),
                Some("Punctuation Mark".to_string()),
            ),
            "fr-proverb" => (Some("French".to_string()), Some("Proverb".to_string())),
            "fr-suffix" => (Some("French".to_string()), Some("Suffix".to_string())),
            "fr-verb" => (Some("French".to_string()), Some("Verb".to_string())),

            _ => (None, None),
        },
    };

    if gender.is_some() {
        gender = tokens.pop().map(|s| s.to_string());
    }

    (lang, pos, gender)
}
