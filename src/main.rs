#![feature(entry_and_modify)]

extern crate clap;
extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::rc::Rc;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use clap::{App, Arg};
use pest::Parser;
use pest::inputs::FileInput;

// force cargo to rebuild on grammar change
#[cfg(debug_assertions)]
const _GRAMMAR: &'static str = include_str!("org.pest");

#[derive(Parser)]
#[grammar = "org.pest"]
struct OrgParser;

type NameToBody = HashMap<String, Vec<String>>;
type PathToName = HashMap<String, String>;

fn tangle(f: &mut BufWriter<File>, name_to_body: &NameToBody, name: &str, prefix: &str) {
    let body = name_to_body.get(name).expect(&format!(
        "Macro `{}` is undefined!",
        name
    ));
    if body.len() == 0 {
        return;
    }
    let paddle = body[0].find(|x| x != ' ').unwrap_or(0);
    for line in body {
        // naive implementation of `:paddle no` source block argument
        let line = if paddle <= line.len() {
            &line[paddle..]
        } else {
            &line
        };
        if let Ok(macros) = OrgParser::parse_str(Rule::orgmacro, line) {
            for m in macros {
                let m: Vec<_> = m.into_inner().map(|x| String::from(x.as_str())).collect();
                let prefix = String::from(prefix) + &m[0];
                tangle(f, name_to_body, &m[1], &prefix);
                f.write(m[2].as_bytes()).unwrap();
            }
        } else {
            f.write(prefix.as_bytes()).unwrap();
            f.write(line.as_bytes()).unwrap();
        }
    }
}

fn tangle_all(path_to_name: &PathToName, name_to_body: &NameToBody) {
    for (path, name) in path_to_name {
        let path = Path::new(&path);
        let parent = path.parent().unwrap();
        if !parent.exists() {
            create_dir_all(parent).unwrap();
        }
        let mut f = BufWriter::new(File::create(path).unwrap());
        tangle(&mut f, &name_to_body, &name, &"");
    }
}

// dumb solution for borrowing problem in parse_doc inner loop
fn unwrap_clone<T: Clone>(x: &Option<T>) -> T {
    x.as_ref().map(|x| x.clone()).unwrap()
}

fn parse_doc(input: &str) -> (PathToName, NameToBody) {
    let input = FileInput::new(input).unwrap();
    let input = Rc::new(input);

    let docs = OrgParser::parse(Rule::doc, input).unwrap();

    let mut path_to_name = HashMap::new();
    let mut name_to_body: NameToBody = HashMap::new();

    let mut counter = 0;
    let mut auto_name = || {
        counter += 1;
        format!("__auto__{}", counter)
    };
    for doc in docs {
        for src in doc.into_inner() {
            let mut name = None;
            let mut path = None;
            for token in src.into_inner() {
                match token.as_rule() {
                    Rule::name => {
                        name = Some(String::from(token.as_str().trim()));
                    }
                    Rule::begin => {
                        for args in token.into_inner().collect::<Vec<_>>().chunks(2) {
                            if args[0].as_str() == ":tangle" {
                                path = Some(String::from(args[1].as_str()));
                                break;
                            }
                        }
                        if path != None && name == None {
                            name = Some(auto_name());
                        }
                    }
                    Rule::body => {
                        if name == None {
                            break;
                        }

                        let mut body: Vec<_> = token
                            .into_inner()
                            .map(|x| String::from(x.as_str()))
                            .collect();

                        name_to_body
                            .entry(unwrap_clone(&name))
                            .and_modify(|x| x.append(&mut body))
                            .or_insert(body);

                        if path != None {
                            path_to_name.insert(unwrap_clone(&path), unwrap_clone(&name));
                        }

                    }
                    _ => continue,
                }
            }
        }
    }
    (path_to_name, name_to_body)
}

fn main() {

    let matches = App::new("tangler")
        .version("1.0")
        .about("Q&D Org mode tangler.")
        .author("Ruslan Prokopchuk")
        .arg(
            Arg::with_name("INPUT")
                .help(".org file to tangle")
                .required(true)
                .index(1),
        )
        .get_matches();

    let input = matches.value_of("INPUT").unwrap();
    let (path_to_name, name_to_body) = parse_doc(input);
    tangle_all(&path_to_name, &name_to_body);
}
